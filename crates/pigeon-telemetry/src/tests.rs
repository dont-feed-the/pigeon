//! The laws of the telemetry directory and the shape of its lines.
#![allow(clippy::unwrap_used, reason = "tests")]

use hegel::TestCase;
use hegel::generators as gs;

use super::{prune, span};

/// Pruning deletes the oldest files first, keeps the newest, and leaves at
/// most the limit unless only the newest is left.
#[hegel::test(test_cases = 1000)]
fn pruning_drops_the_oldest_down_to_the_limit(tc: TestCase) {
    let n = tc.draw(gs::integers::<usize>().max_value(12));
    let files: Vec<(String, u64)> = (0..n)
        .map(|i| {
            (
                format!("f{i:03}"),
                tc.draw(gs::integers::<u64>().max_value(100)),
            )
        })
        .collect();
    let limit = tc.draw(gs::integers::<u64>().max_value(600));
    let gone = prune(&files, limit);
    let kept: Vec<&(String, u64)> = files.iter().filter(|f| !gone.contains(&f.0)).collect();
    // The oldest first: what goes is a prefix.
    let names: Vec<String> = files.iter().map(|f| f.0.clone()).collect();
    assert_eq!(gone, names[..gone.len()].to_vec());
    if let Some(last) = files.last() {
        assert!(!gone.contains(&last.0), "the newest stays");
    }
    let total: u64 = kept.iter().map(|f| f.1).sum();
    assert!(total <= limit || kept.len() <= 1, "{total} > {limit}");
    // Nothing more than needed: putting back the last deleted would overflow.
    if let Some(back) = gone.last() {
        let size = files.iter().find(|f| &f.0 == back).unwrap().1;
        assert!(total + size > limit);
    }
}

#[test]
fn a_span_line_is_otlp_json_with_its_parent() {
    let root = span("round");
    let mut child = root.child("capture");
    child.set("group", "g1").set("files", 3);
    child.fail("disk-full");
    let line = child.line(&serde_json::json!({"attributes": []}), child.start + 5);
    let s = &line["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
    assert_eq!(s["name"], "capture");
    assert_eq!(s["parentSpanId"], super::hex(&root.id));
    assert_eq!(s["traceId"], super::hex(&root.trace));
    assert_eq!(s["status"]["code"], 2);
    assert_eq!(s["attributes"][1]["value"]["intValue"], "3");
    assert_eq!(s["attributes"][0]["value"]["stringValue"], "g1");
}
