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
    assert!(state.reads.values().all(|(_, w)| !w.busy(31_000)));
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
