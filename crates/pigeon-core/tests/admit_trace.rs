//! Plans of `capacity::admit` on random instances, logged as `Admission`
//! trace events for TLC (08 §8.9, 13 §13.3). With `PIGEON_TRACE_DIR` set,
//! one NDJSON file per seed is written there; `scripts/check-code-traces`
//! hands each to TLC through `models/tla/AdmitTrace.tla`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(feature = "tla-trace")]

use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

use pigeon_core::capacity::admit;
use pigeon_core::queue::{Work, compare};
use pigeon_core::time::Time;
use pigeon_core::trace::{Admission, Event, Value};

/// A small deterministic generator, so that a seed names a run.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

fn int(n: u64) -> Value {
    Value::Int(i64::try_from(n).unwrap())
}

fn rec(fields: Vec<(&str, Value)>) -> Vec<(String, Value)> {
    fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

/// The instance of seed `seed` and the events of its plan: keys `k1`… are
/// the model's keys 1…, each logged decision in the code's order.
fn run(seed: u64) -> Vec<Event> {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let n = 1 + rng.below(6);
    let items: Vec<Work> = (1..=n)
        .map(|i| Work::Protect {
            key: format!("k{i}"),
            custody_copies: u32::try_from(rng.below(3)).unwrap(),
            laptop_copies: u32::try_from(rng.below(2)).unwrap(),
            endangered: rng.below(3) == 0,
            introduced: Time(i64::try_from(rng.below(4)).unwrap()),
            size: rng.below(8),
        })
        .collect();
    let room = rng.below(24);
    let index = |w: &Work| match w {
        Work::Protect { key, .. } => key[1..].parse::<u64>().unwrap(),
        _ => unreachable!(),
    };
    let (sizes, endangered): (Vec<Value>, Vec<Value>) = (
        items
            .iter()
            .map(|w| match w {
                Work::Protect { size, .. } => int(*size),
                _ => unreachable!(),
            })
            .collect(),
        items
            .iter()
            .filter(|w| {
                matches!(
                    w,
                    Work::Protect {
                        endangered: true,
                        ..
                    }
                )
            })
            .map(|w| int(index(w)))
            .collect(),
    );
    let mut clock = 0;
    let mut tick = || {
        clock += 1;
        clock
    };
    let mut out = vec![Event::new(
        Admission::Init,
        "c1",
        tick(),
        rec(vec![
            ("room", int(room)),
            ("size", Value::List(sizes)),
            ("endangered", Value::List(endangered)),
        ]),
    )];
    let plan = admit(room, &items);
    let (mut a, mut d) = (
        plan.admitted.iter().peekable(),
        plan.deferred.iter().peekable(),
    );
    loop {
        let (action, w) = match (a.peek(), d.peek()) {
            (Some(x), Some(y)) if compare(x, &y.work) != Ordering::Greater => {
                (Admission::Admit, a.next().unwrap())
            }
            (_, Some(_)) => (Admission::Defer, &d.next().unwrap().work),
            (Some(_), None) => (Admission::Admit, a.next().unwrap()),
            (None, None) => break,
        };
        out.push(Event::new(
            action,
            "c1",
            tick(),
            rec(vec![("k", int(index(w)))]),
        ));
    }
    out.push(Event::new(
        Admission::Done,
        "c1",
        tick(),
        rec(vec![("n", int(n))]),
    ));
    out
}

#[test]
fn traces_for_tlc() {
    let seeds: u64 = std::env::var("PIGEON_TRACE_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let dir = std::env::var_os("PIGEON_TRACE_DIR").map(PathBuf::from);
    let (mut admitted, mut deferred) = (0, 0);
    for seed in 1..=seeds {
        let events = run(seed);
        admitted += events.iter().filter(|e| e.action == "Admit").count();
        deferred += events.iter().filter(|e| e.action == "Defer").count();
        let lines: Vec<String> = events.iter().map(Event::to_ndjson).collect();
        if let Some(d) = &dir {
            fs::create_dir_all(d).unwrap();
            fs::write(
                d.join(format!("admit-{seed:03}.ndjson")),
                lines.join("\n") + "\n",
            )
            .unwrap();
        }
    }
    eprintln!("{seeds} plans: {admitted} keys admitted, {deferred} deferred");
    if seeds >= 20 {
        assert!(admitted > 0 && deferred > 0, "the plans admit and defer");
    }
}
