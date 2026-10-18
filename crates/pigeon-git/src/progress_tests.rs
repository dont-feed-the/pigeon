//! Progress parsing is independent of read boundaries, preserves final
//! output, and cannot count other keys, counters or malformed records.
#![allow(clippy::unwrap_used, reason = "keyed progress parser contracts")]

use super::*;
use hegel::{TestCase, generators as gs};

fn record(key: &str, count: u64) -> Vec<u8> {
    format!("{{\"byte-progress\":{count},\"total-size\":100,\"action\":{{\"command\":\"get\",\"key\":\"{key}\"}}}}\n").into_bytes()
}

#[hegel::test(test_cases = 1000)]
fn read_boundaries_preserve_final_bytes_and_activity(tc: TestCase) {
    let step = usize::from(tc.draw(gs::integers::<u8>().min_value(1)));
    let progress = Progress::new("get", "wanted");
    let final_line = b"{\"key\":\"wanted\",\"success\":true}\n";
    let mut stream = record("wanted", 1);
    stream.extend(record("other", 99));
    stream.extend(record("wanted", 7));
    stream.extend(final_line);
    let mut lines = Lines::default();
    let mut output = Vec::new();
    for bytes in stream.chunks(step) {
        lines.feed(bytes, &progress, &mut output);
    }
    output.append(&mut lines.pending);
    assert_eq!(output, final_line);
    assert_eq!(progress.bytes(), 7);
}

#[test]
fn progress_needs_the_same_command_key_and_increasing_valid_bytes() {
    let progress = Progress::new("get", "wanted");
    assert!(progress.consume(&record("wanted", 8)));
    for line in [
        record("wanted", 8),
        record("wanted", 1),
        record("other", 99),
        record("wanted", 101),
        b"{\"byte-progress\":20,\"action\":{\"command\":\"copy\",\"key\":\"wanted\"}}\n".to_vec(),
    ] {
        assert!(progress.consume(&line));
        assert_eq!(progress.bytes(), 8);
    }
    for line in [
        b"malformed\n".as_slice(),
        b"{\"byte-progress\":9,\"success\":true}\n",
        b"{\"byte-progress\":\"9\"}\n",
    ] {
        assert!(!progress.consume(line));
        assert_eq!(progress.bytes(), 8);
    }
}

#[test]
fn oversized_and_incomplete_lines_pass_through_without_renewing_time() {
    let progress = Progress::new("get", "wanted");
    let mut line = record("wanted", 100);
    line.splice(1..1, std::iter::repeat_n(b' ', MAX_LINE));
    line.extend(b"unterminated");
    let mut lines = Lines::default();
    let mut output = Vec::new();
    lines.feed(&line, &progress, &mut output);
    assert!(lines.pending.len() <= MAX_LINE);
    output.append(&mut lines.pending);
    assert_eq!(output, line);
    assert_eq!(progress.bytes(), 0);
}

/// Accepted counters form a maximum: arrival order and other keys do not
/// change the progress observed for this operation.
#[hegel::test(test_cases = 1000)]
fn matching_progress_is_an_order_independent_maximum(tc: TestCase) {
    let counts: [u64; 12] = std::array::from_fn(|_| tc.draw(gs::integers::<u64>().max_value(100)));
    let expected = counts.iter().copied().max().unwrap();
    for reverse in [false, true] {
        let progress = Progress::new("get", "wanted");
        for i in 0..counts.len() {
            let at = if reverse { counts.len() - 1 - i } else { i };
            assert!(progress.consume(&record("wanted", counts[at])));
            assert!(progress.consume(&record("other", 100)));
        }
        assert_eq!(progress.bytes(), expected);
    }
}

fn capture_record(file: &str, count: u64) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(
        &serde_json::json!({"byte-progress":count,"action":{"command":"add","file":file}}),
    )
    .unwrap();
    bytes.push(b'\n');
    bytes
}
fn capture() -> Progress {
    Progress::capture(
        &["annex", "add", "--json", "--batch", "-z"],
        Some(b"a\nfile\0b\0"),
    )
    .unwrap()
}

#[hegel::test(test_cases = 1000)]
fn capture_progress_is_the_saturating_sum_of_each_planned_files_maximum(tc: TestCase) {
    let counts: [u64; 12] = std::array::from_fn(|_| tc.draw(gs::integers::<u64>()));
    let expected = counts
        .iter()
        .step_by(2)
        .copied()
        .max()
        .unwrap()
        .saturating_add(counts.iter().skip(1).step_by(2).copied().max().unwrap());
    for reverse in [false, true] {
        let progress = capture();
        for index in 0..counts.len() {
            let at = if reverse {
                counts.len() - 1 - index
            } else {
                index
            };
            assert!(progress.consume(&capture_record(
                if at % 2 == 0 { "a\nfile" } else { "b" },
                counts[at]
            )));
            assert!(progress.consume(&capture_record("unplanned", u64::MAX)));
        }
        assert_eq!(progress.bytes(), expected);
    }
}

#[hegel::test(test_cases = 1000)]
fn capture_read_boundaries_preserve_all_final_answers(tc: TestCase) {
    let step = usize::from(tc.draw(gs::integers::<u8>().min_value(1)));
    let progress = capture();
    let first = b"{\"command\":\"add\",\"file\":\"a\\nfile\",\"success\":true}\n";
    let second = b"{\"command\":\"add\",\"file\":\"b\",\"success\":true}\n";
    let mut stream = capture_record("a\nfile", 7);
    stream.extend(first);
    stream.extend(capture_record("b", 11));
    stream.extend(capture_record("a\nfile", 3));
    stream.extend(second);
    let mut expected = first.to_vec();
    expected.extend(second);
    let mut lines = Lines::default();
    let mut output = Vec::new();
    for chunk in stream.chunks(step) {
        lines.feed(chunk, &progress, &mut output);
    }
    output.append(&mut lines.pending);
    assert_eq!(output, expected);
    assert_eq!(progress.bytes(), 18);
}

#[test]
fn only_bounded_complete_capture_batches_enable_file_progress() {
    let args = ["annex", "add", "--json", "--batch", "-z"];
    for input in [
        b"".as_slice(),
        b"unterminated",
        b"same\0same\0",
        b"\xff\0",
        b"a\0\0",
    ] {
        assert!(Progress::capture(&args, Some(input)).is_none());
    }
    let oversized: Vec<u8> = (0..65)
        .flat_map(|n| format!("file{n}\0").into_bytes())
        .collect();
    assert!(Progress::capture(&args, Some(&oversized)).is_none());
    assert!(Progress::capture(&args, Some(b"a\nfile\0")).is_some());
}
