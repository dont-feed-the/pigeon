//! Event-hint scan laws (ADR 0013), 1000 cases each. Events admit a full
//! observation; losing them cannot suppress the periodic observation.
//! An observation cannot acknowledge events that arrived after it began.

use hegel::{TestCase, generators as gs};
use pigeon_core::change_scan::{PERIOD_MS, Schedule};

#[hegel::test(test_cases = 1000)]
fn startup_observes_even_without_an_event(tc: TestCase) {
    let now = tc.draw(gs::integers::<u64>());
    let revision = tc.draw(gs::integers::<u64>());
    assert!(Schedule::default().due(now, revision, true, false));
}

#[hegel::test(test_cases = 1000)]
fn lost_or_duplicate_hints_cannot_suppress_the_periodic_full_observation(tc: TestCase) {
    let now = tc.draw(gs::integers::<u64>().max_value(u64::MAX - PERIOD_MS));
    let before = tc.draw(gs::integers::<u64>());
    let after = tc.draw(gs::integers::<u64>());
    let mut schedule = Schedule::default();
    schedule.completed(now, before);
    for revision in [before, after, before, 0, u64::MAX] {
        assert!(schedule.due(now + PERIOD_MS, revision, true, false));
    }
}

#[hegel::test(test_cases = 1000)]
fn completion_acknowledges_only_the_starting_revision(tc: TestCase) {
    let started = tc.draw(gs::integers::<u64>().max_value(1_000_000));
    let before = tc.draw(gs::integers::<u64>().max_value(u64::MAX - 2));
    let mut schedule = Schedule::default();
    schedule.completed(started, before);
    assert!(!schedule.due(started, before, true, false));
    assert!(schedule.due(started, before + 1, true, false));
    schedule.completed(started + 1, before + 1);
    assert!(schedule.due(started + 1, before + 2, true, false));
}

#[hegel::test(test_cases = 1000)]
fn quiet_native_watching_avoids_round_polling_but_not_settling_or_fallback(tc: TestCase) {
    let now = tc.draw(gs::integers::<u64>().max_value(1_000_000));
    let elapsed = tc.draw(gs::integers::<u64>().max_value(PERIOD_MS - 1));
    let revision = tc.draw(gs::integers::<u64>().max_value(u64::MAX - 1));
    let mut schedule = Schedule::default();
    schedule.completed(now, revision);
    assert!(!schedule.due(now + elapsed, revision, true, false));
    assert!(schedule.due(now + elapsed, revision, false, false));
    assert!(schedule.due(now + elapsed, revision, true, true));
}

#[hegel::test(test_cases = 1000)]
fn saturation_and_failed_observations_never_grant_a_quiet_cache(tc: TestCase) {
    let now = tc.draw(gs::integers::<u64>().max_value(1_000_000));
    let mut schedule = Schedule::default();
    schedule.completed(now, u64::MAX);
    assert!(schedule.due(now, u64::MAX, true, false));
    let before_failure = schedule.clone();
    assert!(schedule.due(now + 1, u64::MAX, true, false));
    assert_eq!(schedule, before_failure);
}

#[hegel::test(test_cases = 1000)]
fn backwards_completion_cannot_create_past_due_credit(tc: TestCase) {
    let now = tc.draw(
        gs::integers::<u64>()
            .min_value(PERIOD_MS)
            .max_value(1_000_000),
    );
    let backward = tc.draw(gs::integers::<u64>().max_value(now));
    let mut schedule = Schedule::default();
    schedule.completed(now, 0);
    schedule.completed(backward, 0);
    assert!(!schedule.due(now, 0, true, false));
    assert!(schedule.due(now + PERIOD_MS, 0, true, false));
}

fn deletions(tc: &TestCase, prefix: &str) -> std::collections::BTreeSet<String> {
    let bits = tc.draw(gs::integers::<u32>());
    let salt = tc.draw(gs::integers::<u16>());
    (0..32)
        .filter(|n| bits & (1 << n) != 0)
        .map(|n| format!("{prefix}/{salt}-{n}"))
        .collect()
}

#[hegel::test(test_cases = 1000)]
fn repeated_deletion_observations_cannot_create_a_self_waking_loop(tc: TestCase) {
    let paths = deletions(&tc, "a");
    let mut hints = pigeon_core::change_scan::Deletions::default();
    assert_eq!(hints.observe("", &paths), !paths.is_empty());
    for _ in 0..tc.draw(gs::integers::<u8>().max_value(32)) {
        assert!(!hints.observe("", &paths));
    }
}

#[hegel::test(test_cases = 1000)]
fn disjoint_folder_observations_commute_and_keep_each_others_requests(tc: TestCase) {
    let a = deletions(&tc, "a");
    let b = deletions(&tc, "b");
    let mut ab = pigeon_core::change_scan::Deletions::default();
    let mut ba = ab.clone();
    assert_eq!(ab.observe("a", &a), !a.is_empty());
    assert_eq!(ab.observe("b", &b), !b.is_empty());
    assert_eq!(ba.observe("b", &b), !b.is_empty());
    assert_eq!(ba.observe("a", &a), !a.is_empty());
    assert_eq!(ab, ba);
    assert!(!ab.observe("a", &a));
    assert!(!ab.observe("b", &b));
    assert!(!ab.observe("a", &b)); // Out-of-scope paths never grant a wake.
    assert!(!ab.observe("b", &b));
}

#[hegel::test(test_cases = 1000)]
fn a_clean_folder_rearms_its_own_deletions_without_rearming_other_views(tc: TestCase) {
    let a = deletions(&tc, "a");
    let b = deletions(&tc, "b");
    let mut hints = pigeon_core::change_scan::Deletions::default();
    let all = a.union(&b).cloned().collect();
    hints.observe("", &all);
    assert!(!hints.observe("a", &Default::default()));
    assert!(!hints.observe("b", &b));
    assert_eq!(hints.observe("a", &a), !a.is_empty());
}

#[hegel::test(test_cases = 1000)]
fn a_completed_deletion_rearms_an_identical_later_deletion(tc: TestCase) {
    let paths = deletions(&tc, "a");
    let mut hints = pigeon_core::change_scan::Deletions::default();
    hints.observe("", &paths);
    hints.consumed();
    assert_eq!(hints.observe("", &paths), !paths.is_empty());
    assert!(!hints.observe("", &paths));
}
