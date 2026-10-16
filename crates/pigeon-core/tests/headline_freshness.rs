//! Laws of the safety evidence's age (spec 06 §6.5): combining versions
//! cannot make an older check recent, and non-safety news cannot refresh it.

use hegel::TestCase;
use hegel::generators as gs;
use pigeon_core::headline::checked_at;
use pigeon_core::state::{Reason, State};
use pigeon_core::time::Time;

#[hegel::test(test_cases = 1000)]
fn the_headline_is_no_fresher_than_any_of_its_safety_checks(tc: TestCase) {
    let n = tc.draw(gs::integers::<usize>().max_value(20));
    let checks: Vec<Time> = (0..n)
        .map(|_| Time(tc.draw(gs::integers::<i64>().min_value(0).max_value(1_000_000))))
        .collect();
    let mut states: Vec<State<String>> = checks.iter().map(|at| State::Protected(*at)).collect();
    let first = checked_at(&states);
    assert_eq!(first, checks.iter().min().copied());
    for at in &checks {
        assert!(first.is_some_and(|first| first <= *at));
    }
    states.reverse();
    assert_eq!(
        first,
        checked_at(&states),
        "arrival order changes no observation"
    );
    states.push(State::Local(Reason::Sending));
    states.push(State::Uncertain(None));
    assert_eq!(
        first,
        checked_at(&states),
        "non-safety news refreshes no proof"
    );
    let at = Time(tc.draw(gs::integers::<i64>().min_value(0).max_value(1_000_000)));
    states.push(State::Uncertain(Some(at)));
    assert_eq!(
        checked_at(&states),
        Some(first.map_or(at, |first| first.min(at)))
    );
}
