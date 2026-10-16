//! Optional collection (ADR 0025, 12 §12.2). This task is independent of
//! capture and custody. It exports only each group's own records to the
//! collector in that group's signed roster. A local endpoint is opt-in.

use crate::daemon::Daemon;
use crate::net::Directory;
use std::sync::Arc;
use std::time::Duration;
mod latency;
mod queue;
pub(crate) use latency::{ProtectionSample, flush_protection};
pub(crate) use latency::{accepted, captured};
pub(crate) use queue::Queue;

/// Stores a peer's telemetry only when the roster selected this device.
pub async fn receive(
    d: &Daemon,
    group: &str,
    device: &str,
    records: Vec<serde_json::Value>,
) -> bool {
    if d.telemetry_device(group).as_deref() != Some(d.device.as_str())
        || d.admit(group, device).is_none()
    {
        return false;
    }
    let Some(records) = pigeon_telemetry::receive(group, device, records) else {
        return false;
    };
    forward(d, &records).await;
    true
}

async fn forward(d: &Daemon, records: &[serde_json::Value]) {
    let endpoint = crate::config::load(&d.dirs.config_file())
        .ok()
        .and_then(|c| c.telemetry.endpoint);
    if let Some(endpoint) = endpoint {
        // Names from this collector's signed rosters, beside the IDs.
        let mut records = records.to_vec();
        let rosters: Vec<_> = d
            .groups()
            .iter()
            .filter_map(|g| Some((g.id.clone(), g.roster()?.roster)))
            .collect();
        pigeon_telemetry::names::name(&mut records, |kind, id| match kind {
            pigeon_telemetry::names::Kind::Group => rosters
                .iter()
                .find(|(g, _)| g == id)
                .map(|(_, r)| r.group.clone()),
            pigeon_telemetry::names::Kind::Device => rosters.iter().find_map(|(_, r)| {
                r.devices
                    .get(&pigeon_core::roster::DeviceId(id.to_owned()))
                    .map(|d| d.name.clone())
            }),
        });
        let records = &records;
        // An unavailable dashboard must never delay capture or repair.
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            pigeon_telemetry::forward(&endpoint, records),
        )
        .await;
    }
}

/// Runs until the daemon exits. Offline or oversubscribed telemetry is
/// discarded from its bounded queue; it never retries a custody operation.
pub async fn collect(d: Arc<Daemon>) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        for g in d.groups() {
            sample(&d, &g).await;
            let Some(collector) = d.telemetry_device(&g.id) else {
                continue;
            };
            let records = pigeon_telemetry::take_group(&g.id);
            if records.is_empty() {
                continue;
            }
            if collector == d.device {
                forward(&d, &records).await;
            } else {
                let _ = d
                    .node
                    .telemetry(
                        &collector,
                        &g.id,
                        Directory::addresses(&*d, &collector),
                        &records,
                    )
                    .await;
            }
        }
    }
}

/// Selects a collector in a signed roster commit; only admins may change
/// a group-wide setting. `off` leaves diagnostics on each device alone.
pub async fn set_collector(d: &Daemon, value: &str) -> Vec<pigeon_proto::Response> {
    use crate::api::{problem, said};
    use pigeon_core::roster::{DeviceId, Role, diff, may_sign};
    use pigeon_proto::Arg;
    let Some(g) = d.default_group() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let _busy = g.busy.lock().await;
    let Some(mut doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let selected = if value == "off" {
        None
    } else {
        let Some((id, _)) = doc.roster.devices.iter().find(|(id, device)| {
            device.role != Role::Recovery && (device.name == value || id.0 == value)
        }) else {
            return vec![problem(
                "setting-value",
                &[
                    ("key", Arg::Text("telemetry-device".into())),
                    ("value", Arg::Text(value.into())),
                ],
                None,
            )];
        };
        Some(id.clone())
    };
    let old = doc.roster.clone();
    doc.roster.telemetry_device = selected;
    let Some(change) = diff(&old, &doc.roster) else {
        return vec![problem("internal-error", &[], None)];
    };
    if !may_sign(&old, &DeviceId(d.device.clone()), &change) {
        return vec![problem("not-allowed", &[], None)];
    }
    if !change.is_empty() {
        if let Err(error) =
            crate::change::commit_roster(&g, &doc, "pigeon: change diagnostic collector", &d.tracer)
                .await
        {
            return vec![problem("internal-error", &[], Some(error))];
        }
        d.wake.notify_one();
    }
    vec![said(
        "setting-changed",
        &[
            ("key", Arg::Text("telemetry-device".into())),
            ("value", Arg::Text(value.into())),
        ],
    )]
}

async fn sample(d: &Daemon, g: &crate::group::Group) {
    let pending = if g.role == pigeon_core::filter::Role::Laptop {
        match crate::capture::pending(g, "").await {
            Ok(p) => p,
            Err(error) => {
                tracing::debug!(group = %g.id, error = %error, "telemetry-pending-unavailable");
                return;
            }
        }
    } else {
        std::collections::BTreeSet::new()
    };
    let mut counts = [0usize; 5];
    let (main, observed) = match crate::status::inventory(g, &d.device, "").await {
        Ok(inventory) => inventory,
        Err(error) => {
            tracing::debug!(group = %g.id, error = %error, "telemetry-inventory-unavailable");
            return;
        }
    };
    latency::protection(g, main, &observed, &pending);
    if let Ok(_busy) = g.busy.try_lock() {
        flush_protection(g).await;
    }
    for version in observed {
        if !pending.contains(&version.path) {
            counts[usize::from(pigeon_core::state::rank(&version.state))] += 1;
        }
    }
    counts[1] += pending.len();
    for (count, (label, uncertain)) in counts.into_iter().zip([
        ("missing", "false"),
        ("local", "false"),
        ("published", "false"),
        ("protected", "true"),
        ("protected", "false"),
    ]) {
        pigeon_telemetry::gauge(
            "pigeon.versions",
            count as f64,
            &[("group", &g.id), ("state", label), ("uncertain", uncertain)],
        );
    }
    let (queued, deferred) = {
        let local = g.lock();
        (
            local.queue_bytes,
            local.shortfall.as_ref().map_or(0, |s| s.keys),
        )
    };
    pigeon_telemetry::gauge("pigeon.queue.bytes", queued as f64, &[("group", &g.id)]);
    pigeon_telemetry::gauge(
        "pigeon.queue.depth",
        deferred as f64,
        &[("group", &g.id), ("class", "deferred")],
    );
}
