//! Verdicts of `commit::judge` on random histories, logged as `Authority`
//! trace events for TLC (04 §4.4, §4.5, 13 §13.3). The world is
//! `Authority.tla`'s: person 1 the admin, person 2 the owner of folder `F`,
//! person 3 a member; device i is person i's and device 4 a fresh key; path
//! `f` is `F/f` and `r` a root file, whose model value is the versions at
//! the path and its conflict copies. Each commit, signed by any device with
//! or without a Force, builds on one accepted parent and either writes or
//! deletes versions and may transfer `F`, add device 4 or remove a device;
//! or rejects or withdraws a proposal the code keeps (a commit refused for
//! folder rules alone); or adopts one whose base it holds, as
//! `proposal::adoption` writes it or with a change dropped or one more.
//! Who wrote each path is kept as the catalogue keeps it (ADR 0070): an
//! adoption writes as its proposal's author.
//! Each line logs the commit as the model's `New` takes it and the code's
//! verdict; `AuthorityTrace.tla` checks that the verdict is `Judge`'s. With `PIGEON_TRACE_DIR` set, one
//! NDJSON file per seed is written there for `scripts/check-code-traces`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(feature = "tla-trace")]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use pigeon_core::authority::{
    self, Decided, Decision, Entry, ROSTER, Refusal, Tree, Verdict, Writers,
};
use pigeon_core::commit::{self, Commit};
use pigeon_core::names::{self, Stamp};
use pigeon_core::proposal;
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
        invitation: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        k: 1,
        retention_days: 30,
        telemetry_device: None,
        relay: None,
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

/// An accepted commit: its tree as the code sees it, its roster, its
/// accepted history, itself included, and who wrote each slot.
#[derive(Clone)]
struct Kept {
    code: Tree,
    roster: Roster,
    anc: BTreeSet<u64>,
    writers: Writers,
}

/// A proposal the code keeps (04 §4.5): a commit on an accepted parent,
/// its base, refused for folder rules alone, with no roster edit.
#[derive(Clone)]
struct Proposal {
    base: u64,
    signer: u64,
    code: Tree,
    verdict: Verdict,
}

fn entry(oid: String) -> Entry {
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid,
    }
}

/// The model's tree of a code tree: for each model path, the versions at
/// its path and at its conflict copies (05 §5.3), `v<n>` being version n.
fn model_tree(code: &Tree) -> BTreeMap<&'static str, BTreeSet<u64>> {
    PATHS
        .iter()
        .map(|(q, path)| {
            let versions = code
                .iter()
                .filter(|(k, _)| k.as_str() == *path || names::is_copy_of(k, path))
                .map(|(_, e)| e.oid[1..].parse::<u64>().unwrap())
                .collect();
            (*q, versions)
        })
        .collect()
}

/// `code` with model path `q` written as version `n` (or deleted): its
/// conflict copies go, as a save of the path resolves them.
fn write(code: &mut Tree, q: &str, n: Option<u64>) {
    let path = PATHS.iter().find(|(p, _)| *p == q).unwrap().1;
    code.retain(|k, _| k != path && !names::is_copy_of(k, path));
    if let Some(n) = n {
        code.insert(path.to_owned(), entry(format!("v{n}")));
    }
}

fn raw(n: u64, signer: u64, force: bool) -> Vec<u8> {
    let mut body = format!(
        "tree 1111111111111111111111111111111111111111\n\
         parent 2222222222222222222222222222222222222222\n\
         author pigeon <pigeon@localhost> {n} +0000\n\
         committer pigeon <pigeon@localhost> {n} +0000\n\npigeon: commit {n}\n"
    );
    if force {
        body.push_str("\nPigeon-Force: traced\n");
    }
    sig::sign(body.as_bytes(), &key(signer)).unwrap()
}

fn blobs_of(after: &Tree) -> BTreeMap<String, Vec<u8>> {
    after
        .iter()
        .filter(|(path, _)| !authority::is_pigeon_path(path))
        .map(|(_, e)| (e.oid.clone(), POINTER.as_bytes().to_vec()))
        .collect()
}

fn pick<T: Copy>(rng: &mut Rng, xs: &[T]) -> T {
    xs[usize::try_from(rng.below(xs.len() as u64)).unwrap()]
}

/// A commit of the history: its parents (the accepted one first), tree,
/// roster edits, decisions, the code's verdict, the proposal it is, and
/// the proposal it adopts.
struct Made {
    parents: Vec<u64>,
    after: Tree,
    roster: Roster,
    edits: Vec<Value>,
    dec: Vec<Value>,
    verdict: Verdict,
    proposal: bool,
    adopts: Option<u64>,
}

/// One commit on the accepted `parent` that writes or deletes versions and
/// may transfer `F`, add device 4 or remove a device.
fn ordinary(rng: &mut Rng, n: u64, parent: u64, p: &Kept, raw: &[u8]) -> Made {
    let mut after = p.code.clone();
    let tree = model_tree(&p.code);
    for (q, _) in PATHS {
        match rng.below(3) {
            0 => write(&mut after, q, Some(n)),
            1 if !tree[q].is_empty() => write(&mut after, q, None),
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
                let d = pick(rng, &present);
                used.insert(if d == 4 { "dev4" } else { "rem" });
                let gone = roster.devices.remove(&dev(d)).unwrap();
                roster.removed.insert(dev(d), gone.annex_uuid);
                edits.push(edit("rem", d, 0, 0, 0));
            }
            _ => {}
        }
    }
    let changed = roster != p.roster;
    if changed {
        after.insert(ROSTER.to_owned(), entry(format!("roster-{n}")));
    }
    let parsed = Ok(roster.clone());
    let verdict = commit::judge(&Commit {
        raw,
        rosters: std::slice::from_ref(&p.roster),
        before: &p.code,
        after: &after,
        writers: &p.writers,
        blobs: &blobs_of(&after),
        new_roster: changed.then_some(&parsed),
        decisions: &BTreeMap::new(),
        adopted: &[],
    });
    if let Verdict::Accepted(_, new) = &verdict {
        assert_eq!(new.as_ref().is_some(), changed);
    }
    let proposal = edits.is_empty() && matches!(verdict, Verdict::Refused(Refusal::NotAllowed(_)));
    Made {
        parents: vec![parent],
        after,
        roster,
        edits,
        dec: Vec::new(),
        verdict,
        proposal,
        adopts: None,
    }
}

/// A decision on the proposal `x` (04 §4.5), on the accepted `parent`.
#[allow(clippy::too_many_arguments, reason = "the history's parts")]
fn decide(
    rng: &mut Rng,
    n: u64,
    parent: u64,
    p: &Kept,
    x: u64,
    prop: &Proposal,
    at: &Kept,
    raw: &[u8],
) -> Made {
    let verdict = if rng.one_in(2) {
        Decided::Rejected
    } else {
        Decided::Withdrawn
    };
    let id = format!("c{x}");
    let path = authority::decision_path(&id);
    let mut after = p.code.clone();
    after.insert(path.clone(), entry(format!("decision-{n}")));
    let about = commit::about(
        &p.roster,
        &at.roster,
        &dev(prop.signer),
        &at.code,
        &prop.code,
        &p.code,
        &p.writers,
    );
    let decisions = BTreeMap::from([(
        path,
        Ok(Decision {
            proposal: id,
            verdict,
            about,
        }),
    )]);
    let v = commit::judge(&Commit {
        raw,
        rosters: std::slice::from_ref(&p.roster),
        before: &p.code,
        after: &after,
        writers: &p.writers,
        blobs: &blobs_of(&after),
        new_roster: None,
        decisions: &decisions,
        adopted: &[],
    });
    let word = match verdict {
        Decided::Rejected => "rejected",
        Decided::Withdrawn => "withdrawn",
    };
    Made {
        parents: vec![parent],
        after,
        roster: p.roster.clone(),
        edits: Vec::new(),
        dec: vec![rec(vec![("p", int(x)), ("v", Value::Str(word.into()))])],
        verdict: v,
        proposal: false,
        adopts: None,
    }
}

/// The adoption of the proposal `x` on the accepted `parent` (rule 7):
/// `proposal::adoption`, or now and then one that drops a change of it or
/// makes one more.
#[allow(clippy::too_many_arguments, reason = "the history's parts")]
fn adopt(
    rng: &mut Rng,
    n: u64,
    parent: u64,
    p: &Kept,
    x: u64,
    prop: &Proposal,
    at: &Kept,
    raw: &[u8],
) -> Made {
    let who = format!("person {}", prop.signer);
    let when = Stamp {
        utc: i64::try_from(x).unwrap(),
        offset: 0,
    };
    let mut after = proposal::adoption(&at.code, &prop.code, &p.code, &who, when);
    match rng.below(4) {
        0 => {
            let changed: Vec<String> = authority::delta(&p.code, &after).into_iter().collect();
            if !changed.is_empty() {
                let path = &changed[usize::try_from(rng.below(changed.len() as u64)).unwrap()];
                match p.code.get(path) {
                    Some(e) => after.insert(path.clone(), e.clone()),
                    None => after.remove(path),
                };
            }
        }
        1 => write(
            &mut after,
            PATHS[usize::try_from(rng.below(2)).unwrap()].0,
            Some(n),
        ),
        _ => {}
    }
    let adopted = [commit::Adopted {
        verdict: &prop.verdict,
        base: &at.code,
        tree: &prop.code,
    }];
    let verdict = commit::judge(&Commit {
        raw,
        rosters: std::slice::from_ref(&p.roster),
        before: &p.code,
        after: &after,
        writers: &p.writers,
        blobs: &blobs_of(&after),
        new_roster: None,
        decisions: &BTreeMap::new(),
        adopted: &adopted,
    });
    Made {
        parents: vec![parent, x],
        after,
        roster: p.roster.clone(),
        edits: Vec::new(),
        dec: Vec::new(),
        verdict,
        proposal: false,
        adopts: Some(x),
    }
}

/// The events of seed `seed`: each commit, numbered from 2 as the model's
/// `cm` (1 is the genesis), with the code's verdict.
fn run(seed: u64) -> Vec<Event> {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut accepted: BTreeMap<u64, Kept> = BTreeMap::new();
    let mut code = Tree::new();
    code.insert(ROSTER.to_owned(), entry("roster-1".into()));
    accepted.insert(
        1,
        Kept {
            code,
            roster: genesis(),
            anc: BTreeSet::from([1]),
            writers: Writers::new(),
        },
    );
    let mut proposals: BTreeMap<u64, Proposal> = BTreeMap::new();
    let mut out = Vec::new();
    for n in 2..2 + 12 + rng.below(10) {
        let ids: Vec<u64> = accepted.keys().copied().collect();
        let parent = if rng.one_in(3) {
            pick(&mut rng, &ids)
        } else {
            *ids.last().unwrap()
        };
        let p = accepted[&parent].clone();
        let signer = 1 + rng.below(4);
        let force = rng.one_in(4);
        let raw = raw(n, signer, force);
        let kept: Vec<u64> = proposals.keys().copied().collect();
        // pigeond adopts a proposal on a `main` that holds its base, as the
        // model's Adopt does: else the merge would bring accepted commits
        // that `main` lacks besides the proposal.
        let based: Vec<u64> = proposals
            .iter()
            .filter(|(_, x)| p.anc.contains(&x.base))
            .map(|(x, _)| *x)
            .collect();
        let made = match rng.below(5) {
            0 if !kept.is_empty() => {
                let x = pick(&mut rng, &kept);
                let prop = proposals[&x].clone();
                decide(
                    &mut rng,
                    n,
                    parent,
                    &p,
                    x,
                    &prop,
                    &accepted[&prop.base],
                    &raw,
                )
            }
            1 if !based.is_empty() => {
                let x = pick(&mut rng, &based);
                let prop = proposals[&x].clone();
                adopt(
                    &mut rng,
                    n,
                    parent,
                    &p,
                    x,
                    &prop,
                    &accepted[&prop.base],
                    &raw,
                )
            }
            _ => ordinary(&mut rng, n, parent, &p, &raw),
        };
        let acc = match &made.verdict {
            Verdict::Accepted(s, _) => {
                assert_eq!(*s, dev(signer));
                true
            }
            Verdict::Refused(_) => false,
            Verdict::Unknown(why) => panic!("unknown verdict {why} under rules 1"),
        };
        if acc {
            let mut anc = p.anc.clone();
            anc.insert(n);
            // The person it writes as: an adoption's proposal's author.
            let who = match made.adopts {
                Some(x) => {
                    let prop = &proposals[&x];
                    accepted[&prop.base].roster.devices[&dev(prop.signer)]
                        .person
                        .clone()
                }
                None => p.roster.devices[&dev(signer)].person.clone(),
            };
            let mut writers = p.writers.clone();
            for path in authority::delta(&p.code, &made.after) {
                writers
                    .entry(names::fold(names::unit(&path)))
                    .or_default()
                    .insert(who.clone());
            }
            accepted.insert(
                n,
                Kept {
                    code: made.after.clone(),
                    roster: made.roster.clone(),
                    anc,
                    writers,
                },
            );
        } else if made.proposal {
            proposals.insert(
                n,
                Proposal {
                    base: parent,
                    signer,
                    code: made.after.clone(),
                    verdict: made.verdict.clone(),
                },
            );
        }
        let tree = model_tree(&made.after);
        let versions = |q: &str| Value::List(tree[q].iter().map(|v| int(*v)).collect());
        out.push(Event::new(
            Authority::Judge,
            "l1",
            n - 1,
            vec![
                (
                    "parents".to_owned(),
                    Value::List(made.parents.iter().map(|p| int(*p)).collect()),
                ),
                ("cbase".to_owned(), int(parent)),
                ("signer".to_owned(), int(signer)),
                (
                    "tree".to_owned(),
                    rec(vec![("f", versions("f")), ("r", versions("r"))]),
                ),
                ("redits".to_owned(), Value::List(made.edits)),
                ("dec".to_owned(), Value::List(made.dec)),
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
