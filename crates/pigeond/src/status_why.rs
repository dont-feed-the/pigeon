//! Why a version or a change of this computer is not safe yet (06 §6.4),
//! as `pigeon status` and the web say it.
//!
//! A version's reason comes from pigeon-core's state, but *sending* and
//! *group unreachable* say which backup machines the version waits for
//! once every one of them has been silent for [`SILENT`]. A change not
//! captured yet has its own reason: this computer was removed from the
//! group (09 §9.7), the person's copy differs from the group's version
//! (`why-differs`, 07 §7.6), its name does not work on every computer
//! (05 §5.4), it is a held deletion (07 §7.4), capture keeps failing, or
//! it simply waits for the next capture.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::state::{Custodian, Reason, State};
use pigeon_core::time::{Span, Time};
use pigeon_proto::api::{Arg, Message};

use crate::conflicts_file::{self, Kind};
use crate::group::Group;
use crate::status::{Observed, msg};
use crate::util::rfc3339;

/// How long every backup machine a version waits for may be silent before
/// its reason names them instead of saying it is being sent (06 §6.4).
pub const SILENT: Span = Span::minutes(5);

/// How many captures in a row must fail before a change's reason says so.
pub const REPEATED: u32 = 2;

/// The backup machines a version waits for, when all have been silent for
/// [`SILENT`]: their devices, and the latest news from any of them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Silent {
    /// The devices.
    pub devices: BTreeSet<String>,
    /// The latest time any of them was heard of, if ever.
    pub since: Option<i64>,
}

/// The backup machines among `custodians` that a version waits for (those
/// that do not hold it with its commit, a lost one aside), if every one of
/// them has been silent for [`SILENT`] at `now`.
pub fn silent(custodians: &[Custodian<String>], now: Time) -> Option<Silent> {
    let waiting: Vec<&Custodian<String>> = custodians
        .iter()
        .filter(|c| !(c.holds && c.has_commit) && !c.repository_lost)
        .collect();
    if waiting.is_empty()
        || waiting
            .iter()
            .any(|c| c.heard.is_some_and(|h| h.within(SILENT, now)))
    {
        return None;
    }
    Some(Silent {
        devices: waiting.iter().map(|c| c.device.clone()).collect(),
        since: waiting.iter().filter_map(|c| c.heard).max().map(|t| t.0),
    })
}

fn number(n: usize) -> Arg {
    Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

/// `machine` offline since `since`, or never heard of on this computer.
fn offline(
    who: impl IntoIterator<Item = String>,
    since: Option<i64>,
    names: &BTreeMap<String, String>,
) -> Message {
    let machine = Arg::Text(
        who.into_iter()
            .map(|d| names.get(&d).cloned().unwrap_or(d))
            .collect::<Vec<_>>()
            .join(", "),
    );
    // Never heard from, as on a computer that has just joined: no time to
    // say it went offline at.
    match since {
        Some(t) => msg(
            "why-machine-offline",
            &[("machine", machine), ("since", Arg::Text(rfc3339(t)))],
        ),
        None => msg("why-machine-not-heard", &[("machine", machine)]),
    }
}

/// The reason of the version `o` (06 §6.4), with the group's K and the
/// devices' names; on a removed computer, that it is no longer backed up.
pub fn version(
    o: &Observed,
    k: usize,
    names: &BTreeMap<String, String>,
    removed: bool,
) -> Option<Message> {
    let r = match &o.state {
        State::Local(r) | State::Published(r) => r,
        _ => return None,
    };
    if removed {
        return Some(msg("why-removed", &[]));
    }
    Some(match (r, &o.silent) {
        (Reason::CustodianOffline(who, since), _) => {
            offline(who.iter().cloned(), since.map(|t| t.0), names)
        }
        (Reason::TooFewCustodians { have, k }, _) => msg(
            "why-too-few-backups",
            &[("have", number(*have)), ("need", number(*k))],
        ),
        (Reason::Sending | Reason::GroupUnreachable(_), Some(s)) => {
            offline(s.devices.iter().cloned(), s.since, names)
        }
        _ => msg(
            "why-safe-copies",
            &[("have", Arg::Number(0)), ("need", number(k))],
        ),
    })
}

/// What status knows of this computer's own changes, read once.
#[derive(Debug, Default)]
pub struct Local {
    removed: bool,
    differs: BTreeSet<String>,
    names: BTreeSet<String>,
    deletions: BTreeSet<String>,
    failed: Option<&'static str>,
}

impl Local {
    /// Reads it for `g`.
    pub fn read(g: &Group) -> Local {
        let table = conflicts_file::read(g).unwrap_or_default();
        let held = crate::held_file::read(g);
        let (removed, mut differs, failed) = {
            let k = g.lock();
            (k.removed, k.diverging.clone(), k.capture_failed)
        };
        differs.extend(
            table
                .paths
                .iter()
                .filter(|(_, d)| {
                    d.state == conflicts_file::State::Conflict || d.kind != Kind::Suggestion
                })
                .map(|(p, _)| p.clone()),
        );
        Local {
            removed,
            differs,
            names: held.names(),
            deletions: held.held(),
            failed: failed.filter(|(n, _)| *n >= REPEATED).map(|(_, why)| why),
        }
    }

    /// Whether the person's copy of `path` differs from the group's
    /// version, which they choose between (07 §7.6).
    pub fn differs(&self, path: &str) -> bool {
        self.differs.contains(path)
    }

    /// Why the change of `path` is not captured yet.
    pub fn reason(&self, path: &str) -> Message {
        if self.removed {
            msg("why-removed", &[])
        } else if self.differs(path) {
            msg("why-differs", &[])
        } else if self.names.contains(path) {
            msg("why-name", &[])
        } else if self.deletions.contains(path) {
            msg("why-held-deletion", &[])
        } else if let Some(why) = self.failed {
            msg("why-capture-failed", &[("why", Arg::Text(why.to_owned()))])
        } else {
            msg("why-capturing", &[])
        }
    }
}

/// `answer` with `note` said around it: before, as progress, which the
/// command line prints and then reads the answer; and after, which the web
/// shows above a list and other clients may ignore.
pub fn noted(note: Option<Message>, answer: pigeon_proto::Response) -> Vec<pigeon_proto::Response> {
    use pigeon_proto::Response;
    let Some(message) = note else {
        return vec![answer];
    };
    vec![
        Response::Progress {
            message: message.clone(),
            done: None,
            total: None,
        },
        answer,
        Response::Said { message },
    ]
}

/// Why a capture failed, in a word the message selects on: `disk-full`,
/// `no-access`, `busy` or `other`.
pub fn failure(g: &Group, e: &crate::git::Error) -> &'static str {
    if crate::status::disk_full(g) {
        return "disk-full";
    }
    classify(e)
}

fn classify(e: &crate::git::Error) -> &'static str {
    use pigeon_git::Error as E;
    use std::io::ErrorKind;
    match e {
        E::Timeout { .. } => "busy",
        E::Spawn { source, .. } | E::Pipe { source, .. } => match source.kind() {
            ErrorKind::StorageFull => "disk-full",
            ErrorKind::PermissionDenied => "no-access",
            ErrorKind::WouldBlock | ErrorKind::ResourceBusy => "busy",
            _ => "other",
        },
        E::Failed { stderr_tail, .. } => {
            let t = stderr_tail.to_lowercase();
            if t.contains("no space left") {
                "disk-full"
            } else if t.contains("permission denied") || t.contains("operation not permitted") {
                "no-access"
            } else if t.contains(".lock") || t.contains("resource busy") {
                "busy"
            } else {
                "other"
            }
        }
        _ => "other",
    }
}

/// Records how `g`'s capture of this round ended.
pub fn note_capture(g: &Group, failed: Option<&crate::git::Error>) {
    let why = failed.map(|e| failure(g, e));
    let mut k = g.lock();
    k.capture_failed = match (why, k.capture_failed) {
        (None, _) => None,
        (Some(why), Some((n, _))) => Some((n.saturating_add(1), why)),
        (Some(why), None) => Some((1, why)),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custodian(device: &str, heard: Option<i64>, holds: bool) -> Custodian<String> {
        Custodian {
            device: device.to_owned(),
            heard: heard.map(Time),
            holds,
            has_commit: holds,
            checking: false,
            repository_lost: false,
        }
    }

    #[test]
    fn a_version_waits_on_silent_machines_only_when_all_it_waits_for_are_silent() {
        let now = Time(10_000);
        let long_ago = now.0 - SILENT.0 - 1;
        let lately = now.0 - 30;
        // The only backup machine it waits for is silent: say so.
        let one = [custodian("nas", Some(long_ago), false)];
        assert_eq!(
            silent(&one, now),
            Some(Silent {
                devices: BTreeSet::from(["nas".to_owned()]),
                since: Some(long_ago),
            })
        );
        // Another machine it waits for answers: it is being sent.
        let two = [
            custodian("nas", Some(long_ago), false),
            custodian("vault", Some(lately), false),
        ];
        assert_eq!(silent(&two, now), None);
        // A machine that already holds it is waited for no more.
        let held = [
            custodian("nas", None, false),
            custodian("vault", Some(lately), true),
        ];
        assert_eq!(
            silent(&held, now),
            Some(Silent {
                devices: BTreeSet::from(["nas".to_owned()]),
                since: None,
            })
        );
        assert_eq!(silent(&[], now), None);
        assert_eq!(silent(&[custodian("nas", Some(lately), false)], now), None);
    }

    #[test]
    fn a_silent_machine_replaces_sending_in_the_reason() {
        let names = BTreeMap::from([("nas".to_owned(), "Paul's NAS".to_owned())]);
        let mut o = Observed {
            path: "a.txt".into(),
            key: "K".into(),
            local: true,
            state: State::Local(Reason::Sending),
            level: pigeon_core::headline::Level::Working,
            since: 0,
            silent: Some(Silent {
                devices: BTreeSet::from(["nas".to_owned()]),
                since: Some(5),
            }),
        };
        let m = version(&o, 1, &names, false).unwrap();
        assert_eq!(m.id, "why-machine-offline");
        assert_eq!(m.args.get("machine"), Some(&Arg::Text("Paul's NAS".into())));
        assert_eq!(version(&o, 1, &names, true).unwrap().id, "why-removed");
        o.silent = None;
        assert_eq!(version(&o, 1, &names, false).unwrap().id, "why-safe-copies");
        o.state = State::Protected(Time(1));
        assert_eq!(version(&o, 1, &names, false), None);
    }

    #[test]
    fn each_change_not_captured_has_its_own_reason() {
        let mut l = Local {
            differs: BTreeSet::from(["mine.txt".to_owned()]),
            names: BTreeSet::from(["a:b.txt".to_owned()]),
            deletions: BTreeSet::from(["gone.txt".to_owned()]),
            ..Local::default()
        };
        assert_eq!(l.reason("mine.txt").id, "why-differs");
        assert_eq!(l.reason("a:b.txt").id, "why-name");
        assert_eq!(l.reason("gone.txt").id, "why-held-deletion");
        assert_eq!(l.reason("new.txt").id, "why-capturing");
        l.failed = Some("disk-full");
        let failed = l.reason("new.txt");
        assert_eq!(failed.id, "why-capture-failed");
        assert_eq!(failed.args.get("why"), Some(&Arg::Text("disk-full".into())));
        l.removed = true;
        assert_eq!(l.reason("mine.txt").id, "why-removed");
    }

    #[test]
    fn a_capture_failure_is_told_in_a_word() {
        let failed = |tail: &str| pigeon_git::Error::Failed {
            argv: vec!["annex".into(), "add".into()],
            status: Some(1),
            stderr_tail: tail.into(),
        };
        assert_eq!(
            classify(&failed("write error: No space left on device")),
            "disk-full"
        );
        assert_eq!(
            classify(&failed("open(a.txt): Permission denied")),
            "no-access"
        );
        assert_eq!(
            classify(&failed("Unable to create '.git/index.lock': File exists.")),
            "busy"
        );
        assert_eq!(classify(&failed("fatal: something else")), "other");
        let timeout = pigeon_git::Error::Timeout {
            argv: vec!["annex".into()],
            after: std::time::Duration::from_secs(1),
        };
        assert_eq!(classify(&timeout), "busy");
    }
}
