//! Laws of held mass deletions (07 §7.3 step 3, §7.4; pigeon-core `held`),
//! over random rounds of a few paths with a small limit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeSet;

use common::{flag, index};
use hegel::TestCase;
use pigeon_core::held::{Split, Why, split, top};

const PATHS: [&str; 8] = ["a", "b", "x/a", "x/b", "x/c", "y/a", "y/b", "z/a"];

fn subset(tc: &TestCase) -> BTreeSet<String> {
    PATHS
        .iter()
        .filter(|_| flag(tc))
        .map(|p| (*p).to_owned())
        .collect()
}

struct Round {
    limit: usize,
    deleted: BTreeSet<String>,
    held: BTreeSet<String>,
    released: BTreeSet<String>,
    gone: BTreeSet<String>,
}

fn round(tc: &TestCase) -> Round {
    let gone = ["x", "y", "z"]
        .iter()
        .filter(|_| index(tc, 4) == 0)
        .map(|t| (*t).to_owned())
        .collect();
    Round {
        limit: index(tc, 6),
        deleted: subset(tc),
        held: subset(tc),
        released: subset(tc),
        gone,
    }
}

fn run(r: &Round) -> Split {
    split(r.limit, &r.deleted, &r.held, &r.released, &r.gone)
}

#[hegel::test(test_cases = 1000)]
fn what_goes_and_what_waits_split_the_rounds_deletions(tc: TestCase) {
    let r = round(&tc);
    let s = run(&r);
    assert!(s.delete.is_disjoint(&s.hold));
    assert!(s.delete.is_subset(&r.deleted) && s.hold.is_subset(&r.deleted));
    for p in &r.deleted {
        let held_before = r.held.contains(p) && !r.released.contains(p);
        assert_eq!(
            s.delete.contains(p) || s.hold.contains(p),
            !held_before,
            "{p}"
        );
    }
    assert_eq!(s.why.is_some(), !s.hold.is_empty());
}

#[hegel::test(test_cases = 1000)]
fn nothing_held_goes_unless_released(tc: TestCase) {
    let r = round(&tc);
    let s = run(&r);
    for p in &s.delete {
        assert!(r.released.contains(p) || !r.held.contains(p), "{p}");
    }
    assert!(
        r.released
            .intersection(&r.deleted)
            .all(|p| s.delete.contains(p))
    );
}

#[hegel::test(test_cases = 1000)]
fn what_goes_unasked_stays_within_the_limit_and_no_removed_folder(tc: TestCase) {
    let r = round(&tc);
    let s = run(&r);
    let unasked: BTreeSet<&String> = s.delete.difference(&r.released).collect();
    if !unasked.is_empty() {
        let waiting = r.held.difference(&r.released);
        let with_held: BTreeSet<&String> = unasked.iter().copied().chain(waiting).collect();
        assert!(with_held.len() <= r.limit);
        assert!(
            unasked
                .iter()
                .all(|p| top(p).is_none_or(|t| !r.gone.contains(t)))
        );
    }
    if s.why == Some(Why::Mass) {
        let waiting: BTreeSet<String> = r.held.difference(&r.released).cloned().collect();
        assert!(s.hold.union(&waiting).count() > r.limit);
    }
}

#[hegel::test(test_cases = 1000)]
fn a_larger_round_is_held_whenever_a_smaller_one_is(tc: TestCase) {
    let r = round(&tc);
    let more: BTreeSet<String> = r.deleted.union(&subset(&tc)).cloned().collect();
    let small = run(&r);
    let large = split(r.limit, &more, &r.held, &r.released, &r.gone);
    if small.why.is_some() {
        assert!(large.why.is_some());
        assert!(small.hold.is_subset(&large.hold));
    }
}
