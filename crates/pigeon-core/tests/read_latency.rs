//! Own-read feedback laws: bounded memory, time, strict ratio and independent
//! devices. Each law uses at least 1000 generated cases (ADR 0012).

use hegel::{TestCase, generators as gs};
use pigeon_core::read_latency::Window;

fn trained(base: u64) -> Window {
    let mut w = Window::default();
    for at in (0..=10_000).step_by(1_000) {
        w.observe(at, base, true);
    }
    w
}

#[hegel::test(test_cases = 1000)]
fn strict_tripling_is_required_and_devices_do_not_share_baselines(tc: TestCase) {
    let base = tc.draw(gs::integers::<u64>().min_value(1).max_value(u64::MAX / 4));
    let mut a = trained(base);
    let mut b = trained(base);
    for at in (11_000..=21_000).step_by(1_000) {
        a.observe(at, base * 3, true);
        b.observe(at, base * 3 + 1, true);
    }
    assert!(!a.busy(21_000));
    assert!(b.busy(21_000));
    assert!(!b.busy(31_000));
}

#[hegel::test(test_cases = 1000)]
fn arbitrary_and_backwards_clocks_cannot_make_unbounded_samples(tc: TestCase) {
    let readings = tc.draw(gs::vecs(gs::integers::<u64>()).max_size(150));
    let mut w = Window::default();
    let mut now = 0;
    for reading in readings {
        now = now.max(reading);
        w.observe(reading, reading, true);
        assert!(w.samples() <= 1000);
        let before = w.samples();
        for _ in 0..20 {
            w.observe(reading.saturating_sub(1), u64::MAX, false);
        }
        assert_eq!(w.samples(), before);
        assert!(!w.busy(now.saturating_add(10_000)) || now > u64::MAX - 10_000);
    }
}

#[hegel::test(test_cases = 1000)]
fn independently_busy_observations_cannot_train_a_quiet_baseline(tc: TestCase) {
    let base = tc.draw(gs::integers::<u64>().min_value(1).max_value(u64::MAX / 4));
    let mut w = Window::default();
    for at in (0..=20_000).step_by(1_000) {
        w.observe(at, base, false);
    }
    assert!(!w.busy(20_000));
    // A quiet flag on the latest sample does not make the preceding
    // independently busy portion of the median window quiet.
    for at in (21_000..=29_000).step_by(1_000) {
        w.observe(at, base * 4, true);
        assert!(!w.busy(at));
    }
    for at in (30_000..=40_000).step_by(1_000) {
        w.observe(at, base, true);
    }
    for at in (41_000..=51_000).step_by(1_000) {
        w.observe(at, base * 4, true);
    }
    assert!(w.busy(51_000));
}

#[test]
fn dense_reads_keep_the_full_bounded_time_window() {
    let mut w = trained(10);
    for now in 11_000..41_000 {
        w.observe(now, 40, true);
    }
    assert_eq!(w.samples(), 1000);
    assert!(w.busy(40_999));
}

#[hegel::test(test_cases = 1000)]
fn extreme_latencies_cannot_wrap_the_tripling_threshold(tc: TestCase) {
    let base = tc.draw(gs::integers::<u64>().min_value(1));
    let observed = tc.draw(gs::integers::<u64>().min_value(base));
    let mut w = trained(base);
    for now in (11_000..=21_000).step_by(1_000) {
        w.observe(now, observed, true);
    }
    assert_eq!(w.busy(21_000), u128::from(observed) > 3 * u128::from(base));
}
