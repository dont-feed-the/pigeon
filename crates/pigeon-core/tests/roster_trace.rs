//! Runs of the roster fold, logged as `Roster` trace events for TLC (ADR
//! 0008, 12 §12.3), and the check that every traced action name is an
//! operator of its specification. With `PIGEON_TRACE_DIR` set, one NDJSON
//! file per seed is written there; `scripts/check-code-traces` hands each to
//! TLC through `models/tla/RosterTrace.tla`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(feature = "tla-trace")]
mod common;

use std::fs;
use std::path::PathBuf;

use common::roster_sim::{Logged, ND, Sim, dnum, pnum};
use pigeon_core::roster::{Edit, Field, Roster, Value as V};
use pigeon_core::trace::{Actions, CaptureInstall, Event, Lamport, Roster as Action, Value};

fn models() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/tla")
}

fn ints(xs: impl IntoIterator<Item = usize>) -> Value {
    Value::List(xs.into_iter().map(|x| Value::Int(x as i64)).collect())
}

fn rec(fields: Vec<(&str, Value)>) -> Value {
    Value::Record(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

/// A roster as the model's record [mem, adm, dev, gone].
fn ros(r: &Roster) -> Value {
    let dev = (1..=ND).map(|i| {
        r.devices
            .iter()
            .find(|(d, _)| dnum(d) == i)
            .map_or(0, |(_, x)| pnum(&x.person))
    });
    rec(vec![
        ("mem", ints(r.persons.keys().map(pnum))),
        (
            "adm",
            ints(
                r.persons
                    .iter()
                    .filter(|(_, p)| p.admin)
                    .map(|(i, _)| pnum(i)),
            ),
        ),
        ("dev", ints(dev)),
        ("gone", ints(r.removed.keys().map(dnum))),
    ])
}

fn edit(e: &Edit) -> Value {
    let s = |x: &str| Value::Str(x.to_string());
    match e {
        Edit::Set {
            field: Field::PersonAdmin(p),
            to: V::Flag(b),
            ..
        } => rec(vec![
            ("k", s("admin")),
            ("p", Value::Int(pnum(p) as i64)),
            ("new", Value::Bool(*b)),
        ]),
        Edit::AddDevice(d, x) => rec(vec![
            ("k", s("add")),
            ("d", Value::Int(dnum(d) as i64)),
            ("p", Value::Int(pnum(&x.person) as i64)),
        ]),
        Edit::RemoveDevice(d) => rec(vec![("k", s("rem")), ("d", Value::Int(dnum(d) as i64))]),
        Edit::RemovePerson(p) => rec(vec![("k", s("remp")), ("p", Value::Int(pnum(p) as i64))]),
        other => panic!("the Roster model has no edit {other:?}"),
    }
}

/// The run's events, in the order they happened. One process simulates
/// every replica, so a step counter is a Lamport clock of the run.
fn events(sim: &Sim) -> Vec<Event> {
    let mut clock = Lamport::default();
    sim.log
        .iter()
        .map(|l| match l {
            Logged::Commit {
                r,
                t,
                op,
                edits,
                ros: x,
            } => Event::new(
                Action::Commit,
                format!("r{}", r + 1),
                clock.tick(),
                vec![
                    ("r".into(), Value::Int(*r as i64 + 1)),
                    ("t".into(), Value::Int(*t)),
                    ("op".into(), Value::Str(op.to_string())),
                    (
                        "edits".into(),
                        Value::List(edits.iter().map(edit).collect()),
                    ),
                    ("ros".into(), ros(x)),
                ],
            ),
            Logged::Merge { r, y, ff, ros: x } => Event::new(
                Action::Merge,
                format!("r{}", r + 1),
                clock.tick(),
                vec![
                    ("r".into(), Value::Int(*r as i64 + 1)),
                    ("y".into(), Value::Int(i64::from(*y))),
                    ("ff".into(), Value::Bool(*ff)),
                    ("ros".into(), ros(x)),
                ],
            ),
        })
        .collect()
}

/// A small deterministic generator, so that a seed names a run.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// The run of seed `seed`: 24 random steps of the simulated group.
pub fn run(seed: u64) -> Sim {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut sim = Sim::new();
    sim.run(24, &mut |n| rng.below(n));
    sim
}

#[test]
fn traces_for_tlc() {
    let seeds: u64 = std::env::var("PIGEON_TRACE_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let dir = std::env::var_os("PIGEON_TRACE_DIR").map(PathBuf::from);
    let (mut merges, mut skips) = (0, 0);
    for seed in 1..=seeds {
        let sim = run(seed);
        merges += sim
            .log
            .iter()
            .filter(|l| matches!(l, Logged::Merge { ff: false, .. }))
            .count();
        skips += sim
            .ids()
            .iter()
            .map(|c| sim.history.fold(c).skipped.len())
            .max()
            .unwrap_or(0);
        let lines: Vec<String> = events(&sim).iter().map(Event::to_ndjson).collect();
        assert!(lines.iter().all(|l| l.is_ascii() && !l.contains('\n')));
        if let Some(d) = &dir {
            fs::create_dir_all(d).unwrap();
            fs::write(
                d.join(format!("roster-{seed:03}.ndjson")),
                lines.join("\n") + "\n",
            )
            .unwrap();
        }
    }
    eprintln!("{seeds} runs: {merges} merge commits, {skips} skipped changes at the heads");
    if seeds >= 20 {
        assert!(
            merges > 0 && skips > 0,
            "the runs exercise merges and skips"
        );
    }
}

fn operators(spec: &str) -> Vec<String> {
    let text = fs::read_to_string(models().join(format!("{spec}.tla"))).unwrap();
    text.lines()
        .filter_map(|l| {
            let name: String = l
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let rest = l[name.len()..].trim_start();
            (!name.is_empty() && (rest.starts_with("==") || rest.starts_with('('))).then_some(name)
        })
        .collect()
}

fn names_match<A: Actions>() {
    let ops = operators(A::SPEC);
    for a in A::ALL {
        assert!(
            ops.iter().any(|o| o == a.name()),
            "{} is not an operator of {}.tla",
            a.name(),
            A::SPEC
        );
    }
}

#[test]
fn action_names_are_operators_of_their_specs() {
    names_match::<Action>();
    names_match::<CaptureInstall>();
    names_match::<pigeon_core::trace::Admission>();
    names_match::<pigeon_core::trace::Authority>();
    names_match::<pigeon_core::trace::Catalogue>();
}
