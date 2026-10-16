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

use pigeon_core::headline::{Level, checked_at, headline, version_level};
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
    /// Whether the annex content is present on this computer.
    pub local: bool,
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
    observe_checked(g, me, prefix).await.unwrap_or_default()
}

pub(crate) async fn observe_checked(
    g: &Group,
    me: &str,
    prefix: &str,
) -> Result<Vec<Observed>, crate::git::Error> {
    inventory(g, me, prefix).await.map(|(_, versions)| versions)
}

/// A consistent immutable accepted ref and the observations read from it.
pub(crate) async fn inventory(
    g: &Group,
    me: &str,
    prefix: &str,
) -> Result<(String, Vec<Observed>), crate::git::Error> {
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return Err(crate::git::Error::Parse {
            argv: vec!["main".into()],
            detail: "accepted main unavailable".into(),
        });
    };
    let versions = observe_at(g, me, prefix, &main).await?;
    Ok((main, versions))
}

async fn observe_at(
    g: &Group,
    me: &str,
    prefix: &str,
    main: &str,
) -> Result<Vec<Observed>, crate::git::Error> {
    let keys = g.git.keys(main).await?;
    let doc = g.roster();
    let saved = g.git.commit_time(main).await.unwrap_or(0);
    let now = crate::util::now();
    let s = settings(g);
    let (mut heard, mut lost, reported, mut checking) = {
        let k = g.lock();
        let mains: BTreeMap<String, String> = k
            .report_files
            .iter()
            .filter_map(|(d, r)| Some((d.clone(), r.main.clone()?)))
            .collect();
        let checking: BTreeSet<String> = k
            .report_files
            .iter()
            .filter(|(_, r)| r.checking)
            .map(|(id, _)| id.clone())
            .collect();
        (k.heard.clone(), k.lost.clone(), mains, checking)
    };
    lost.extend(crate::report::lost(g));
    if g.role == pigeon_core::filter::Role::Custodian
        && crate::health::own_report(g, me)
            .await
            .is_some_and(|r| r.checking)
    {
        checking.insert(me.to_owned());
    }
    let mut peer_keys: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut report_keys: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut custodians: Vec<(String, String)> = Vec::new();
    if let Some(doc) = &doc {
        for (id, d) in &doc.roster.devices {
            if id.0 == me {
                if d.role == DeviceRole::Custodian
                    && g.role == pigeon_core::filter::Role::Custodian
                    && let Some(uuid) = &d.annex_uuid
                    && g.git.config("annex.uuid").await.as_ref() == Some(uuid)
                {
                    custodians.push((id.0.clone(), uuid.clone()));
                    heard.insert(id.0.clone(), now);
                }
                continue;
            }
            if d.role == DeviceRole::Recovery {
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
            id == me
                || [&peer_keys, &report_keys]
                    .iter()
                    .any(|m| m.get(id).and_then(|k| k.get(&path)) == Some(&key))
        };
        let local = present(g, &key).await.is_some();
        let version = Version {
            custodians: custodians
                .iter()
                .map(|(id, uuid)| Custodian {
                    device: id.clone(),
                    heard: heard.get(id).map(|t| Time(*t)),
                    holds: if id == me { local } else { have.contains(uuid) },
                    has_commit: named(id),
                    checking: checking.contains(id),
                    repository_lost: lost.contains(id),
                })
                .collect(),
            other_device_has_commit: peer_keys
                .keys()
                .chain(report_keys.keys())
                .any(|id| named(id)),
            this_device_holds: local,
            erased: false,
        };
        let st = state(&version, &s, Time(now));
        let level = version_level(&st, Span(now - saved), GRACE, &s, Time(now));
        out.push(Observed {
            path,
            key,
            local: version.this_device_holds,
            state: st,
            level,
        });
    }
    Ok(out)
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
        Reason::CustodianOffline(who, since) => {
            let machine = Arg::Text(
                who.iter()
                    .map(|d| names.get(d).cloned().unwrap_or_else(|| d.clone()))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            // Never heard from, as on a computer that has just joined: no
            // time to say it went offline at.
            match since {
                Some(t) => msg(
                    "why-machine-offline",
                    &[("machine", machine), ("since", Arg::Text(rfc3339(t.0)))],
                ),
                None => msg("why-machine-not-heard", &[("machine", machine)]),
            }
        }
        Reason::TooFewCustodians { have, k } => msg(
            "why-too-few-backups",
            &[
                (
                    "have",
                    Arg::Number(i64::try_from(*have).unwrap_or(i64::MAX)),
                ),
                ("need", Arg::Number(i64::try_from(*k).unwrap_or(i64::MAX))),
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
    inventory_report(g, me, prefix, not_shared).await.0
}

/// Counts use the same main/ref snapshot and pending scan as the headline.
pub(crate) struct Inventory {
    pub current: Option<pigeon_core::health::Counts>,
    pub retained: Option<pigeon_core::health::Counts>,
}

pub(crate) async fn inventory_report(
    g: &Group,
    me: &str,
    prefix: &str,
    not_shared: bool,
) -> (StatusReport, Inventory) {
    let pending = crate::capture::pending(g, prefix).await;
    let pending_unreadable = pending.is_err();
    let pending = pending.unwrap_or_default();
    let (observed, retained, current_unreadable) = if prefix.is_empty() {
        match crate::retention::snapshot(g).await {
            Ok(snapshot) => {
                let observed = match &snapshot.main {
                    Some(main) => observe_at(g, me, prefix, main).await,
                    None => Err(crate::git::Error::Parse {
                        argv: vec!["main".into()],
                        detail: "accepted main unavailable".into(),
                    }),
                };
                let unreadable = observed.is_err();
                let observed = observed.unwrap_or_default();
                let current = observed.iter().map(|o| o.key.clone()).collect();
                let retained = crate::retention::states(g, me, &current, &snapshot.tips).await;
                (observed, retained, unreadable)
            }
            Err(e) => (Vec::new(), Err(e), true),
        }
    } else {
        let observed = observe_checked(g, me, prefix).await;
        let unreadable = observed.is_err();
        (observed.unwrap_or_default(), Ok(Vec::new()), unreadable)
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
    let mut level = headline(
        observed
            .iter()
            .map(|o| o.level)
            .chain(historical_levels.iter().copied()),
    );
    let unsafe_files = observed
        .iter()
        .filter(|o| o.level != Level::Safe && !pending.contains(&o.path))
        .count()
        + pending.len()
        + historical_levels
            .iter()
            .filter(|l| **l != Level::Safe)
            .count();
    let files = Arg::Number(i64::try_from(unsafe_files).unwrap_or(i64::MAX));
    let backups = g
        .roster()
        .map(|doc| {
            doc.roster
                .devices
                .values()
                .filter(|d| d.role == DeviceRole::Custodian)
                .count()
        })
        .unwrap_or(0);
    let too_few_backups = unsafe_files > 0 && backups < k;
    if too_few_backups && level < Level::NotSafeYet {
        level = Level::NotSafeYet;
    } else if !pending.is_empty() && level < Level::Working {
        level = Level::Working;
    }
    let last =
        checked_at(observed.iter().map(|o| &o.state).chain(retained.iter())).map_or(now, |at| at.0);
    let full = disk_full(g);
    let (mut lost, shortfall) = {
        let k = g.lock();
        (k.lost.clone(), k.shortfall.clone())
    };
    lost.extend(crate::report::lost(g));
    let peers = crate::report::problems(g);
    let held_table = crate::held_file::read(g);
    let (held, held_names) = (held_table.held().len(), held_table.names().len());
    let level = if current_unreadable
        || history_unreadable
        || pending_unreadable
        || full
        || held > 0
        || held_names > 0
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
        _ if current_unreadable || history_unreadable => msg("headline-inventory-unreadable", &[]),
        _ if pending_unreadable => msg("headline-checking", &[]),
        _ if full => msg("headline-disk-full", &[]),
        _ if held > 0 => msg(
            "headline-held",
            &[(
                "files",
                Arg::Number(i64::try_from(held).unwrap_or(i64::MAX)),
            )],
        ),
        _ if held_names > 0 => msg(
            "headline-held-names",
            &[(
                "files",
                Arg::Number(i64::try_from(held_names).unwrap_or(i64::MAX)),
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
        _ if !pending.is_empty() && level != Level::NeedsYou => msg(
            "headline-capturing",
            &[(
                "files",
                Arg::Number(i64::try_from(pending.len()).unwrap_or(i64::MAX)),
            )],
        ),
        _ if too_few_backups && level != Level::NeedsYou => msg("headline-too-few-backups", &[]),
        _ if level == Level::Safe
            && observed.is_empty()
            && retained.is_empty()
            && pending.is_empty() =>
        {
            msg("headline-empty", &[])
        }
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
    let worst = if pending.is_empty() {
        worst(observed.iter().map(|o| &o.state).chain(retained.iter()))
    } else {
        api::State::Local
    };
    let asked = !prefix.is_empty();
    let inventory = Inventory {
        current: (!current_unreadable && !pending_unreadable).then(|| {
            pigeon_core::health::current(observed.iter().map(|o| (&o.path, &o.state)), &pending)
        }),
        retained: (!current_unreadable && !history_unreadable).then(|| {
            retained.iter().map(pigeon_core::health::Counts::one).fold(
                pigeon_core::health::Counts::default(),
                pigeon_core::health::Counts::combine,
            )
        }),
    };
    let mut versions: Vec<VersionStatus> = observed
        .iter()
        .filter(|o| !pending.contains(&o.path))
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
    versions.extend(pending.into_iter().map(|path| VersionStatus {
        path,
        number: 1,
        saved_at: rfc3339(now),
        state: api::State::Local,
        observed_at: None,
        reason: Some(msg("why-capturing", &[])),
    }));
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
    (
        StatusReport {
            headline,
            level,
            state: worst,
            versions,
            not_shared,
        },
        inventory,
    )
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
