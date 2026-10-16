//! Group-health laws (12 §12.2): partition, monoid, conservative safety and
//! pending-save precedence. Every law runs 1000 cases without telemetry.
use hegel::{TestCase, generators as gs};
use pigeon_core::{
    health::{Counts, current},
    state::{Missing, Reason, State},
    time::Time,
};
use std::collections::{BTreeMap, BTreeSet};

fn state(tc: &TestCase) -> State<()> {
    match tc.draw(gs::integers::<u8>().max_value(5)) {
        0 => State::Missing(Missing::ContentMissing),
        1 => State::Missing(Missing::Erased),
        2 => State::Local(Reason::Sending),
        3 => State::Published(Reason::Sending),
        4 => State::Uncertain(Some(Time(1))),
        _ => State::Protected(Time(1)),
    }
}

fn counts(tc: &TestCase) -> Counts {
    Counts {
        missing: tc.draw(gs::integers::<u64>()),
        erased: tc.draw(gs::integers::<u64>()),
        local: tc.draw(gs::integers::<u64>()),
        published: tc.draw(gs::integers::<u64>()),
        uncertain: tc.draw(gs::integers::<u64>()),
        protected: tc.draw(gs::integers::<u64>()),
    }
}

#[hegel::test(test_cases = 1000)]
fn counts_partition_each_observation_without_promoting_uncertainty(tc: TestCase) {
    let state = state(&tc);
    let counts = Counts::one(&state);
    assert_eq!(counts.total(), 1);
    assert_eq!(
        counts.protected,
        u64::from(matches!(state, State::Protected(_)))
    );
    assert_eq!(
        counts.uncertain,
        u64::from(matches!(state, State::Uncertain(_)))
    );
}

#[hegel::test(test_cases = 1000)]
fn combining_is_associative_even_at_saturation(tc: TestCase) {
    let (a, b, c) = (counts(&tc), counts(&tc), counts(&tc));
    assert_eq!(a.combine(b).combine(c), a.combine(b.combine(c)));
}

#[hegel::test(test_cases = 1000)]
fn combining_is_commutative_with_an_empty_identity(tc: TestCase) {
    let (a, b) = (counts(&tc), counts(&tc));
    assert_eq!(a.combine(b), b.combine(a));
    assert_eq!(a.combine(Counts::default()), a);
    assert_eq!(Counts::default().combine(a), a);
}

#[hegel::test(test_cases = 1000)]
fn pending_saves_replace_their_previous_state_and_count_once_as_local(tc: TestCase) {
    let observed: BTreeMap<u8, State<()>> = (0..tc.draw(gs::integers::<u8>().max_value(32)))
        .map(|path| (path, state(&tc)))
        .collect();
    let pending: BTreeSet<u8> = (0..tc.draw(gs::integers::<u8>().max_value(32))).collect();
    let actual = current(observed.iter(), &pending);
    let expected_total = observed
        .keys()
        .copied()
        .chain(pending.iter().copied())
        .collect::<BTreeSet<_>>()
        .len();
    assert_eq!(actual.total(), expected_total as u64);
    let changed: BTreeMap<_, _> = observed
        .iter()
        .map(|(path, old)| {
            (
                *path,
                if pending.contains(path) {
                    state(&tc)
                } else {
                    old.clone()
                },
            )
        })
        .collect();
    assert_eq!(current(changed.iter(), &pending), actual);
    assert!(actual.local >= pending.len() as u64);
}
