//! Laws of version states (06 §6.2, ADRs 0002, 0049; pigeon-core `state`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::{flag, index, int, label};
use hegel::TestCase;
use pigeon_core::domain::Facts;
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

fn custodian(tc: &TestCase, i: u8, shared: bool) -> Custodian<u8> {
    Custodian {
        facts: Facts {
            device: i,
            fingerprints: if shared {
                (0..int(tc, 0, 2))
                    .map(|_| format!("f{}", int(tc, 0, 3)))
                    .collect()
            } else {
                [format!("own{i}")].into()
            },
            host: if shared { label(tc, "h", 3) } else { None },
            domain: if shared { label(tc, "x", 2) } else { None },
        },
        site: if shared { label(tc, "s", 2) } else { None },
        heard: heard(tc),
        holds: int(tc, 0, 3) != 0,
        has_commit: int(tc, 0, 3) != 0,
        checking: int(tc, 0, 4) == 0,
        repository_lost: int(tc, 0, 4) == 0,
    }
}

fn version(tc: &TestCase, shared: bool) -> Version<u8> {
    Version {
        custodians: (0..int(tc, 0, 5) as u8)
            .map(|i| custodian(tc, i, shared))
            .collect(),
        other_device_has_commit: flag(tc),
        this_device_holds: flag(tc),
        erased: flag(tc),
    }
}

fn settings(tc: &TestCase) -> Settings {
    Settings {
        k: int(tc, 1, 3) as usize,
        min_sites: int(tc, 1, 2) as usize,
        ..Settings::default()
    }
}

#[hegel::test(test_cases = 2000)]
fn more_presence_never_lowers_the_state(tc: TestCase) {
    let v = version(&tc, true);
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

#[hegel::test(test_cases = 2000)]
fn a_new_fact_never_raises_the_state(tc: TestCase) {
    let v = version(&tc, true);
    tc.assume(!v.custodians.is_empty());
    let s = settings(&tc);
    let before = state(&v, &s, NOW);
    let mut more = v.clone();
    let i = index(&tc, more.custodians.len());
    more.custodians[i]
        .facts
        .fingerprints
        .insert(format!("f{}", int(&tc, 0, 3)));
    let after = state(&more, &s, NOW);
    assert!(rank(&after) <= rank(&before), "{before:?} -> {after:?}");
}

#[hegel::test(test_cases = 1000)]
fn custodian_order_does_not_matter(tc: TestCase) {
    let v = version(&tc, true);
    let s = settings(&tc);
    let mut reversed = v.clone();
    reversed.custodians.reverse();
    assert_eq!(state(&v, &s, NOW), state(&reversed, &s, NOW));
}

#[hegel::test(test_cases = 2000)]
fn one_custodian_per_domain_is_adr_0002(tc: TestCase) {
    let v = version(&tc, false);
    let s = Settings {
        k: int(&tc, 1, 3) as usize,
        ..Settings::default()
    };
    let h = v
        .custodians
        .iter()
        .filter(|c| c.holds && c.has_commit && !c.checking && !c.repository_lost)
        .filter(|c| c.heard.is_some_and(|t| t.within(s.offline_after, NOW)))
        .count();
    let reachable = v
        .custodians
        .iter()
        .any(|c| c.heard.is_some_and(|t| t.within(s.stale_after, NOW)));
    let got = state(&v, &s, NOW);
    match got {
        State::Protected(_) => assert!(h >= s.k && reachable),
        State::Uncertain(_) => assert!(h >= s.k && !reachable),
        _ => assert!(h < s.k, "{got:?} with |H| = {h}"),
    }
}

#[hegel::test(test_cases = 1000)]
fn protected_is_observed_within_offline_after(tc: TestCase) {
    let v = version(&tc, true);
    let s = settings(&tc);
    if let State::Protected(at) = state(&v, &s, NOW) {
        assert!(at.within(s.offline_after, NOW) && at <= NOW);
        assert!(NOW.since(at) >= Span(0));
    }
}

/// A custodian sharing hardware with two others joins their domains whether
/// or not it holds the key: its new copy must not lower the state (ADR 0064).
#[test]
fn a_bridging_custodian_does_not_merge_domains_by_holding() {
    let c = |i: u8, fps: &[&str], holds: bool| Custodian {
        facts: Facts {
            device: i,
            fingerprints: fps.iter().map(|f| f.to_string()).collect(),
            host: None,
            domain: None,
        },
        site: None,
        heard: Some(NOW),
        holds,
        has_commit: true,
        checking: false,
        repository_lost: false,
    };
    let s = Settings::default();
    let v = |bridge_holds| Version {
        custodians: vec![
            c(0, &["f0"], true),
            c(1, &["f1"], true),
            c(2, &["f0", "f1"], bridge_holds),
        ],
        other_device_has_commit: true,
        this_device_holds: true,
        erased: false,
    };
    assert_eq!(
        rank(&state(&v(false), &s, NOW)),
        rank(&state(&v(true), &s, NOW))
    );
    assert!(matches!(state(&v(true), &s, NOW), State::Published(_)));
}

#[hegel::test(test_cases = 3000)]
fn a_new_holder_never_lowers_the_state(tc: TestCase) {
    let n = int(&tc, 1, 5) as u8;
    let custodians: Vec<Custodian<u8>> = (0..n)
        .map(|i| Custodian {
            facts: Facts {
                device: i,
                fingerprints: (0..int(&tc, 0, 2))
                    .map(|_| format!("f{}", int(&tc, 0, 3)))
                    .collect(),
                host: None,
                domain: None,
            },
            site: None,
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
    let s = Settings {
        k: int(&tc, 1, 3) as usize,
        ..Settings::default()
    };
    let mut more = v.clone();
    if let Some(last) = more.custodians.last_mut() {
        last.holds = true;
    }
    assert!(rank(&state(&more, &s, NOW)) >= rank(&state(&v, &s, NOW)));
}
