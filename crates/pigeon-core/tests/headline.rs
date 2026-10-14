//! Laws of the headline (06 §6.5; pigeon-core `headline`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::{flag, int, pick};
use hegel::TestCase;
use pigeon_core::headline::{Level, headline, version_level};
use pigeon_core::state::{Custodian, Settings, State, Version, state};
use pigeon_core::time::{Span, Time};

const LEVELS: [Level; 5] = [
    Level::Safe,
    Level::Working,
    Level::NotCheckedRecently,
    Level::NotSafeYet,
    Level::NeedsYou,
];

fn levels(tc: &TestCase) -> Vec<Level> {
    (0..int(tc, 0, 6)).map(|_| pick(tc, &LEVELS)).collect()
}

#[hegel::test(test_cases = 1000)]
fn join_is_a_semilattice(tc: TestCase) {
    let (a, b, c) = (pick(&tc, &LEVELS), pick(&tc, &LEVELS), pick(&tc, &LEVELS));
    assert_eq!(a.join(b).join(c), a.join(b.join(c)));
    assert_eq!(a.join(b), b.join(a));
    assert_eq!(a.join(a), a);
    assert_eq!(a.join(Level::Safe), a);
    assert_eq!(a.join(Level::NeedsYou), Level::NeedsYou);
}

#[hegel::test(test_cases = 1000)]
fn headline_of_a_union_is_the_join(tc: TestCase) {
    let (a, b) = (levels(&tc), levels(&tc));
    let both: Vec<Level> = a.iter().chain(b.iter()).copied().collect();
    assert_eq!(
        headline(both.clone()),
        headline(a.clone()).join(headline(b))
    );
    let mut reversed = both.clone();
    reversed.reverse();
    assert_eq!(headline(both), headline(reversed));
    assert_eq!(headline(Vec::new()), Level::Safe);
}

#[hegel::test(test_cases = 1000)]
fn adding_an_item_never_lowers_it(tc: TestCase) {
    let mut items = levels(&tc);
    let before = headline(items.clone());
    items.push(pick(&tc, &LEVELS));
    assert!(headline(items) >= before);
}

#[hegel::test(test_cases = 1000)]
fn only_here_is_not_safe_yet_after_grace_even_unreachable(tc: TestCase) {
    let now = Time(int(&tc, 0, 1_000_000));
    let custodians = (0..int(&tc, 0, 3))
        .map(|i| Custodian {
            device: i as u8,
            heard: if flag(&tc) {
                Some(Time(now.0 - int(&tc, 0, 200_000)))
            } else {
                None
            },
            holds: false,
            has_commit: flag(&tc),
            checking: flag(&tc),
            repository_lost: false,
        })
        .collect();
    let v = Version {
        custodians,
        other_device_has_commit: false,
        this_device_holds: true,
        erased: false,
    };
    let settings = Settings::default();
    let s = state(&v, &settings, now);
    assert!(matches!(s, State::Local(_)), "{s:?}");
    let grace = Span::minutes(10);
    let age = Span(grace.0 + int(&tc, 1, 100_000));
    assert_eq!(
        version_level(&s, age, grace, &settings, now),
        Level::NotSafeYet
    );
}
