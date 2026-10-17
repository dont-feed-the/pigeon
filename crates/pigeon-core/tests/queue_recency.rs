//! Recency is considered after equal copy risks, before size and key
//! (15 §15.2). Every case compares protection work with genuine dates.

use hegel::{TestCase, generators as gs};
use pigeon_core::{
    queue::{Work, compare},
    time::Time,
};

/// Law: among equal custody/laptop counts and endangered status, a newer
/// introduction precedes an older one regardless of size or lexical key.
#[hegel::test(test_cases = 1000)]
fn newer_equal_risk_work_precedes_older_smaller_work(tc: TestCase) {
    let old = tc.draw(gs::integers::<i64>().max_value(i64::MAX - 1));
    let new = tc.draw(gs::integers::<i64>().min_value(old + 1));
    let custody = tc.draw(gs::integers::<u32>());
    let laptops = tc.draw(gs::integers::<u32>());
    let endangered = tc.draw(gs::integers::<u8>()).is_multiple_of(2);
    let work = |at, size, key: &str| Work::Protect {
        key: key.into(),
        custody_copies: custody,
        laptop_copies: laptops,
        endangered,
        introduced: Time(at),
        size,
    };
    assert_eq!(
        compare(&work(new, u64::MAX, "z"), &work(old, 0, "a")),
        std::cmp::Ordering::Less
    );
}
