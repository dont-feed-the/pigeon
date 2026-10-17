//! Stable/hot-path laws (ADR 0037, 15 §15.5), 1000 generated cases
//! each. Only actual distinct captures heat a path; stat changes restart
//! settling, backwards time grants nothing, and the last save becomes due.

use hegel::{TestCase, generators as gs};
use pigeon_core::capture_schedule::{Readiness, Schedule};
use pigeon_core::resources::quiet_ms;
use std::collections::BTreeSet;

fn hot(path: &str) -> (Schedule<u64>, u64) {
    let mut schedule = Schedule::default();
    let mut last = 0;
    for i in 0..7 {
        let now = i * 5_000;
        assert_eq!(schedule.observe(path, i, 0, now), Readiness::Wait(2_000));
        assert_eq!(schedule.observe(path, i, 0, now + 2_000), Readiness::Ready);
        assert!(schedule.captured(path, &i.to_string(), now + 2_000));
        last = now + 2_000;
    }
    (schedule, last)
}

#[hegel::test(test_cases = 1000)]
fn two_equal_observations_span_the_entire_size_dependent_quiet_time(tc: TestCase) {
    let size = tc.draw(gs::integers::<u64>());
    let stamp = tc.draw(gs::integers::<u64>());
    let quiet = quiet_ms(size);
    let mut schedule = Schedule::default();
    assert_eq!(
        schedule.observe("a", stamp, size, 0),
        Readiness::Wait(quiet)
    );
    assert_eq!(
        schedule.observe("a", stamp, size, quiet - 1),
        Readiness::Wait(1)
    );
    assert_eq!(schedule.observe("a", stamp, size, quiet), Readiness::Ready);
}

#[hegel::test(test_cases = 1000)]
fn any_stat_change_restarts_the_whole_quiet_period(tc: TestCase) {
    let size = tc.draw(gs::integers::<u64>());
    let stamp = tc.draw(gs::integers::<u64>());
    let quiet = quiet_ms(size);
    let mut schedule = Schedule::default();
    schedule.observe("a", stamp, size, 0);
    assert_eq!(schedule.observe("a", stamp, size, quiet), Readiness::Ready);
    assert_eq!(
        schedule.observe("a", stamp ^ 1, size, quiet + 1),
        Readiness::Wait(quiet)
    );
    assert_eq!(
        schedule.observe("a", stamp ^ 1, size, 2 * quiet),
        Readiness::Wait(1)
    );
    assert_eq!(
        schedule.observe("a", stamp ^ 1, size, 2 * quiet + 1),
        Readiness::Ready
    );
}

#[hegel::test(test_cases = 1000)]
fn seven_captures_limit_the_next_capture_to_ten_minutes(tc: TestCase) {
    let elapsed = tc.draw(gs::integers::<u64>().max_value(599_999));
    let (mut schedule, last) = hot("a");
    assert_eq!(schedule.observe("a", 99, 0, last), Readiness::Wait(600_000));
    assert_eq!(
        schedule.observe("a", 99, 0, last + elapsed),
        Readiness::Wait(600_000 - elapsed)
    );
    assert_eq!(
        schedule.observe("a", 99, 0, last + 600_000),
        Readiness::Ready
    );
}

#[hegel::test(test_cases = 1000)]
fn the_last_save_eventually_becomes_ready_after_both_limits(tc: TestCase) {
    let size = tc.draw(gs::integers::<u64>());
    let delay = tc.draw(gs::integers::<u64>().max_value(1_000_000));
    let (mut schedule, last) = hot("a");
    let saved = last + delay;
    schedule.observe("a", 99, size, saved);
    let due = (saved + quiet_ms(size)).max(last + 600_000);
    assert_eq!(schedule.observe("a", 99, size, due), Readiness::Ready);
}

#[hegel::test(test_cases = 1000)]
fn a_quiet_hour_cools_the_path_even_if_the_next_observation_is_a_new_save(tc: TestCase) {
    let extra = tc.draw(gs::integers::<u64>().max_value(3_600_000));
    let (mut schedule, last) = hot("a");
    let now = last + 3_600_000 + extra;
    assert_eq!(schedule.observe("a", 99, 0, now), Readiness::Wait(2_000));
    assert_eq!(schedule.observe("a", 99, 0, now + 2_000), Readiness::Ready);
    schedule.captured("a", "cooled", now + 2_000);
    assert_eq!(
        schedule.observe("a", 100, 0, now + 3_000),
        Readiness::Wait(2_000)
    );
    assert_eq!(schedule.observe("a", 100, 0, now + 5_000), Readiness::Ready);
}

#[hegel::test(test_cases = 1000)]
fn one_hot_path_cannot_delay_an_unrelated_path(tc: TestCase) {
    let stamp = tc.draw(gs::integers::<u64>());
    let (mut schedule, last) = hot("a");
    schedule.observe("a", 99, 0, last);
    assert_eq!(
        schedule.observe("b", stamp, 0, last),
        Readiness::Wait(2_000)
    );
    assert_eq!(
        schedule.observe("b", stamp, 0, last + 2_000),
        Readiness::Ready
    );
    assert_eq!(
        schedule.observe("a", 99, 0, last + 2_000),
        Readiness::Wait(598_000)
    );
}

#[hegel::test(test_cases = 1000)]
fn repeated_observations_and_duplicate_capture_notifications_do_not_heat(tc: TestCase) {
    let repeats = tc.draw(gs::integers::<u64>().min_value(7).max_value(100));
    let mut schedule = Schedule::default();
    schedule.observe("a", 0, 0, 0);
    assert_eq!(schedule.observe("a", 0, 0, 2_000), Readiness::Ready);
    assert!(schedule.captured("a", "v1", 2_000));
    for i in 0..repeats {
        schedule.observe("a", 0, 0, 3_000 + i);
        assert!(!schedule.captured("a", "v1", 3_000 + i));
    }
    assert_eq!(schedule.observe("a", 1, 0, 4_000), Readiness::Wait(2_000));
    assert_eq!(schedule.observe("a", 1, 0, 6_000), Readiness::Ready);
}

#[hegel::test(test_cases = 1000)]
fn a_backwards_clock_cannot_spend_the_remaining_quiet_time(tc: TestCase) {
    let elapsed = tc.draw(gs::integers::<u64>().max_value(1_999));
    let earlier = tc.draw(gs::integers::<u64>()).min(elapsed);
    let mut schedule = Schedule::default();
    schedule.observe("a", 0, 0, 0);
    let waiting = schedule.observe("a", 0, 0, elapsed);
    assert_eq!(waiting, Readiness::Wait(2_000 - elapsed));
    assert_eq!(schedule.observe("a", 0, 0, earlier), waiting);
}

#[hegel::test(test_cases = 1000)]
fn inactive_fingerprints_are_bounded_to_the_recent_hour(tc: TestCase) {
    let now = tc.draw(gs::integers::<u64>().max_value(1_000_000));
    let mut schedule = Schedule::default();
    schedule.observe("a", 0, 0, now);
    schedule.observe("b", 0, 0, now);
    let active = BTreeSet::from(["a".to_owned()]);
    schedule.prune(&active, now + 3_599_999);
    assert_eq!(schedule.tracked_paths(), 2);
    schedule.prune(&active, now + 3_600_000);
    assert_eq!(schedule.tracked_paths(), 1);
    assert_eq!(
        schedule.observe("b", 0, 0, now + 3_600_000),
        Readiness::Wait(2_000)
    );
}
