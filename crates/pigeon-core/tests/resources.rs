//! Resource controller laws (ADR 0037, 15 §15.3–15.6), 1000 generated
//! cases each. Arbitrary clock/rate ranges cannot bypass explicit ceilings,
//! lose capture's floor, accumulate unbounded bursts or invent past credit.

use hegel::{TestCase, generators as gs};
use pigeon_core::resources::{
    self, ActiveClock, Bucket, Conditions, Delay, Disk, Permit, Preset, Wait,
};

#[hegel::test(test_cases = 1000)]
fn propagation_on_different_paths_cannot_manufacture_queue_delay(tc: TestCase) {
    let near = tc.draw(gs::integers::<u64>().max_value(100));
    let far = tc.draw(gs::integers::<u64>().min_value(101).max_value(10_000));
    let queue = tc.draw(gs::integers::<u64>().max_value(10_000));
    let mut lan = Delay::default();
    let mut wan = Delay::default();
    assert_eq!(lan.observe(0, near), 0);
    assert_eq!(wan.observe(200, far), 0);
    assert_eq!(lan.observe(400, near), 0);
    assert_eq!(wan.observe(400, far + queue), queue);
    assert_eq!(lan.observe(600, near), 0);
}

#[hegel::test(test_cases = 1000)]
fn base_delay_expires_after_ten_minutes_including_the_boundary(tc: TestCase) {
    let base = tc.draw(gs::integers::<u64>().max_value(100_000));
    let queue = tc.draw(gs::integers::<u64>().min_value(1).max_value(100_000));
    let mut delay = Delay::default();
    assert_eq!(delay.observe(0, base), 0);
    assert_eq!(delay.observe(600_000, base + queue), queue);
    assert_eq!(delay.observe(600_001, base + queue), 0);
}

#[hegel::test(test_cases = 1000)]
fn path_delay_matches_the_minimum_of_actual_recent_observations(tc: TestCase) {
    let readings = tc.draw(gs::vecs(gs::integers::<u64>()).max_size(80));
    let step = tc.draw(gs::integers::<u64>().max_value(700_000));
    let mut delay = Delay::default();
    let mut history = Vec::new();
    for (i, rtt) in readings.into_iter().enumerate() {
        let now = i as u64 * step;
        history.push((now, rtt));
        let minimum = history
            .iter()
            .filter(|(at, _)| now - at <= 600_000)
            .map(|(_, value)| *value)
            .min()
            .unwrap_or(rtt);
        assert_eq!(delay.observe(now, rtt), rtt - minimum);
    }
}

#[hegel::test(test_cases = 1000)]
fn quiet_time_is_monotone_and_bounded_for_every_file_size(tc: TestCase) {
    let a = tc.draw(gs::integers::<u64>());
    let b = tc.draw(gs::integers::<u64>());
    assert!((2_000..=60_000).contains(&resources::quiet_ms(a)));
    assert!(resources::quiet_ms(a.min(b)) <= resources::quiet_ms(a.max(b)));
    assert_eq!(resources::quiet_ms(0), 2_000);
    assert_eq!(resources::quiet_ms(100_000_000), 3_000);
}

#[hegel::test(test_cases = 1000)]
fn unknown_conditions_do_not_pause_and_explicit_policy_wins(tc: TestCase) {
    let threshold = tc.draw(gs::integers::<u8>().max_value(100));
    let battery = tc.draw(gs::integers::<u8>().max_value(100));
    let metered = tc.draw(gs::booleans());
    let low_power = tc.draw(gs::booleans());
    assert_eq!(
        resources::external_wait(Conditions::default(), threshold, true),
        None
    );
    let conditions = Conditions {
        battery: Some(battery),
        low_power,
        metered,
    };
    assert_eq!(resources::external_wait(conditions, 0, false), None);
    let wait = resources::external_wait(conditions, threshold, true);
    if threshold > 0 && (battery < threshold || low_power) {
        assert_eq!(wait, Some(Wait::Battery));
    } else {
        assert_eq!(wait, metered.then_some(Wait::Metered));
    }
}

#[hegel::test(test_cases = 1000)]
fn contention_never_starves_capture_and_holds_lower_classes(tc: TestCase) {
    let time = tc.draw(gs::integers::<u64>());
    let mut disk = Disk::default();
    disk.observe(time, true);
    for preset in [Preset::Gentle, Preset::Balanced, Preset::Fast] {
        assert_eq!(disk.share(time, preset, true), preset.capture_floor());
        assert_eq!(disk.share(time, preset, false), 0);
    }
}

#[hegel::test(test_cases = 1000)]
fn quiet_recovery_is_gradual_and_new_contention_resets_it(tc: TestCase) {
    let elapsed = tc.draw(gs::integers::<u64>().max_value(60_000));
    let mut disk = Disk::default();
    disk.observe(1, true);
    disk.observe(2, false);
    let share = disk.share(2 + elapsed, Preset::Gentle, false);
    assert!(share <= 100);
    assert_eq!(share == 0, elapsed < 5_300);
    assert_eq!(share == 100, elapsed >= 35_000);
    assert!(share <= disk.share(2 + elapsed + 1, Preset::Gentle, false));
    disk.observe(2 + elapsed, true);
    assert_eq!(disk.share(2 + elapsed, Preset::Gentle, false), 0);
}

#[hegel::test(test_cases = 1000)]
fn every_rate_update_respects_the_explicit_ceiling(tc: TestCase) {
    let old = tc.draw(gs::integers::<u64>());
    let capacity = tc.draw(gs::integers::<u64>());
    let delay = tc.draw(gs::integers::<u64>());
    let ceiling = tc.draw(gs::integers::<u64>().min_value(1));
    let classes = tc.draw(gs::integers::<u32>());
    for target in [Some(25), Some(60), None] {
        let rate = resources::rate(old, capacity, delay, target, classes, Some(ceiling));
        assert!((1..=ceiling).contains(&rate));
        assert!(
            rate >= u64::from(classes.max(1))
                .saturating_mul(16_384)
                .min(ceiling)
        );
    }
    assert_eq!(
        resources::rate(old, capacity, delay, None, classes, Some(ceiling)),
        ceiling
    );
}

#[hegel::test(test_cases = 1000)]
fn excess_queueing_never_increases_a_rate_within_its_bounds(tc: TestCase) {
    let old = tc.draw(
        gs::integers::<u64>()
            .min_value(16_384)
            .max_value(1_000_000_000),
    );
    let capacity = tc.draw(gs::integers::<u64>());
    let delay = tc.draw(gs::integers::<u64>().min_value(26));
    assert!(resources::rate(old, capacity, delay, Some(25), 1, Some(1_000_000_000)) <= old);
    assert!(resources::rate(old, capacity, 0, Some(25), 1, Some(1_000_000_000)) >= old);
}

#[hegel::test(test_cases = 1000)]
fn bucket_spending_cannot_exceed_rate_time_or_burst(tc: TestCase) {
    let rate = tc.draw(gs::integers::<u64>().min_value(1).max_value(1_000_000));
    let burst = tc.draw(gs::integers::<u64>().min_value(1).max_value(65_536));
    let delta = tc.draw(gs::integers::<u64>().max_value(5_000));
    let mut bucket = Bucket::new(0, burst, rate);
    let mut spent = 0u128;
    for step in 0..100u64 {
        let bytes = 1 + step % burst;
        if bucket.reserve(step * delta, bytes, rate) == Permit::Granted {
            spent += u128::from(bytes) * 1_000;
        }
        assert!(spent <= u128::from(step * delta) * u128::from(rate));
    }
    assert_eq!(
        bucket.reserve(100 * delta, burst + 1, rate),
        Permit::TooLarge(burst)
    );
}

#[hegel::test(test_cases = 1000)]
fn rate_changes_and_backwards_clocks_grant_no_past_credit(tc: TestCase) {
    let now = tc.draw(gs::integers::<u64>());
    let faster = tc.draw(gs::integers::<u64>().min_value(1));
    let mut bucket = Bucket::new(now, 65_536, 1);
    assert!(matches!(bucket.reserve(now, 1, faster), Permit::Wait(_)));
    assert!(matches!(
        bucket.reserve(now.saturating_sub(1), 1, faster),
        Permit::Wait(_)
    ));
    let mut bucket = Bucket::new(0, 65_536, 1);
    assert!(matches!(bucket.reserve(1, 1, faster), Permit::Wait(_)));
}

#[hegel::test(test_cases = 1000)]
fn arbitrary_pauses_do_not_spend_an_operations_active_budget(tc: TestCase) {
    let active = tc.draw(gs::integers::<u64>().max_value(1_000_000));
    let pause = tc.draw(gs::integers::<u64>().max_value(u64::MAX - 2_000_000));
    let mut clock = ActiveClock::new(0, true);
    clock.observe(active, false);
    clock.observe(active + pause, true);
    clock.observe(active + pause + active, false);
    assert_eq!(clock.used(), 2 * active);
    assert_eq!(clock.remaining(2 * active), 0);
    assert_eq!(clock.remaining(2 * active + 1), 1);
}

#[hegel::test(test_cases = 1000)]
fn backwards_or_repeated_clock_samples_cannot_refund_active_time(tc: TestCase) {
    let start = tc.draw(gs::integers::<u64>());
    let now = tc.draw(gs::integers::<u64>()).max(start);
    let earlier = tc.draw(gs::integers::<u64>()).min(now);
    let mut clock = ActiveClock::new(start, true);
    clock.observe(now, false);
    let spent = clock.used();
    clock.observe(earlier, true);
    clock.observe(now, true);
    assert_eq!(clock.used(), spent);
    assert_eq!(spent, now - start);
}

#[hegel::test(test_cases = 1000)]
fn observing_a_running_interval_in_parts_has_the_same_charge(tc: TestCase) {
    let start = tc.draw(gs::integers::<u64>());
    let middle = tc.draw(gs::integers::<u64>()).max(start);
    let end = tc.draw(gs::integers::<u64>()).max(middle);
    let mut one = ActiveClock::new(start, true);
    let mut parts = one;
    one.observe(end, false);
    parts.observe(middle, true);
    parts.observe(end, false);
    assert_eq!(one, parts);
}
