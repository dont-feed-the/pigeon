//! Laws of the install journal and its recovery (07 §7.5, ADR 0040).
//!
//! A model of one path's disk runs install's steps forward, with one action
//! of a person at any point and a crash at any point, then recovery; the
//! laws are checked on what recovery leaves.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::{int, pick};
use hegel::TestCase;
use pigeon_core::journal::{Begin, End, Entry, Found, Record, Step, entries, orphans, recover};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum C {
    Old,
    New,
    Person,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Modify,
    Create,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    Save,
    Remove,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Disk {
    p: Option<C>,
    aside: Option<C>,
    stage: Option<C>,
    index: Option<C>,
    records: Vec<Record>,
    /// A rename that would have replaced a name (never allowed).
    overwrites: u32,
}

impl Disk {
    fn new(kind: Kind) -> Disk {
        let old = if kind == Kind::Create {
            None
        } else {
            Some(C::Old)
        };
        Disk {
            p: old,
            aside: None,
            stage: None,
            index: old,
            records: Vec::new(),
            overwrites: 0,
        }
    }

    fn person(&mut self, act: Act) {
        self.p = match act {
            Act::Save => Some(C::Person),
            Act::Remove => None,
        };
    }

    fn end(&mut self, e: End) {
        self.records.push(Record::End(1, e));
    }

    fn ended(&self) -> bool {
        entries(&self.records).values().all(|e| e.end.is_some())
    }

    fn entry(&self) -> Option<Entry> {
        entries(&self.records).remove(&1)
    }

    fn found(&self) -> Found {
        Found {
            path: self.p.is_some(),
            aside: self.aside.is_some(),
            stage: self.stage.is_some(),
            aside_is_old: self.aside == Some(C::Old),
        }
    }

    /// Run one recovery step as pigeond would, NOREPLACE renames included.
    fn run(&mut self, step: Step, kind: Kind) {
        match step {
            Step::RenameIn => {
                if self.p.is_some() {
                    self.overwrites += 1;
                } else {
                    self.p = self.stage.take();
                }
            }
            Step::Restage => {
                self.index = if kind == Kind::Delete {
                    None
                } else {
                    Some(C::New)
                }
            }
            Step::PutAsideBack => {
                if self.p.is_some() {
                    self.overwrites += 1;
                } else {
                    self.p = self.aside.take();
                }
            }
            Step::Append(e) => self.end(e),
            Step::RemoveStage => self.stage = None,
        }
    }
}

fn begin(kind: Kind) -> Begin {
    Begin {
        seq: 1,
        path: "p".into(),
        old_key: (kind != Kind::Create).then(|| "old".into()),
        new_key: (kind != Kind::Delete).then(|| "new".into()),
        stat: None,
        stage: (kind != Kind::Delete).then(|| "stage/1".into()),
        aside: "aside/1".into(),
    }
}

/// Install's forward steps 2 to 9 of 07 §7.5, for one path. Returns false
/// once the entry has ended early.
fn forward(d: &mut Disk, kind: Kind, step: usize) -> bool {
    match step {
        0 => {
            if kind != Kind::Delete {
                d.stage = Some(C::New);
            }
        }
        1 => d.records.push(Record::Begin(begin(kind))),
        2 => {
            if kind != Kind::Create {
                match d.p.take() {
                    Some(c) => d.aside = Some(c),
                    None => {
                        d.stage = None;
                        d.end(End::Skip);
                        return false;
                    }
                }
            }
        }
        3 => {
            let passed = match kind {
                Kind::Create => d.p.is_none(),
                _ => d.aside == Some(C::Old),
            };
            if passed {
                d.records.push(Record::Proved(vec![1]));
            } else {
                match (d.p, d.aside) {
                    (None, Some(a)) => {
                        d.p = Some(a);
                        d.aside = None;
                        d.end(End::Skip);
                    }
                    (Some(_), Some(_)) => d.end(End::Recovered),
                    _ => d.end(End::Skip),
                }
                d.stage = None;
                return false;
            }
        }
        4 => {
            if kind != Kind::Delete {
                if d.p.is_some() {
                    d.stage = None;
                    d.end(End::Raced);
                    return false;
                }
                d.p = d.stage.take();
            }
        }
        5 => {
            d.index = if kind == Kind::Delete {
                None
            } else {
                Some(C::New)
            }
        }
        _ => {
            d.end(End::Done);
            return false;
        }
    }
    true
}

struct Run {
    kind: Kind,
    disk: Disk,
    act: Option<Act>,
    /// The person acted after pigeon renamed the new file in.
    after_rename: bool,
}

/// Install with one person action at step `at` and a crash after `crash`
/// steps (7 or more: no crash).
fn run(tc: &TestCase) -> Run {
    let kind = pick(tc, &[Kind::Modify, Kind::Create, Kind::Delete]);
    let act = pick(tc, &[None, Some(Act::Save), Some(Act::Remove)]);
    let at = int(tc, 0, 8) as usize;
    let crash = int(tc, 0, 8) as usize;
    let mut d = Disk::new(kind);
    let mut after_rename = false;
    for step in 0..7 {
        if step == at
            && let Some(a) = act
        {
            after_rename = step > 4;
            d.person(a);
        }
        if step == crash || !forward(&mut d, kind, step) {
            break;
        }
    }
    if at > crash.min(6)
        && let Some(a) = act
    {
        after_rename = d.p == Some(C::New) || after_rename;
        d.person(a);
    }
    Run {
        kind,
        disk: d,
        act,
        after_rename,
    }
}

fn recover_all(d: &mut Disk, kind: Kind, stop_after: Option<usize>) {
    let staged: Vec<String> = d.stage.map(|_| "stage/1".to_string()).into_iter().collect();
    if !orphans(&entries(&d.records), &staged).is_empty() {
        d.stage = None;
    }
    let Some(e) = d.entry() else { return };
    for (i, step) in recover(&e, d.found()).into_iter().enumerate() {
        if stop_after == Some(i) {
            return;
        }
        d.run(step, kind);
    }
}

#[hegel::test(test_cases = 3000)]
fn recovery_ends_every_entry_and_a_second_does_nothing(tc: TestCase) {
    let mut r = run(&tc);
    recover_all(&mut r.disk, r.kind, None);
    assert!(r.disk.ended());
    assert_eq!(r.disk.stage, None, "{:?}", r.disk);
    if let Some(e) = r.disk.entry() {
        assert!(recover(&e, r.disk.found()).is_empty());
    }
    assert_eq!(r.disk.overwrites, 0, "pigeon never renames over a name");
}

#[hegel::test(test_cases = 3000)]
fn a_crash_during_recovery_ends_as_one_recovery(tc: TestCase) {
    let r = run(&tc);
    let mut once = r.disk.clone();
    recover_all(&mut once, r.kind, None);
    let mut twice = r.disk.clone();
    recover_all(&mut twice, r.kind, Some(int(&tc, 0, 4) as usize));
    recover_all(&mut twice, r.kind, None);
    let state = |d: &Disk| {
        (
            d.p,
            d.aside,
            d.stage,
            d.index,
            d.entry().and_then(|e| e.end),
        )
    };
    assert_eq!(state(&once), state(&twice));
}

#[hegel::test(test_cases = 3000)]
fn nothing_the_person_wrote_is_lost(tc: TestCase) {
    let mut r = run(&tc);
    recover_all(&mut r.disk, r.kind, None);
    let d = &r.disk;
    match r.act {
        Some(Act::Save) => assert!(
            d.p == Some(C::Person) || d.aside == Some(C::Person),
            "{:?}",
            d
        ),
        Some(Act::Remove) if r.after_rename => {
            assert_eq!(d.p, None, "a deletion after install stays")
        }
        _ => {}
    }
    if r.act.is_none() {
        let allowed: &[Option<C>] = match r.kind {
            Kind::Modify => &[Some(C::Old), Some(C::New)],
            Kind::Create => &[None, Some(C::New)],
            Kind::Delete => &[Some(C::Old), None],
        };
        assert!(allowed.contains(&d.p), "{:?}", d);
        assert!(
            d.p == d.index || d.p.is_none() && d.aside.is_some(),
            "INV-Journal: {:?}",
            d
        );
    }
}
