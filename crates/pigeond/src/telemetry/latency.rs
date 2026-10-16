//! Diagnostic lags are separate from safety clocks. Acceptance is timed
//! when a capture reaches this peer; protection is the first sampled
//! unprotected→protected transition in this process. Historic already-safe
//! versions are not presented as new protection measurements after restart.

use crate::{group::Group, status::Observed};
use pigeon_core::{
    catalogue::Content,
    latency::{Milestone, Sample},
    state::State,
};
use std::collections::{BTreeMap, BTreeSet};

const CAPACITY: usize = 4096;

fn record(g: &Group, id: String, milestone: Milestone, captured: i64, observation: &str) {
    record_at(g, id, milestone, captured, observation, crate::util::now());
}

fn record_at(
    g: &Group,
    id: String,
    milestone: Milestone,
    captured: i64,
    observation: &str,
    at: i64,
) {
    let sample = g
        .lock()
        .telemetry_latency
        .observe(CAPACITY, id, milestone, captured, at);
    let name = match milestone {
        Milestone::Accepted => "pigeon.sync.lag",
        Milestone::Protected => "pigeon.protect.lag",
    };
    let labels = [("group", g.id.as_str()), ("observation", observation)];
    match sample {
        Some(Sample::Seconds(seconds)) => {
            pigeon_telemetry::histogram(name, seconds as f64, &labels)
        }
        Some(Sample::ClockSkew) => pigeon_telemetry::counter("pigeon.lag.clock_skew", 1, &labels),
        None => {}
    }
}

/// Local acceptance follows the successful capture CAS; no extra disk read.
pub(crate) fn captured(g: &Group, commit: &str) {
    record(
        g,
        format!("commit:{commit}"),
        Milestone::Accepted,
        crate::util::now(),
        "local",
    );
}

/// At most 256 newly accepted metadata records, never a content scan. Skipped
/// commits do not count. The truncation counter makes the bounded omission explicit.
pub(crate) async fn accepted(g: &Group, old: Option<&str>, target: &str) {
    let mut args = vec![
        "rev-list",
        "--max-count=256",
        "--no-commit-header",
        "--format=%H%x09%ct%x09%s",
        target,
    ];
    if let Some(old) = old {
        args.extend(["--not", old]);
    }
    let log = match g.git.run(&args).await {
        Ok(log) => log,
        Err(error) => {
            tracing::debug!(group = %g.id, error = %error, "telemetry-acceptance-unavailable");
            return;
        }
    };
    if log.lines().count() == 256 {
        pigeon_telemetry::counter("pigeon.lag.history_truncated", 1, &[("group", &g.id)]);
    }
    let mut capture_rows = 0;
    let mut admitted_rows = 0;
    for line in log.lines() {
        let mut fields = line.splitn(3, '\t');
        let (Some(id), Some(at), Some("pigeon: capture")) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        capture_rows += 1;
        if !g.lock().engine.accepted(id) {
            continue;
        }
        admitted_rows += 1;
        if let Ok(at) = at.parse() {
            record(g, format!("commit:{id}"), Milestone::Accepted, at, "peer");
        }
    }
    tracing::debug!(group = %g.id, rows = log.lines().count(), capture_rows, admitted_rows, "telemetry-accepted-metadata");
}

/// One bounded sample; its observed time survives delayed metadata resolution.
#[derive(Debug)]
pub(crate) struct ProtectionSample {
    main: String,
    at: i64,
    versions: Vec<(String, String)>,
}

fn identity(path: &str, key: &str) -> String {
    format!("version:{path}:{key}")
}

pub(super) fn protection(
    g: &Group,
    main: String,
    observed: &[Observed],
    pending: &BTreeSet<String>,
) {
    let mut versions = Vec::new();
    let mut known = g.lock();
    for version in observed {
        if pending.contains(&version.path) {
            continue;
        }
        let id = identity(&version.path, &version.key);
        if matches!(version.state, State::Protected(_)) {
            if known.telemetry_unprotected.contains(&id) {
                if versions.len() < CAPACITY {
                    versions.push((version.path.clone(), version.key.clone()));
                }
            } else {
                // Already-safe startup content must not become a new timing
                // after restart or a later repair.
                known
                    .telemetry_latency
                    .observe(CAPACITY, id, Milestone::Protected, 0, 0);
            }
        } else if known.telemetry_unprotected.len() < CAPACITY {
            known.telemetry_unprotected.insert(id);
        }
    }
    if !versions.is_empty() && known.telemetry_protection.is_none() {
        known.telemetry_protection = Some(ProtectionSample {
            main,
            at: crate::util::now(),
            versions,
        });
    }
}

/// Caller holds the group lock. The sampler may stage this while a long
/// round runs; the round resolves it before returning, without another
/// content/status scan. Immutable sample refs preserve the actual observation.
pub(crate) async fn flush_protection(g: &Group) {
    let Some(sample) = g.lock().telemetry_protection.take() else {
        return;
    };
    let main = &sample.main;
    if !matches!(crate::verify::judge(g, None, main).await, Ok(j) if j.accepted.as_deref() == Some(main))
    {
        return;
    }
    let origins = {
        let mut known = g.lock();
        let view = known.engine.catalogue(std::slice::from_ref(main));
        let mut origins = BTreeMap::new();
        for shown in view.shown.values().flatten() {
            let Some(rank) = known.engine.rank(&shown.origin) else {
                continue;
            };
            let paths = match &shown.version.content {
                Content::File(_) => vec![shown.path.clone()],
                Content::Package(files) => files
                    .keys()
                    .map(|p| format!("{}/{p}", shown.path))
                    .collect(),
            };
            for path in paths {
                origins.insert(path, rank.when);
            }
        }
        origins
    };
    let Ok(keys) = g.git.keys(main).await else {
        return;
    };
    for (path, key) in sample.versions {
        let Some(at) = origins.get(&path) else {
            continue;
        };
        if keys.get(&path) != Some(&key) {
            continue;
        }
        let id = identity(&path, &key);
        let waiting = g.lock().telemetry_unprotected.remove(&id);
        if waiting {
            record_at(
                g,
                id,
                Milestone::Protected,
                *at,
                "sampled-transition",
                sample.at,
            );
        }
    }
}
