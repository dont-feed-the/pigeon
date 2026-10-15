//! Verdicts of `authority::judge` on random histories, logged as
//! `Authority` trace events for TLC (04 §4.4, 13 §13.3). The world is
//! `Authority.tla`'s: person 1 the admin, person 2 the owner of folder `F`,
//! person 3 a member; device i is person i's and device 4 a fresh key; path
//! `f` is `F/f` and `r` a root file. Each commit has one accepted parent and
//! is signed by any device, which may be absent or removed; it writes or
//! deletes versions, and may transfer `F`, add device 4 or remove a device,
//! with or without a Force. Each line logs the commit as the model's `New`
//! takes it and the code's verdict; `AuthorityTrace.tla` checks that the
//! verdict is `Judge`'s. With `PIGEON_TRACE_DIR` set, one NDJSON file per
//! seed is written there for `scripts/check-code-traces`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(feature = "tla-trace")]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use pigeon_core::authority::{self, Entry, ROSTER, Step, Tree, Verdict};
use pigeon_core::roster::{Device, DeviceId, Folder, Person, PersonId, Role, Roster, Rule};
use pigeon_core::sig;
use pigeon_core::trace::{Authority, Event, Value};

const POINTER: &str = "/annex/objects/SHA256E-s3--2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae.txt\n";
const PATHS: [(&str, &str); 2] = [("f", "F/f"), ("r", "r")];

/// A small deterministic generator, so that a seed names a run.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

fn key(d: u64) -> [u8; 32] {
    [u8::try_from(d).unwrap(); 32]
}

fn dev(d: u64) -> DeviceId {
    sig::device_id(&key(d))
}

fn pid(p: u64) -> PersonId {
    PersonId(format!("p{p}"))
}

fn device(d: u64, p: u64) -> Device {
    Device {
        person: pid(p),
        name: format!("device {d}"),
        role: Role::Laptop,
        annex_uuid: Some(format!("00000000-0000-4000-8000-00000000000{d}")),
        maxsize: None,
    }
}

fn genesis() -> Roster {
    Roster {
        rules: 1,
        group: "g".into(),
        k: 1,
        retention_days: 30,
        persons: (1..=3)
            .map(|p| {
                (
                    pid(p),
                    Person {
                        name: format!("person {p}"),
                        admin: p == 1,
                    },
                )
            })
            .collect(),
        devices: (1..=3).map(|d| (dev(d), device(d, d))).collect(),
        removed: BTreeMap::new(),
        folders: vec![Folder {
            path: "F".into(),
            rule: Rule::Owned(pid(2)),
        }],
    }
}

fn owner(r: &Roster) -> u64 {
    match &r.folders[0].rule {
        Rule::Owned(p) => p.0[1..].parse().unwrap(),
        Rule::AddOnly(_) => unreachable!(),
    }
}

fn int(n: u64) -> Value {
    Value::Int(i64::try_from(n).unwrap())
}

fn rec(fields: Vec<(&str, Value)>) -> Value {
    Value::Record(fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

/// A roster edit as the model writes it: every field, those its kind does
/// not use 0.
fn edit(k: &str, d: u64, p: u64, old: u64, new: u64) -> Value {
    rec(vec![
        ("k", Value::Str(k.into())),
        ("d", int(d)),
        ("p", int(p)),
        ("old", int(old)),
        ("new", int(new)),
    ])
}

/// An accepted commit: its model tree (path → version, 0 for none), its
/// roster and the object ID of its roster file.
#[derive(Clone)]
struct Accepted {
    tree: BTreeMap<&'static str, u64>,
    roster: Roster,
    roster_oid: String,
}

fn code_tree(a: &Accepted) -> Tree {
    let entry = |oid: String| Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid,
    };
    let mut t: Tree = PATHS
        .iter()
        .filter(|(q, _)| a.tree[q] != 0)
        .map(|(q, path)| ((*path).to_owned(), entry(format!("v{}", a.tree[q]))))
        .collect();
    t.insert(ROSTER.to_owned(), entry(a.roster_oid.clone()));
    t
}

/// The events of seed `seed`: each commit, numbered from 2 as the model's
/// `cm` (1 is the genesis), with the code's verdict.
fn run(seed: u64) -> Vec<Event> {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut accepted: BTreeMap<u64, Accepted> = BTreeMap::new();
    accepted.insert(
        1,
        Accepted {
            tree: PATHS.iter().map(|(q, _)| (*q, 0)).collect(),
            roster: genesis(),
            roster_oid: "roster-1".into(),
        },
    );
    let mut out = Vec::new();
    for n in 2..2 + 10 + rng.below(8) {
        let ids: Vec<u64> = accepted.keys().copied().collect();
        let parent = if rng.one_in(3) {
            ids[usize::try_from(rng.below(ids.len() as u64)).unwrap()]
        } else {
            *ids.last().unwrap()
        };
        let p = accepted[&parent].clone();
        let signer = 1 + rng.below(4);
        let force = rng.one_in(4);
        let mut tree = p.tree.clone();
        for (q, _) in PATHS {
            match rng.below(3) {
                0 => {
                    tree.insert(q, n);
                }
                1 if tree[q] != 0 => {
                    tree.insert(q, 0);
                }
                _ => {}
            }
        }
        let mut roster = p.roster.clone();
        let mut edits = Vec::new();
        let mut used = BTreeSet::new();
        for _ in 0..rng.below(3) {
            match rng.below(3) {
                0 if used.insert("own") => {
                    let old = owner(&roster);
                    let new = 1 + (old + rng.below(2)) % 3;
                    roster.folders[0].rule = Rule::Owned(pid(new));
                    edits.push(edit("own", 0, 0, old, new));
                }
                1 if !p.roster.devices.contains_key(&dev(4)) && used.insert("dev4") => {
                    let person = 1 + rng.below(3);
                    roster.devices.insert(dev(4), device(4, person));
                    edits.push(edit("add", 4, person, 0, 0));
                }
                2 => {
                    let present: Vec<u64> = (1..=4)
                        .filter(|d| roster.devices.contains_key(&dev(*d)))
                        .filter(|d| *d != 4 || !used.contains("dev4"))
                        .collect();
                    if present.is_empty() {
                        continue;
                    }
                    let d = present[usize::try_from(rng.below(present.len() as u64)).unwrap()];
                    used.insert(if d == 4 { "dev4" } else { "rem" });
                    let gone = roster.devices.remove(&dev(d)).unwrap();
                    roster.removed.insert(dev(d), gone.annex_uuid);
                    edits.push(edit("rem", d, 0, 0, 0));
                }
                _ => {}
            }
        }
        let changed = roster != p.roster;
        let after_model = Accepted {
            tree: tree.clone(),
            roster: roster.clone(),
            roster_oid: if changed {
                format!("roster-{n}")
            } else {
                p.roster_oid.clone()
            },
        };
        let (before, after) = (code_tree(&p), code_tree(&after_model));
        let blobs: BTreeMap<String, Vec<u8>> = after
            .iter()
            .filter(|(path, _)| !authority::is_pigeon_path(path))
            .map(|(_, e)| (e.oid.clone(), POINTER.as_bytes().to_vec()))
            .collect();
        let mut body = format!(
            "tree 1111111111111111111111111111111111111111\n\
             parent 2222222222222222222222222222222222222222\n\
             author pigeon <pigeon@localhost> {n} +0000\n\
             committer pigeon <pigeon@localhost> {n} +0000\n\npigeon: commit {n}\n"
        );
        if force {
            body.push_str("\nPigeon-Force: traced\n");
        }
        let raw = sig::sign(body.as_bytes(), &key(signer)).unwrap();
        let parsed = Ok(roster.clone());
        let verdict = authority::judge(&Step {
            raw: &raw,
            roster: &p.roster,
            before: &before,
            after: &after,
            blobs: &blobs,
            new_roster: changed.then_some(&parsed),
        });
        let acc = match &verdict {
            Verdict::Accepted(s, new) => {
                assert_eq!(*s, dev(signer));
                assert_eq!(new.as_ref().is_some(), changed);
                true
            }
            Verdict::Refused(_) => false,
            Verdict::Unknown(why) => panic!("unknown verdict {why} under rules 1"),
        };
        if acc {
            accepted.insert(n, after_model);
        }
        let versions = |q: &str| {
            Value::List(if tree[q] == 0 {
                vec![]
            } else {
                vec![int(tree[q])]
            })
        };
        out.push(Event::new(
            Authority::Judge,
            "l1",
            n - 1,
            vec![
                ("parent".to_owned(), int(parent)),
                ("signer".to_owned(), int(signer)),
                (
                    "tree".to_owned(),
                    rec(vec![("f", versions("f")), ("r", versions("r"))]),
                ),
                ("redits".to_owned(), Value::List(edits)),
                ("force".to_owned(), Value::Bool(force)),
                ("acc".to_owned(), Value::Bool(acc)),
            ],
        ));
    }
    out
}

#[test]
fn traces_for_tlc() {
    let seeds: u64 = std::env::var("PIGEON_TRACE_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let dir = std::env::var_os("PIGEON_TRACE_DIR").map(PathBuf::from);
    let (mut acc, mut refused) = (0, 0);
    for seed in 1..=seeds {
        let events = run(seed);
        for e in &events {
            if e.vars
                .iter()
                .any(|(k, v)| k == "acc" && *v == Value::Bool(true))
            {
                acc += 1;
            } else {
                refused += 1;
            }
        }
        let lines: Vec<String> = events.iter().map(Event::to_ndjson).collect();
        if let Some(d) = &dir {
            fs::create_dir_all(d).unwrap();
            fs::write(
                d.join(format!("authority-{seed:03}.ndjson")),
                lines.join("\n") + "\n",
            )
            .unwrap();
        }
    }
    eprintln!("{seeds} histories: {acc} commits accepted, {refused} refused");
    if seeds >= 20 {
        assert!(acc > 0 && refused > 0, "the histories accept and refuse");
    }
}
