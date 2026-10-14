//! Laws of version states (06 §6.2, ADRs 0002 and 0046; pigeon-core `state`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeSet;

use common::{flag, index, int};
use hegel::TestCase;
use pigeon_core::state::{Custodian, Settings, State, Version, rank, state};
use pigeon_core::time::{Span, Time};

const NOW: Time = Time(10_000_000);

fn heard(tc: &TestCase) -> Option<Time> {
    match int(tc, 0, 3) {
        0 => None,
        1 => Some(Time(NOW.0 - int(tc, 0, 200_000))),
        _ => Some(Time(NOW.0 - int(tc, 0, 3_000))),
    }
}

fn custodian(tc: &TestCase, device: u8) -> Custodian<u8> {
    Custodian {
        device,
        heard: heard(tc),
        holds: int(tc, 0, 3) != 0,
        has_commit: int(tc, 0, 3) != 0,
        checking: int(tc, 0, 4) == 0,
        repository_lost: int(tc, 0, 4) == 0,
    }
}

/// A version whose custodians have distinct devices.
fn version(tc: &TestCase) -> Version<u8> {
    Version {
        custodians: (0..int(tc, 0, 5) as u8).map(|i| custodian(tc, i)).collect(),
        other_device_has_commit: flag(tc),
        this_device_holds: flag(tc),
        erased: flag(tc),
    }
}

fn settings(tc: &TestCase) -> Settings {
    Settings {
        k: int(tc, 1, 3) as usize,
        ..Settings::default()
    }
}

#[hegel::test(test_cases = 2000)]
fn more_presence_never_lowers_the_state(tc: TestCase) {
    let v = version(&tc);
    let s = settings(&tc);
    let before = state(&v, &s, NOW);
    let mut more = v.clone();
    for _ in 0..int(&tc, 1, 4) {
        match int(&tc, 0, 4) {
            0 => more.other_device_has_commit = true,
            1 => more.this_device_holds = true,
            _ if more.custodians.is_empty() => {}
            k => {
                let i = index(&tc, more.custodians.len());
                let c = &mut more.custodians[i];
                match k {
                    2 => c.holds = true,
                    3 => c.has_commit = true,
                    _ => c.heard = c.heard.max(Some(Time(NOW.0 - int(&tc, 0, 200_000)))),
                }
            }
        }
    }
    let after = state(&more, &s, NOW);
    assert!(rank(&after) >= rank(&before), "{before:?} -> {after:?}");
}

/// Each custodian counts once, by its device: a second, identical entry for
/// a device changes nothing.
#[hegel::test(test_cases = 2000)]
fn a_second_entry_for_one_device_changes_nothing(tc: TestCase) {
    let v = version(&tc);
    tc.assume(!v.custodians.is_empty());
    let s = settings(&tc);
    let i = index(&tc, v.custodians.len());
    let mut twice = v.clone();
    twice.custodians.push(v.custodians[i].clone());
    assert_eq!(state(&v, &s, NOW), state(&twice, &s, NOW));
}

#[hegel::test(test_cases = 1000)]
fn custodian_order_does_not_matter(tc: TestCase) {
    let mut v = version(&tc);
    if !v.custodians.is_empty() && flag(&tc) {
        let i = index(&tc, v.custodians.len());
        let again = custodian(&tc, v.custodians[i].device);
        v.custodians.push(again);
    }
    let s = settings(&tc);
    let mut reversed = v.clone();
    reversed.custodians.reverse();
    assert_eq!(state(&v, &s, NOW), state(&reversed, &s, NOW));
}

/// Protected exactly when K distinct custodians count and the group was
/// heard of within `stale_after` (ADRs 0002 and 0046).
#[hegel::test(test_cases = 2000)]
fn protected_when_k_distinct_custodians_count(tc: TestCase) {
    let mut v = version(&tc);
    for _ in 0..int(&tc, 0, 2) {
        if v.custodians.is_empty() {
            break;
        }
        let i = index(&tc, v.custodians.len());
        let again = custodian(&tc, v.custodians[i].device);
        v.custodians.push(again);
    }
    let s = settings(&tc);
    let h: BTreeSet<u8> = v
        .custodians
        .iter()
        .filter(|c| c.holds && c.has_commit && !c.checking && !c.repository_lost)
        .filter(|c| c.heard.is_some_and(|t| t.within(s.offline_after, NOW)))
        .map(|c| c.device)
        .collect();
    let reachable = v
        .custodians
        .iter()
        .any(|c| c.heard.is_some_and(|t| t.within(s.stale_after, NOW)));
    let got = state(&v, &s, NOW);
    match got {
        State::Protected(_) => assert!(h.len() >= s.k && reachable),
        State::Uncertain(_) => assert!(h.len() >= s.k && !reachable),
        _ => assert!(h.len() < s.k, "{got:?} with |H| = {}", h.len()),
    }
}

#[hegel::test(test_cases = 1000)]
fn protected_is_observed_within_offline_after(tc: TestCase) {
    let v = version(&tc);
    let s = settings(&tc);
    if let State::Protected(at) = state(&v, &s, NOW) {
        assert!(at.within(s.offline_after, NOW) && at <= NOW);
        assert!(NOW.since(at) >= Span(0));
    }
}

#[hegel::test(test_cases = 3000)]
fn a_new_holder_never_lowers_the_state(tc: TestCase) {
    let n = int(&tc, 1, 5) as u8;
    let custodians: Vec<Custodian<u8>> = (0..n)
        .map(|i| Custodian {
            device: i,
            heard: Some(NOW),
            holds: i + 1 < n,
            has_commit: true,
            checking: false,
            repository_lost: false,
        })
        .collect();
    let v = Version {
        custodians,
        other_device_has_commit: true,
        this_device_holds: true,
        erased: false,
    };
    let s = settings(&tc);
    let mut more = v.clone();
    if let Some(last) = more.custodians.last_mut() {
        last.holds = true;
    }
    assert!(rank(&state(&more, &s, NOW)) >= rank(&state(&v, &s, NOW)));
}
