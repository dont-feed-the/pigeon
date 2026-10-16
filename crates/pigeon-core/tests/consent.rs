//! Laws of one-use diagnostic consent (12 §12.2): recipient and group
//! isolation, expiry, one use, and refusals that do not consume a grant.

use hegel::{TestCase, generators as gs};
use pigeon_core::consent::Grant;

fn inputs(tc: &TestCase) -> ((u64, u64), u64, u64) {
    (
        (tc.draw(gs::integers()), tc.draw(gs::integers())),
        tc.draw(gs::integers::<u64>().max_value(1_000_000)),
        tc.draw(gs::integers::<u64>().min_value(1).max_value(1000)),
    )
}

#[hegel::test(test_cases = 1000)]
fn expiry_never_discloses(tc: TestCase) {
    let (recipient, now, ttl) = inputs(&tc);
    let mut grant = Grant::new(recipient, now, ttl);
    let after = now + ttl + tc.draw(gs::integers::<u64>().max_value(1000));
    assert!(!grant.live(after));
    assert!(!grant.take(&recipient, after));
}

#[hegel::test(test_cases = 1000)]
fn recipient_and_group_must_match(tc: TestCase) {
    let (recipient, now, ttl) = inputs(&tc);
    let mut grant = Grant::new(recipient, now, ttl);
    assert!(!grant.take(&(recipient.0.wrapping_add(1), recipient.1), now));
    assert!(!grant.take(&(recipient.0, recipient.1.wrapping_add(1)), now));
    assert!(grant.take(&recipient, now));
}

#[hegel::test(test_cases = 1000)]
fn a_successful_use_cannot_repeat(tc: TestCase) {
    let (recipient, now, ttl) = inputs(&tc);
    let mut grant = Grant::new(recipient, now, ttl);
    let at = now + tc.draw(gs::integers::<u64>().max_value(ttl - 1));
    assert!(grant.take(&recipient, at));
    assert!(!grant.live(at));
    assert!(!grant.take(&recipient, at));
}

#[hegel::test(test_cases = 1000)]
fn refused_attempts_do_not_spend_consent(tc: TestCase) {
    let (recipient, now, ttl) = inputs(&tc);
    let mut grant = Grant::new(recipient, now, ttl);
    let before = grant.clone();
    assert!(!grant.take(&(recipient.0, recipient.1.wrapping_add(1)), now));
    assert_eq!(grant, before);
    assert!(grant.take(&recipient, now));
    let mut empty = Grant::new(recipient, now, 0);
    assert!(!empty.take(&recipient, now));
}
