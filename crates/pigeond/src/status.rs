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
//!
//! A version's grace period counts from when this computer first saw it
//! (its key), so a later commit on `main` does not restart it (06 §6.7).
//! On a computer removed from the group (09 §9.7) no backup machine
//! counts, and the headline says so. The reasons are `status_why`'s.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::headline::{Level, checked_at, headline, version_level};
use pigeon_core::roster::Role as DeviceRole;
use pigeon_core::state::{Custodian, Settings, State, Version, rank, state};
use pigeon_core::time::{Span, Time};
use pigeon_proto::api::{self, Arg, Message, StatusReport, VersionStatus};

use crate::custody::{holders_for, present_for};
use crate::group::Group;
use crate::status_why::Silent;
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
    /// An earlier erasure covered these unavailable contents before a new reference.
    pub erased_restoration: bool,
    /// When this computer first saw it, while it was not safe yet; for a
    /// safe version, at most the time of the `main` it was observed in.
    pub since: i64,
    /// The backup machines it waits for, if all have been silent for a
    /// while (`status_why::SILENT`).
    pub silent: Option<Silent>,
}

/// Whether `s` is a state whose level counts from the version's age.
fn young(s: &State<String>) -> bool {
    matches!(
        s,
        State::Local(_)
            | State::Published(_)
            | State::Missing(pigeon_core::state::Missing::ContentMissing)
    )
}

/// When each of `states`, by key, was first seen not safe: as `first`
/// remembers, else `seed`, the time of the `main` they were read from.
/// `first` keeps only those not safe; after a look at every version
/// (`all`), it forgets those this look did not see, unless a look at a
/// newer `main` (one that read it while this one ran) added them.
fn ages(
    first: &mut BTreeMap<String, i64>,
    states: &[(String, bool)],
    seed: i64,
    all: bool,
) -> Vec<i64> {
    let out = states
        .iter()
        .map(|(key, not_safe)| {
            if *not_safe {
                *first.entry(key.clone()).or_insert(seed)
            } else {
                first.remove(key);
                seed
            }
        })
        .collect();
    if all {
        let seen: BTreeSet<&String> = states
            .iter()
            .filter(|(_, not_safe)| *not_safe)
            .map(|(k, _)| k)
            .collect();
        first.retain(|k, since| seen.contains(k) || *since >= seed);
    }
    out
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
    if let Some(doc) = &doc {
        checking.extend(
            doc.roster
                .devices
                .keys()
                .filter(|id| crate::membership::pending(g, &id.0))
                .map(|id| id.0.clone()),
        );
    }
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
    if crate::removed::is_removed(g) {
        // The backup machines refuse this computer: none counts here.
        custodians.clear();
    }
    let renewed = if g.git.purge_gate.policy().waiting {
        BTreeSet::new()
    } else {
        g.lock()
            .renewed_after_erasure
            .as_ref()
            .filter(|(observed, _)| observed == main)
            .map(|(_, keys)| keys.clone())
            .unwrap_or_default()
    };
    let writers = if renewed.is_empty() {
        Default::default()
    } else {
        g.lock()
            .engine
            .catalogue(&[main.to_owned()])
            .writers
            .clone()
    };
    let person = doc.as_ref().and_then(|d| {
        d.roster
            .devices
            .get(&pigeon_core::roster::DeviceId(me.to_owned()))
            .map(|d| d.person.clone())
    });
    let wanted_keys = keys
        .iter()
        .filter(|(path, _)| {
            prefix.is_empty() || *path == prefix || path.starts_with(&format!("{prefix}/"))
        })
        .map(|(_, key)| key.clone())
        .collect();
    let locations = holders_for(&g.git, &wanted_keys).await;
    let objects = present_for(&g.git, &wanted_keys).await;
    let mut out = Vec::new();
    for (path, key) in keys {
        if !(prefix.is_empty() || path == prefix || path.starts_with(&format!("{prefix}/"))) {
            continue;
        }
        let have = locations.get(&key).cloned().unwrap_or_default();
        let named = |id: &str| {
            id == me
                || [&peer_keys, &report_keys]
                    .iter()
                    .any(|m| m.get(id).and_then(|k| k.get(&path)) == Some(&key))
        };
        let local = objects.contains_key(&key);
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
        let erased_restoration = renewed.contains(&key)
            && matches!(
                st,
                State::Missing(pigeon_core::state::Missing::ContentMissing)
            );
        let decider = erased_restoration
            && doc
                .as_ref()
                .zip(person.as_ref())
                .is_some_and(|(doc, person)| {
                    pigeon_core::authority::deciders(&doc.roster, &writers, &[(&path, false)])
                        .0
                        .contains(person)
                });
        let silent = crate::status_why::silent(&version.custodians, Time(now));
        out.push(Observed {
            path,
            key,
            local: version.this_device_holds,
            erased_restoration,
            state: st,
            level: if erased_restoration && decider {
                Level::NeedsYou
            } else {
                Level::Safe
            },
            since: now,
            silent,
        });
    }
    // A version is at least as old as the first `main` seen holding it.
    let seed = if saved > 0 { saved.min(now) } else { now };
    let states: Vec<(String, bool)> = out
        .iter()
        .map(|o| (o.key.clone(), young(&o.state)))
        .collect();
    let since = ages(&mut g.lock().first_seen, &states, seed, prefix.is_empty());
    for (o, since) in out.iter_mut().zip(since) {
        o.since = since;
        o.level = o.level.join(version_level(
            &o.state,
            Span(now - since),
            GRACE,
            &s,
            Time(now),
        ));
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
    let history_waiting = retained
        .as_ref()
        .err()
        .is_some_and(crate::retention::waiting);
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
    let removed = crate::removed::is_removed(g);
    let level = if removed
        || current_unreadable
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
    let erased_missing = observed
        .iter()
        .filter(|o| o.erased_restoration && o.level == Level::NeedsYou)
        .count();
    let headline = match (level, &shortfall) {
        _ if removed => msg("headline-removed", &[]),
        _ if !current_unreadable && history_waiting => msg("headline-history-waiting", &[]),
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
        _ if erased_missing > 0 => msg(
            "headline-erased-restoration-missing",
            &[(
                "files",
                Arg::Number(i64::try_from(erased_missing).unwrap_or(i64::MAX)),
            )],
        ),
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
    // On a removed computer the headline says it of every file: the list
    // names a file only when asked about it.
    let listed = asked || !removed;
    let mut versions: Vec<VersionStatus> = observed
        .iter()
        .filter(|_| listed)
        .filter(|o| !pending.contains(&o.path))
        .filter(|o| asked || !matches!(o.state, State::Protected(_)))
        .map(|o| VersionStatus {
            path: o.path.clone(),
            number: 1,
            saved_at: rfc3339(o.since),
            state: proto_state(&o.state),
            observed_at: match o.state {
                State::Protected(t) => Some(rfc3339(t.0)),
                State::Uncertain(t) => t.map(|t| rfc3339(t.0)),
                _ => None,
            },
            reason: crate::status_why::version(o, k, &names, removed),
        })
        .collect();
    let local = crate::status_why::Local::read(g);
    versions.extend(
        pending
            .into_iter()
            .filter(|_| listed)
            .map(|path| VersionStatus {
                reason: Some(local.reason(&path)),
                path,
                number: 1,
                saved_at: rfc3339(now),
                state: api::State::Local,
                observed_at: None,
            }),
    );
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
            paused: None,
            scan_only: crate::capture_watch::scan_only(g).then(|| msg("status-scan-only", &[])),
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

    #[test]
    fn a_later_commit_does_not_restart_a_versions_grace() {
        let mut first = std::collections::BTreeMap::new();
        let not_safe = |k: &str| (k.to_owned(), true);
        // First seen in a main committed at 100.
        assert_eq!(
            super::ages(&mut first, &[not_safe("a")], 100, true),
            vec![100]
        );
        // Another capture moves main to 500: "a" keeps its age, "b" is new.
        assert_eq!(
            super::ages(&mut first, &[not_safe("a"), not_safe("b")], 500, true),
            vec![100, 500]
        );
        // A look at one folder forgets nothing it did not see.
        assert_eq!(
            super::ages(&mut first, &[not_safe("b")], 900, false),
            vec![500]
        );
        assert_eq!(first.len(), 2);
        // A look at an older main, finishing late, keeps what a newer one
        // added meanwhile ("c" at 950), and forgets what main dropped.
        first.insert("c".to_owned(), 950);
        assert_eq!(
            super::ages(&mut first, &[not_safe("a")], 900, true),
            vec![100]
        );
        assert_eq!(first.keys().collect::<Vec<_>>(), ["a", "c"]);
        first.remove("c");
        // Once safe, a version is forgotten; one no longer in main too.
        assert_eq!(
            super::ages(&mut first, &[("a".to_owned(), false)], 900, true),
            vec![900]
        );
        assert!(first.is_empty(), "{first:?}");
    }
}
