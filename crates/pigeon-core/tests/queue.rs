//! Laws of the work queue's order (15 §15.2, spec 08 §8.9; pigeon-core `queue`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::cmp::Ordering;

use common::{flag, int, pick};
use hegel::TestCase;
use pigeon_core::queue::{Priority, Work, compare, order};
use pigeon_core::time::Time;

fn work(tc: &TestCase) -> Work {
    let t = |tc: &TestCase| Time(int(tc, 0, 3));
    let key = |tc: &TestCase| format!("k{}", int(tc, 0, 3));
    match int(tc, 1, 6) {
        1 => Work::Capture {
            changed: t(tc),
            path: format!("p{}", int(tc, 0, 2)),
        },
        2 => Work::History {
            notice: t(tc),
            peer: format!("d{}", int(tc, 0, 2)),
        },
        3 => Work::Protect {
            key: key(tc),
            custody_copies: int(tc, 0, 2) as u32,
            laptop_copies: int(tc, 0, 1) as u32,
            endangered: flag(tc),
            introduced: t(tc),
            size: int(tc, 0, 2) as u64,
        },
        4 => Work::Download {
            key: key(tc),
            priority: pick(tc, &[Priority::High, Priority::Normal, Priority::Low]),
            changed: t(tc),
        },
        5 => Work::Scrub {
            slice: int(tc, 0, 3) as u64,
        },
        _ => Work::Rebalance { key: key(tc) },
    }
}

#[hegel::test(test_cases = 1000)]
fn compare_is_a_total_order(tc: TestCase) {
    let (a, b, c) = (work(&tc), work(&tc), work(&tc));
    assert_eq!(compare(&a, &a), Ordering::Equal);
    assert_eq!(compare(&a, &b), compare(&b, &a).reverse());
    assert_eq!(compare(&a, &b) == Ordering::Equal, a == b);
    if compare(&a, &b) != Ordering::Greater && compare(&b, &c) != Ordering::Greater {
        assert_ne!(compare(&a, &c), Ordering::Greater);
    }
}

#[hegel::test(test_cases = 1000)]
fn a_higher_class_goes_first(tc: TestCase) {
    let (a, b) = (work(&tc), work(&tc));
    if a.class() < b.class() {
        assert_eq!(compare(&a, &b), Ordering::Less);
    }
}

#[hegel::test(test_cases = 1000)]
fn order_does_not_depend_on_arrival(tc: TestCase) {
    let items: Vec<Work> = (0..int(&tc, 0, 8)).map(|_| work(&tc)).collect();
    let mut a = items.clone();
    let mut b = items;
    b.reverse();
    if b.len() > 2 {
        b.swap(0, 1);
    }
    order(&mut a);
    order(&mut b);
    assert_eq!(a, b);
}

#[hegel::test(test_cases = 1000)]
fn fewer_custody_copies_never_wait_behind_more(tc: TestCase) {
    let (a, b) = (work(&tc), work(&tc));
    let copies = |w: &Work| match w {
        Work::Protect { custody_copies, .. } => Some(*custody_copies),
        _ => None,
    };
    if let (Some(x), Some(y)) = (copies(&a), copies(&b))
        && x < y
    {
        assert_eq!(compare(&a, &b), Ordering::Less);
    }
}
