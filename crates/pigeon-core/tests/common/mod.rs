//! Helpers shared by the law tests: small draws from a hegel test case.
#![allow(dead_code)]

pub mod roster_sim;

use hegel::TestCase;
use hegel::generators as gs;

/// An integer in `lo..=hi`.
pub fn int(tc: &TestCase, lo: i64, hi: i64) -> i64 {
    tc.draw(gs::integers::<i64>().min_value(lo).max_value(hi))
}

/// An index in `0..n`, for `n > 0`.
pub fn index(tc: &TestCase, n: usize) -> usize {
    tc.draw(gs::integers::<usize>().min_value(0).max_value(n - 1))
}

/// A boolean.
pub fn flag(tc: &TestCase) -> bool {
    tc.draw(gs::booleans())
}

/// One of `items`, which is not empty.
pub fn pick<T: Clone>(tc: &TestCase, items: &[T]) -> T {
    items[index(tc, items.len())].clone()
}
