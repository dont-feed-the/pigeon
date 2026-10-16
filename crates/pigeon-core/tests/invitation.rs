//! Invitation scope and single-device redemption laws (09 §9.6).

use hegel::{TestCase, generators as gs};
use pigeon_core::invitation::{Redemption, redeem};

#[hegel::test(test_cases = 1000)]
fn another_group_or_an_unbound_record_never_grants(tc: TestCase) {
    let group = tc.draw(gs::integers::<u64>());
    let peer = tc.draw(gs::integers::<u64>());
    for used in [None, Some(&peer)] {
        assert_eq!(
            redeem(Some(&group), &group.wrapping_add(1), used, &peer),
            Redemption::Refused
        );
        assert_eq!(redeem(None, &group, used, &peer), Redemption::Refused);
    }
}

#[hegel::test(test_cases = 1000)]
fn an_unused_scoped_record_can_be_used_once(tc: TestCase) {
    let group = tc.draw(gs::integers::<u64>());
    let peer = tc.draw(gs::integers::<u64>());
    assert_eq!(redeem(Some(&group), &group, None, &peer), Redemption::First);
    assert_eq!(
        redeem(Some(&group), &group, Some(&peer), &peer),
        Redemption::Resume
    );
}

#[hegel::test(test_cases = 1000)]
fn a_recorded_identity_never_grants_another_first_use(tc: TestCase) {
    let group = tc.draw(gs::integers::<u64>());
    let first = tc.draw(gs::integers::<u64>());
    let next = tc.draw(gs::integers::<u64>());
    assert_ne!(
        redeem(Some(&group), &group, Some(&first), &next),
        Redemption::First
    );
}

#[hegel::test(test_cases = 1000)]
fn only_the_original_authenticated_device_can_resume(tc: TestCase) {
    let group = tc.draw(gs::integers::<u64>());
    let peer = tc.draw(gs::integers::<u64>());
    assert_eq!(
        redeem(Some(&group), &group, Some(&peer), &peer),
        Redemption::Resume
    );
    assert_eq!(
        redeem(Some(&group), &group, Some(&peer), &peer.wrapping_add(1)),
        Redemption::Refused
    );
}
