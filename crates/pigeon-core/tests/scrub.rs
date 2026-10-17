//! Scrub laws (08 §8.8): exact local windows, no invented completion,
//! idempotent findings and overflow-safe full-pass scheduling.
#![allow(clippy::panic, reason = "generated valid window parsing law")]
use std::collections::BTreeSet;

use hegel::{TestCase, generators as gs};
use pigeon_core::scrub::{Pass, Window, due};

#[hegel::test(test_cases = 1000)]
fn a_window_permits_exactly_its_half_open_circular_width(tc: TestCase) {
    let start = tc.draw(gs::integers::<u16>()) % 1440;
    let end = tc.draw(gs::integers::<u16>()) % 1441;
    let text = format!(
        "{:02}:{:02}-{:02}:{:02}",
        start / 60,
        start % 60,
        end / 60,
        end % 60
    );
    let Some(window) = Window::parse(&text) else {
        panic!("valid fixture window: {text}")
    };
    let actual = (0..1440).filter(|minute| window.contains(*minute)).count();
    let expected = if end >= start {
        end - start
    } else {
        1440 - start + end
    };
    assert_eq!(actual, usize::from(expected));
    assert!(!window.contains(1440));
    for invalid in [
        "24:00-01:00",
        "00:60-01:00",
        "1:00-06:00",
        "01:00-25:00",
        "01:00-06:00-extra",
    ] {
        assert!(Window::parse(invalid).is_none());
    }
}

#[hegel::test(test_cases = 1000)]
fn partial_and_repeated_acknowledgements_cannot_invent_a_full_pass(tc: TestCase) {
    let keys: BTreeSet<u16> = tc
        .draw(gs::vecs(gs::integers::<u16>()).max_size(32))
        .into_iter()
        .collect();
    let mut pass = Pass::new(keys.clone());
    for (i, key) in keys.iter().enumerate() {
        assert!(!pass.complete());
        pass.checked(*key, i % 2 == 0);
        let once = pass.clone();
        pass.checked(*key, i % 2 != 0);
        assert_eq!(pass, once);
        assert!(pass.pending.is_disjoint(&pass.bad));
        assert!(pass.pending.is_subset(&keys) && pass.bad.is_subset(&keys));
    }
    assert!(pass.complete());
    assert_eq!(pass.bad.len(), keys.len().div_ceil(2));
}

#[hegel::test(test_cases = 1000)]
fn no_timestamp_overflow_or_backward_jump_can_make_a_pass_due_early(tc: TestCase) {
    let now = tc.draw(gs::integers::<i64>());
    let last = tc.draw(gs::integers::<i64>());
    let period = tc.draw(gs::integers::<u64>());
    let age = i128::from(now) - i128::from(last);
    assert_eq!(due(now, Some(last), period), age >= i128::from(period));
    assert!(due(now, None, period));
    if now < last {
        assert!(!due(now, Some(last), period));
    }
}
