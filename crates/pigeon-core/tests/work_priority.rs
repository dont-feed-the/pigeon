//! Laws of cross-group heavy-child priority (ADR 0037, 15 §15.2).
//! Counts are pending operations, not filesystem or custody evidence.

use hegel::{TestCase, generators as gs};
use pigeon_core::resources::WorkClass;

fn counts(tc: &TestCase) -> [u64; 3] {
    std::array::from_fn(|_| tc.draw(gs::integers::<u64>()))
}

/// Law *capture progress*: no count in any class can deny capture.
#[hegel::test(test_cases = 1000)]
fn capture_is_never_held_by_pending_children(tc: TestCase) {
    assert!(WorkClass::Capture.admitted(counts(&tc)));
}

/// Law *strict priority*: a nonempty higher class holds every lower one,
/// independently of the lower classes' counts, including maximum counts.
#[hegel::test(test_cases = 1000)]
fn higher_work_holds_lower_work(tc: TestCase) {
    let mut pending = counts(&tc);
    let high = usize::from(tc.draw(gs::integers::<u8>().max_value(1)));
    pending[high] = tc.draw(gs::integers::<u64>().min_value(1));
    for class in &WorkClass::ALL[high + 1..] {
        assert!(!class.admitted(pending));
    }
}

/// Law *release*: with no higher class pending, same/lower-class work
/// cannot keep a class suspended. Empty queues admit every class.
#[hegel::test(test_cases = 1000)]
fn releasing_higher_work_restores_admission(tc: TestCase) {
    let pending = counts(&tc);
    for (index, class) in WorkClass::ALL.into_iter().enumerate() {
        let mut released = pending;
        released[..index].fill(0);
        assert!(class.admitted(released));
    }
}
