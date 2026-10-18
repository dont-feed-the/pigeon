//! Native controller wiring: device-specific training, bounded retention,
//! stale observations and unchanged capture/quiet-recovery policy.

use super::*;
use pigeon_core::resources::Preset;

#[test]
fn slow_own_reads_apply_backoff_without_cross_training_devices() {
    let controller = Controller::default();
    for now in (0..=10_000).step_by(1_000) {
        controller.read_at(now, 1, Duration::from_micros(10));
        controller.read_at(now, 2, Duration::from_millis(1));
    }
    assert_eq!(
        controller.lock().disk.share(10_000, Preset::Gentle, false),
        100
    );
    for now in (11_000..=21_000).step_by(1_000) {
        controller.read_at(now, 1, Duration::from_micros(40));
        controller.read_at(now, 2, Duration::from_millis(1));
    }
    let mut state = controller.lock();
    assert!(state.reads[&1].1.busy(21_000));
    assert!(!state.reads[&2].1.busy(21_000));
    assert_eq!(state.disk.share(21_000, Preset::Gentle, false), 0);
    assert_eq!(state.disk.share(21_000, Preset::Gentle, true), 5);
    assert_eq!(state.disk.share(21_000, Preset::Fast, true), 50);
    // No fresh read after the last ten seconds: the existing quiet ramp
    // applies, without extending the effect of a stale latency sample.
    assert!(state.reads.values().all(|(_, w, _)| !w.busy(31_000)));
    state.disk.observe(31_000, false);
    assert_eq!(state.disk.share(36_000, Preset::Gentle, false), 0);
    assert_eq!(state.disk.share(66_000, Preset::Gentle, false), 100);
}

#[test]
fn device_retention_is_bounded_and_inactive_devices_expire() {
    let controller = Controller::default();
    for dev in 0..100 {
        controller.read_at(0, dev, Duration::from_micros(1));
    }
    assert_eq!(controller.lock().reads.len(), 32);
    controller.read_at(600_000, 999, Duration::from_micros(1));
    assert_eq!(controller.lock().reads.len(), 1);
    assert!(controller.lock().reads.contains_key(&999));
}

#[test]
fn capture_read_shares_are_isolated_by_device_but_keep_global_pressure() {
    let controller = Controller::default();
    for now in (0..=10_000).step_by(1000) {
        controller.read_at(now, 1, Duration::from_micros(10));
        controller.read_at(now, 2, Duration::from_millis(1));
    }
    for now in (11_000..=21_000).step_by(1000) {
        controller.read_at(now, 1, Duration::from_micros(40));
        controller.read_at(now, 2, Duration::from_millis(1));
    }
    let mut state = controller.lock();
    assert_eq!(state.read_share(21_000, 1), 5);
    assert_eq!(state.read_share(21_000, 2), 100);
    assert_eq!(state.read_share(21_000, 999), 100);
    // Native children still conservatively use the combined disk signal.
    assert_eq!(state.disk.share(21_000, Preset::Gentle, false), 0);
    state.pressure.observe(22_000, true);
    assert_eq!(state.read_share(22_000, 1), 5);
    assert_eq!(state.read_share(22_000, 2), 5);
    assert_eq!(state.read_share(22_000, 999), 5);
    state.settings.preset = Preset::Fast;
    assert_eq!(state.read_share(22_000, 2), 50);
    state.settings.preset = Preset::Gentle;
    state.pressure.observe(32_000, false);
    for (_, window, disk) in state.reads.values_mut() {
        disk.observe(32_000, window.busy(32_000));
    }
    assert_eq!(state.read_share(37_000, 2), 5);
    assert_eq!(state.read_share(67_000, 1), 100);
    assert_eq!(state.read_share(67_000, 2), 100);
}

/// Independent devices never spend each other's quiet read share, including
/// large latency values whose comparison requires wider arithmetic.
#[hegel::test(test_cases = 1000)]
fn an_unrelated_device_keeps_its_read_share(tc: hegel::TestCase) {
    use hegel::generators as gs;
    let device = tc.draw(gs::integers::<u64>());
    let other = device.wrapping_add(1);
    let quiet = tc.draw(gs::integers::<u64>().min_value(1).max_value(u64::MAX / 4));
    let unrelated = tc.draw(gs::integers::<u64>().min_value(1));
    let controller = Controller::default();
    for now in (0..=10_000).step_by(1000) {
        controller.read_at(now, device, Duration::from_nanos(quiet));
        controller.read_at(now, other, Duration::from_nanos(unrelated));
    }
    for now in (11_000..=21_000).step_by(1000) {
        controller.read_at(now, device, Duration::from_nanos(quiet * 4));
        controller.read_at(now, other, Duration::from_nanos(unrelated));
    }
    let state = controller.lock();
    assert_eq!(state.read_share(21_000, device), 5);
    assert_eq!(state.read_share(21_000, other), 100);
    assert_eq!(state.read_share(21_000, device.wrapping_add(2)), 100);
}
