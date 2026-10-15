//! Laws of notifications (10 §10.4, ADR 0029; pigeon-core `notify`), over
//! random runs of situations at increasing times.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::{flag, int};
use hegel::TestCase;
use hegel::generators as gs;
use pigeon_core::notify::{KINDS, Kind, Memory, RATE, Situation, step};
use pigeon_core::time::Time;

/// Absent, or one of few fingerprints, so that they repeat.
fn kind(tc: &TestCase) -> Option<u64> {
    flag(tc).then(|| tc.draw(gs::integers::<u64>().max_value(2)))
}

fn situation(tc: &TestCase) -> Situation {
    Situation {
        needs_you: kind(tc),
        not_safe_long: kind(tc),
        waiting: kind(tc),
    }
}

/// One step of a run: the time, the situation, whether enabled, and what
/// was shown.
struct Step {
    at: Time,
    situation: Situation,
    enabled: bool,
    shown: Vec<Kind>,
}

/// A run of up to 30 steps, up to 2 days apart.
fn run(tc: &TestCase) -> Vec<Step> {
    let mut memory = Memory::default();
    let mut now = Time(int(tc, 0, 1 << 40));
    let mut steps = Vec::new();
    for _ in 0..tc.draw(gs::integers::<u8>().max_value(30)) {
        now = Time(now.0 + int(tc, 0, 2 * RATE.0));
        let s = situation(tc);
        let enabled = tc.draw(gs::integers::<u8>().max_value(9)) > 0;
        let (next, shown) = step(memory, &s, now, enabled);
        memory = next;
        steps.push(Step {
            at: now,
            situation: s,
            enabled,
            shown,
        });
    }
    steps
}

#[hegel::test(test_cases = 1000)]
fn nothing_when_off_nor_for_an_absent_kind(tc: TestCase) {
    for s in run(&tc) {
        assert!(s.enabled || s.shown.is_empty());
        for k in &s.shown {
            assert!(s.situation.get(*k).is_some(), "never for success");
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn at_most_one_per_kind_and_step(tc: TestCase) {
    for s in run(&tc) {
        for k in KINDS {
            assert!(s.shown.iter().filter(|x| **x == k).count() <= 1);
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn two_of_a_kind_are_a_day_apart(tc: TestCase) {
    let steps = run(&tc);
    for k in KINDS {
        let times: Vec<Time> = steps
            .iter()
            .filter(|s| s.shown.contains(&k))
            .map(|s| s.at)
            .collect();
        for w in times.windows(2) {
            assert!(w[1].since(w[0]) >= RATE, "{k:?}: {w:?}");
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn repeated_only_if_the_situation_changed(tc: TestCase) {
    let steps = run(&tc);
    for k in KINDS {
        // The fingerprint last shown, forgotten when the kind is absent.
        let mut last: Option<u64> = None;
        for s in &steps {
            let now = s.situation.get(k);
            if s.shown.contains(&k) {
                assert_ne!(now, last, "{k:?} shown twice for the same items");
                last = now;
            } else if now.is_none() {
                last = None;
            }
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn a_change_is_told_once_the_day_has_passed(tc: TestCase) {
    let steps = run(&tc);
    for k in KINDS {
        let mut last: Option<(Time, Option<u64>)> = None;
        for s in &steps {
            let now = s.situation.get(k);
            let rested = last.is_none_or(|(at, _)| s.at.since(at) >= RATE);
            let changed = last.is_none_or(|(_, fp)| fp != now);
            if s.enabled && now.is_some() && rested && changed {
                assert!(s.shown.contains(&k), "{k:?} not told at {:?}", s.at);
            }
            if s.shown.contains(&k) {
                last = Some((s.at, now));
            } else if now.is_none()
                && let Some((_, fp)) = &mut last
            {
                *fp = None;
            }
        }
    }
}
