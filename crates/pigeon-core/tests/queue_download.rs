//! Download priorities precede version dates, which precede key ties
//! (15 §15.2). Exercise full signed timestamps, independently of arrival.

mod common;

use common::{int, pick};
use hegel::TestCase;
use pigeon_core::queue::{Priority, Work, compare};
use pigeon_core::time::Time;

#[hegel::test(test_cases = 1000)]
fn folder_priority_precedes_any_recency_or_key(tc: TestCase) {
    let (higher, lower) = pick(
        &tc,
        &[
            (Priority::High, Priority::Normal),
            (Priority::High, Priority::Low),
            (Priority::Normal, Priority::Low),
        ],
    );
    let first = Work::Download {
        key: "z".into(),
        priority: higher,
        changed: Time(int(&tc, i64::MIN, i64::MAX)),
    };
    let second = Work::Download {
        key: "a".into(),
        priority: lower,
        changed: Time(int(&tc, i64::MIN, i64::MAX)),
    };
    assert!(compare(&first, &second).is_lt());
}

#[hegel::test(test_cases = 1000)]
fn equal_priority_downloads_choose_newer_before_key(tc: TestCase) {
    let old = int(&tc, i64::MIN, i64::MAX - 1);
    let new = int(&tc, old + 1, i64::MAX);
    let priority = pick(&tc, &[Priority::High, Priority::Normal, Priority::Low]);
    let first = Work::Download {
        key: "z".into(),
        priority,
        changed: Time(new),
    };
    let second = Work::Download {
        key: "a".into(),
        priority,
        changed: Time(old),
    };
    assert!(compare(&first, &second).is_lt());
}
