//! `pigeon status` (06): the state of each version of `main`, decided by
//! pigeon-core from what this device observes, and the headline.
//!
//! A custodian *holds* a version if git-annex's location log says so, and
//! *has the commit* if its fetched `main` names the same key at the same
//! path. It is heard of when a fetch from it last succeeded (13 §13.12),
//! and its repository is lost while it answers but serves the group no
//! more, or its report says so: it then counts for nothing, and the
//! headline says *Needs you*. A custodian's report (06 §6.3) tells the
//! `main` it has, and whether it is full or short of room.

use std::collections::{BTreeMap, BTreeSet};

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

/// Whether the disk of `g`'s repository has less free space than its
/// `annex.diskreserve`, below which git-annex refuses every transfer
/// (`checkDiskSpace` in `Annex/Content/LowLevel.hs`; 08 §8.9).
pub fn disk_full(g: &Group) -> bool {
    pigeon_sys::fs::space(&g.git_dir)
        .is_ok_and(|s| s.available < crate::settings::reserve_bytes(g.role, &g.git_dir))
}

/// This device's name in `g`'s roster.
pub fn this_name(g: &Group, me: &str) -> String {
    g.roster()
        .and_then(|doc| {
            let id = pigeon_core::roster::DeviceId(me.to_owned());
            doc.roster.devices.get(&id).map(|x| x.name.clone())
        })
        .unwrap_or_else(|| "This computer".to_owned())
}

/// The settings of the roster.
pub fn settings(g: &Group) -> Settings {
    let mut s = Settings::default();
    if let Some(doc) = g.roster() {
        s.k = usize::try_from(doc.roster.k).unwrap_or(2);
    }
    s
}

/// The state of each version of `main` under `prefix` (a path in the
/// folder, `""` for all).
pub async fn observe(g: &Group, me: &str, prefix: &str) -> Vec<Observed> {
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return Vec::new();
    };
    observe_at(g, me, prefix, &main).await
}

async fn observe_at(g: &Group, me: &str, prefix: &str, main: &str) -> Vec<Observed> {
    let keys = g.git.keys(main).await.unwrap_or_default();
    let doc = g.roster();
    let saved = g.git.commit_time(main).await.unwrap_or(0);
    let now = crate::util::now();
    let s = settings(g);
    let (heard, mut lost, reported) = {
        let k = g.lock();
        let mains: BTreeMap<String, String> = k
            .report_files
            .iter()
            .filter_map(|(d, r)| Some((d.clone(), r.main.clone()?)))
            .collect();
        (k.heard.clone(), k.lost.clone(), mains)
    };
    lost.extend(crate::report::lost(g));
    let mut peer_keys: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut report_keys: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut custodians: Vec<(String, String)> = Vec::new();
    if let Some(doc) = &doc {
        for (id, d) in &doc.roster.devices {
            if id.0 == me || d.role == DeviceRole::Recovery {
                continue;
            }
            let fetched = g.git.rev(&format!("refs/pigeon/peers/{}/main", id.0)).await;
            if let Some(theirs) = &fetched {
                peer_keys.insert(id.0.clone(), g.git.keys(theirs).await.unwrap_or_default());
            }
            if let Some(main) = reported.get(&id.0).filter(|m| fetched.as_ref() != Some(*m)) {
                report_keys.insert(id.0.clone(), g.git.keys(main).await.unwrap_or_default());
            }
            if d.role == DeviceRole::Custodian
                && let Some(uuid) = &d.annex_uuid
            {
                custodians.push((id.0.clone(), uuid.clone()));
            }
        }
    }
    let mut out = Vec::new();
    for (path, key) in keys {
        if !(prefix.is_empty() || path == prefix || path.starts_with(&format!("{prefix}/"))) {
            continue;
        }
        let have: BTreeSet<String> = holders(&g.git, &key).await;
        let named = |id: &str| {
            [&peer_keys, &report_keys]
                .iter()
                .any(|m| m.get(id).and_then(|k| k.get(&path)) == Some(&key))
        };
        let version = Version {
            custodians: custodians
                .iter()
                .map(|(id, uuid)| Custodian {
                    device: id.clone(),
                    heard: heard.get(id).map(|t| Time(*t)),
                    holds: have.contains(uuid),
                    has_commit: named(id),
                    checking: false,
                    repository_lost: lost.contains(id),
                })
                .collect(),
            other_device_has_commit: peer_keys
                .keys()
                .chain(report_keys.keys())
                .any(|id| named(id)),
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

/// The worst of `states`, the lowest in rank (missing, local, published,
/// …); protected when there is none.
pub(crate) fn worst<'a>(states: impl Iterator<Item = &'a State<String>>) -> api::State {
    states
        .min_by_key(|s| rank(*s))
        .map_or(api::State::Protected, proto_state)
}

/// The answer to `Status` for the versions under `prefix`.
pub async fn report(g: &Group, me: &str, prefix: &str, not_shared: bool) -> StatusReport {
    let (observed, retained) = if prefix.is_empty() {
        match crate::retention::snapshot(g).await {
            Ok(snapshot) => {
                let observed = match &snapshot.main {
                    Some(main) => observe_at(g, me, prefix, main).await,
                    None => Vec::new(),
                };
                let current = observed.iter().map(|o| o.key.clone()).collect();
                let retained = crate::retention::states(g, me, &current, &snapshot.tips).await;
                (observed, retained)
            }
            Err(e) => (observe(g, me, prefix).await, Err(e)),
        }
    } else {
        (observe(g, me, prefix).await, Ok(Vec::new()))
    };
    let history_unreadable = retained.is_err();
    let retained = retained.unwrap_or_default();
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
    let settings = settings(g);
    let historical_levels: Vec<Level> = retained
        .iter()
        .map(|s| version_level(s, Span::hours(1), GRACE, &settings, Time(now)))
        .collect();
    let level = headline(
        observed
            .iter()
            .map(|o| o.level)
            .chain(historical_levels.iter().copied()),
    );
    let unsafe_files = observed.iter().filter(|o| o.level != Level::Safe).count()
        + historical_levels
            .iter()
            .filter(|l| **l != Level::Safe)
            .count();
    let files = Arg::Number(i64::try_from(unsafe_files).unwrap_or(i64::MAX));
    let last = g.lock().heard.values().max().copied().unwrap_or(now);
    let full = disk_full(g);
    let (mut lost, shortfall) = {
        let k = g.lock();
        (k.lost.clone(), k.shortfall.clone())
    };
    lost.extend(crate::report::lost(g));
    let peers = crate::report::problems(g);
    let held = crate::held_file::read(g).held().len();
    let level = if history_unreadable
        || full
        || held > 0
        || !lost.is_empty()
        || shortfall.is_some()
        || !peers.is_empty()
    {
        Level::NeedsYou
    } else {
        level
    };
    let name = |d: &str| Arg::Text(names.get(d).cloned().unwrap_or_else(|| d.to_owned()));
    let headline = match (level, &shortfall) {
        _ if full => msg("headline-disk-full", &[]),
        _ if held > 0 => msg(
            "headline-held",
            &[(
                "files",
                Arg::Number(i64::try_from(held).unwrap_or(i64::MAX)),
            )],
        ),
        (_, Some(s)) => msg(
            "headline-no-capacity",
            &crate::admission::no_capacity_args(s),
        ),
        _ if let Some(d) = lost.first() => msg("headline-machine-lost", &[("machine", name(d))]),
        _ if let Some((code, d, files, missing)) = peers.first() => match *code {
            "disk-full" => msg("headline-machine-full", &[("machine", name(d))]),
            _ => {
                let n = |x: u64| Arg::Number(i64::try_from(x).unwrap_or(i64::MAX));
                msg(
                    "headline-machine-no-capacity",
                    &[
                        ("machine", name(d)),
                        ("files", n(*files)),
                        ("missing_mb", n(missing.div_ceil(1_000_000))),
                    ],
                )
            }
        },
        (level, None) => match level {
            Level::Safe => msg("headline-safe", &[("ago", Arg::Text(rfc3339(last)))]),
            Level::Working => msg("headline-working", &[("files", files)]),
            Level::NotCheckedRecently => msg(
                "headline-not-checked",
                &[("when", Arg::Text(rfc3339(last)))],
            ),
            Level::NotSafeYet | Level::NeedsYou => msg("headline-not-safe", &[("files", files)]),
        },
    };
    let worst = worst(observed.iter().map(|o| &o.state).chain(retained.iter()));
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

#[cfg(test)]
mod tests {
    use pigeon_core::state::{Reason, State};
    use pigeon_core::time::Time;
    use pigeon_proto::api;

    #[test]
    fn the_folder_is_as_safe_as_its_least_safe_version() {
        let protected = State::Protected(Time(5));
        let published = State::Published(Reason::Sending);
        assert_eq!(
            super::worst([&protected, &published, &protected].into_iter()),
            api::State::Published
        );
        assert_eq!(super::worst(std::iter::empty()), api::State::Protected);
    }
}
