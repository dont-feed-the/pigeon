//! Capture partition laws (15 §15.5), 1000 cases each, including path-byte
//! limits independently of the file-count bound and oversized singleton input.

use hegel::{TestCase, generators as gs};
use pigeon_core::capture_batch::{MAX_FILES, MAX_INPUT_BYTES, ranges};

fn paths(tc: &TestCase) -> Vec<String> {
    let count = tc.draw(gs::integers::<usize>().max_value(2500));
    let width = tc.draw(gs::integers::<usize>().max_value(200));
    (0..count)
        .map(|i| format!("{}-{i}", "é".repeat(width)))
        .collect()
}

#[hegel::test(test_cases = 1000)]
fn every_input_is_covered_exactly_once_in_order(tc: TestCase) {
    let paths = paths(&tc);
    assert_eq!(
        ranges(&paths).flatten().collect::<Vec<_>>(),
        (0..paths.len()).collect::<Vec<_>>()
    );
}

#[hegel::test(test_cases = 1000)]
fn both_native_argument_bytes_and_progress_identity_count_are_bounded(tc: TestCase) {
    let paths = paths(&tc);
    for range in ranges(&paths) {
        assert!(!range.is_empty() && range.len() <= MAX_FILES);
        assert!(paths[range].iter().map(|p| p.len() + 1).sum::<usize>() <= MAX_INPUT_BYTES);
    }
}

#[hegel::test(test_cases = 1000)]
fn another_path_cannot_fit_a_nonfinal_batch(tc: TestCase) {
    let paths = paths(&tc);
    for range in ranges(&paths) {
        if range.end < paths.len() && range.len() < MAX_FILES {
            let bytes = paths[range.clone()]
                .iter()
                .map(|p| p.len() + 1)
                .sum::<usize>();
            assert!(bytes + paths[range.end].len() + 1 > MAX_INPUT_BYTES);
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn an_oversized_path_stands_alone_without_losing_its_neighbors(tc: TestCase) {
    let extra = tc.draw(gs::integers::<usize>().max_value(1000));
    let paths = vec![
        "before".into(),
        "x".repeat(MAX_INPUT_BYTES + extra),
        "after".into(),
    ];
    assert_eq!(ranges(&paths).collect::<Vec<_>>(), vec![0..1, 1..2, 2..3]);
}
