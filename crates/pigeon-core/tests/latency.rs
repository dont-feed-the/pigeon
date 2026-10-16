//! Diagnostic milestone laws (12 §12.2), including full clock ranges,
//! duplicate suppression, independent phases and a hard memory bound.
#![allow(clippy::unwrap_used, reason = "property-test assertions")]
use hegel::{TestCase, generators as gs};
use pigeon_core::latency::{Ledger, Milestone::*, Sample};

#[hegel::test(test_cases = 1000)]
fn a_milestone_is_measured_once_and_clock_skew_is_never_zero(tc: TestCase) {
    let captured = tc.draw(gs::integers::<i64>());
    let now = tc.draw(gs::integers::<i64>());
    let mut ledger = Ledger::default();
    let sample = ledger.observe(1, 7, Accepted, captured, now).unwrap();
    assert_eq!(
        sample,
        if now < captured {
            Sample::ClockSkew
        } else {
            Sample::Seconds((i128::from(now) - i128::from(captured)) as u64)
        }
    );
    assert_eq!(ledger.observe(1, 7, Accepted, captured, now), None);
}

#[hegel::test(test_cases = 1000)]
fn acceptance_and_protection_are_independent(tc: TestCase) {
    let reverse = tc.draw(gs::booleans());
    let (first, second) = if reverse {
        (Protected, Accepted)
    } else {
        (Accepted, Protected)
    };
    let mut ledger = Ledger::default();
    assert!(ledger.observe(1, 3, first, 10, 12).is_some());
    assert!(ledger.observe(1, 3, second, 10, 15).is_some());
    assert_eq!(ledger.len(), 1);
    assert!(ledger.observe(1, 3, first, 10, 20).is_none());
    assert!(ledger.observe(1, 3, second, 10, 20).is_none());
}

#[hegel::test(test_cases = 1000)]
fn capacity_never_evicts_or_remeasures_an_existing_identity(tc: TestCase) {
    let capacity = tc.draw(gs::integers::<usize>().max_value(30));
    let mut ledger = Ledger::default();
    for id in 0..60 {
        assert_eq!(
            ledger.observe(capacity, id, Accepted, 0, 1).is_some(),
            id < capacity
        );
        assert!(ledger.len() <= capacity);
    }
    for id in 0..capacity {
        assert!(ledger.observe(capacity, id, Accepted, 0, 2).is_none());
        assert!(ledger.observe(capacity, id, Protected, 0, 2).is_some());
    }
}

#[hegel::test(test_cases = 1000)]
fn versions_never_suppress_other_versions(tc: TestCase) {
    let a = tc.draw(gs::integers::<u32>());
    let b = a.wrapping_add(1);
    let mut ledger = Ledger::default();
    assert_eq!(
        ledger.observe(2, a, Accepted, 1, 3),
        Some(Sample::Seconds(2))
    );
    assert_eq!(
        ledger.observe(2, b, Accepted, 1, 3),
        Some(Sample::Seconds(2))
    );
}
