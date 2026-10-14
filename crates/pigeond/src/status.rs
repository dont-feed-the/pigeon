//! `pigeon status` (06): the state of each version of `main`, decided by
//! pigeon-core from what this device observes, and the headline.
//!
//! A custodian *holds* a version if git-annex's location log says so, and
//! *has the commit* if its fetched `main` names the same key at the same
//! path. It is heard of when a fetch from it last succeeded (13 §13.12).

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::domain::Facts;
use pigeon_core::headline::{Level, headline, version_level};
use pigeon_core::roster::Role as DeviceRole;
use pigeon_core::state::{Custodian, Reason, Settings, State, Version, rank, state};
use pigeon_core::time::{Span, Time};
use pigeon_proto::api::{self, Arg, Message, StatusReport, VersionStatus};

use crate::custody::{holders, present};
use crate::group::Group;
use crate::util::rfc3339;

/// How long a new version may take before it is *not safe yet* (06 §6.7).
pub const GRACE: Span = Span::minutes(10);

/// A message with arguments.
pub fn msg(id: &str, args: &[(&str, Arg)]) -> Message {
    Message {
        id: id.to_owned(),
        args: args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    }
}

/// The observed state of one version.
#[derive(Clone, Debug)]
pub struct Observed {
    /// The path.
    pub path: String,
    /// The key.
    pub key: String,
    /// Its state.
    pub state: State<String>,
    /// Its level.
    pub level: Level,
}

/// The settings of the roster.
pub fn settings(g: &Group) -> Settings {
    let mut s = Settings::default();
    if let Some(doc) = g.roster() {
        s.k = usize::try_from(doc.roster.k).unwrap_or(2);
        s.min_sites = usize::try_from(doc.roster.min_sites).unwrap_or(1);
    }
    s
}

/// The state of each version of `main` under `prefix` (a path in the
/// folder, `""` for all).
pub async fn observe(g: &Group, me: &str, prefix: &str) -> Vec<Observed> {
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return Vec::new();
    };
    let keys = g.git.keys(&main).await.unwrap_or_default();
    let doc = g.roster();
    let saved = g.git.commit_time(&main).await.unwrap_or(0);
    let now = crate::util::now();
    let s = settings(g);
    let heard = g.lock().heard.clone();
    let mut peer_keys: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut custodians: Vec<(String, String, Facts<String>, Option<String>)> = Vec::new();
    if let Some(doc) = &doc {
        for (id, d) in &doc.roster.devices {
            if id.0 == me || d.role == DeviceRole::Recovery {
                continue;
            }
            if let Some(theirs) = g.git.rev(&format!("refs/pigeon/peers/{}/main", id.0)).await {
                peer_keys.insert(id.0.clone(), g.git.keys(&theirs).await.unwrap_or_default());
            }
            if d.role == DeviceRole::Custodian
                && let Some(uuid) = &d.annex_uuid
            {
                let f = doc.facts.get(id).cloned().unwrap_or_default();
                let facts = Facts {
                    device: id.0.clone(),
                    fingerprints: f.fingerprints,
                    host: f.host,
                    domain: d.domain.clone(),
                };
                custodians.push((id.0.clone(), uuid.clone(), facts, d.site.clone()));
            }
        }
    }
    let mut out = Vec::new();
    for (path, key) in keys {
        if !(prefix.is_empty() || path == prefix || path.starts_with(&format!("{prefix}/"))) {
            continue;
        }
        let have: BTreeSet<String> = holders(&g.git, &key).await;
        let named = |id: &str| peer_keys.get(id).and_then(|k| k.get(&path)) == Some(&key);
        let version = Version {
            custodians: custodians
                .iter()
                .map(|(id, uuid, facts, site)| Custodian {
                    facts: facts.clone(),
                    site: site.clone(),
                    heard: heard.get(id).map(|t| Time(*t)),
                    holds: have.contains(uuid),
                    has_commit: named(id),
                    checking: false,
                    repository_lost: false,
                })
                .collect(),
            other_device_has_commit: peer_keys.keys().any(|id| named(id)),
            this_device_holds: present(g, &key).await.is_some(),
            erased: false,
        };
        let st = state(&version, &s, Time(now));
        let level = version_level(&st, Span(now - saved), GRACE, &s, Time(now));
        out.push(Observed {
            path,
            key,
            state: st,
            level,
        });
    }
    out
}

fn proto_state(s: &State<String>) -> api::State {
    match s {
        State::Missing(pigeon_core::state::Missing::Erased) => api::State::Erased,
        State::Missing(_) => api::State::Missing,
        State::Local(_) => api::State::Local,
        State::Published(_) => api::State::Published,
        State::Uncertain(_) | State::Protected(_) => api::State::Protected,
    }
}

fn reason(s: &State<String>, k: usize, names: &BTreeMap<String, String>) -> Option<Message> {
    let r = match s {
        State::Local(r) | State::Published(r) => r,
        _ => return None,
    };
    Some(match r {
        Reason::CustodianOffline(who, since) => msg(
            "why-machine-offline",
            &[
                (
                    "machine",
                    Arg::Text(
                        who.iter()
                            .map(|d| names.get(d).cloned().unwrap_or_else(|| d.clone()))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ),
                (
                    "since",
                    Arg::Text(since.map_or_else(String::new, |t| rfc3339(t.0))),
                ),
            ],
        ),
        _ => msg(
            "why-safe-copies",
            &[
                ("have", Arg::Number(0)),
                ("need", Arg::Number(i64::try_from(k).unwrap_or(2))),
            ],
        ),
    })
}

/// The answer to `Status` for the versions under `prefix`.
pub async fn report(g: &Group, me: &str, prefix: &str, not_shared: bool) -> StatusReport {
    let observed = observe(g, me, prefix).await;
    let k = settings(g).k;
    let names: BTreeMap<String, String> = g
        .roster()
        .map(|d| {
            d.roster
                .devices
                .iter()
                .map(|(id, x)| (id.0.clone(), x.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    let now = crate::util::now();
    let level = headline(observed.iter().map(|o| o.level));
    let unsafe_files = observed.iter().filter(|o| o.level != Level::Safe).count();
    let files = Arg::Number(i64::try_from(unsafe_files).unwrap_or(i64::MAX));
    let last = g.lock().heard.values().max().copied().unwrap_or(now);
    let headline = match level {
        Level::Safe => msg("headline-safe", &[("ago", Arg::Text(rfc3339(last)))]),
        Level::Working => msg("headline-working", &[("files", files)]),
        Level::NotCheckedRecently => msg(
            "headline-not-checked",
            &[("when", Arg::Text(rfc3339(last)))],
        ),
        Level::NotSafeYet | Level::NeedsYou => msg("headline-not-safe", &[("files", files)]),
    };
    let worst = observed
        .iter()
        .max_by_key(|o| rank(&o.state))
        .map_or(api::State::Protected, |o| proto_state(&o.state));
    let asked = !prefix.is_empty();
    let versions = observed
        .iter()
        .filter(|o| asked || !matches!(o.state, State::Protected(_)))
        .map(|o| VersionStatus {
            path: o.path.clone(),
            number: 1,
            saved_at: rfc3339(now),
            state: proto_state(&o.state),
            observed_at: match o.state {
                State::Protected(t) => Some(rfc3339(t.0)),
                State::Uncertain(t) => t.map(|t| rfc3339(t.0)),
                _ => None,
            },
            reason: reason(&o.state, k, &names),
        })
        .collect();
    let not_shared = if not_shared {
        g.lock()
            .not_shared
            .iter()
            .map(|(p, kind)| api::NotShared {
                path: p.clone(),
                reason: msg(
                    "not-shared-kind",
                    &[
                        ("path", Arg::Text(p.clone())),
                        ("kind", Arg::Text(kind.clone())),
                    ],
                ),
            })
            .collect()
    } else {
        Vec::new()
    };
    let level = match level {
        Level::Safe => api::Level::Safe,
        Level::Working => api::Level::Working,
        Level::NotCheckedRecently => api::Level::NotChecked,
        Level::NotSafeYet => api::Level::NotSafe,
        Level::NeedsYou => api::Level::NeedsYou,
    };
    StatusReport {
        headline,
        level,
        state: worst,
        versions,
        not_shared,
    }
}
