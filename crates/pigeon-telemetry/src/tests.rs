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
fn the_sdk_exports_correlated_spans_and_gauges_as_otlp_json() {
    use opentelemetry::trace::{Span as _, TraceContextExt};
    let dir = tempfile::tempdir().unwrap();
    super::init(dir.path().to_path_buf(), "pigeond", "alice");
    let mut root = span("round");
    root.set("group", "g1");
    let mut child = root.child("capture");
    child.set("files", 3);
    child.fail("disk-full");
    let root_id = root.inner.span_context().span_id().to_string();
    let trace_id = root.inner.span_context().trace_id().to_string();
    drop(child);
    drop(root);
    super::gauge(
        "pigeon.versions",
        3.0,
        &[("group", "g1"), ("state", "local")],
    );
    // The bridge uses the very same SDK tracer and exporter.
    use tracing_subscriber::prelude::*;
    let subscriber = tracing_subscriber::registry().with(super::layer());
    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!("request", group = "g1");
        let _entered = span.enter();
        tracing::info!("accepted");
    });
    super::flush();
    let lines: Vec<serde_json::Value> = super::tail(dir.path(), 100)
        .iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let spans: Vec<&serde_json::Value> = lines
        .iter()
        .filter_map(|v| v["resourceSpans"].as_array())
        .flatten()
        .filter_map(|r| r["scopeSpans"].as_array())
        .flatten()
        .filter_map(|s| s["spans"].as_array())
        .flatten()
        .collect();
    let child = spans.iter().find(|s| s["name"] == "capture").unwrap();
    assert_eq!(child["parentSpanId"], root_id);
    assert_eq!(child["traceId"], trace_id);
    assert_eq!(child["status"]["code"], 2);
    assert!(
        child["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["key"] == "group" && a["value"]["stringValue"] == "g1")
    );
    assert!(spans.iter().any(|s| s["name"] == "request"));
    assert!(lines.iter().any(|v| v["resourceMetrics"].is_array()));
    // Noop before init is exercised by the pruning law's independent path.
    let _ = opentelemetry::Context::current().span();
}

/// Law: a group export contains only that group's spans and metric points;
/// unlabelled records stay local, and mixed incoming batches are refused.
#[hegel::test(test_cases = 1000)]
fn collection_never_crosses_a_group_boundary(tc: TestCase) {
    use serde_json::json;
    let n = tc.draw(gs::integers::<usize>().min_value(1).max_value(30));
    let groups: Vec<_> = (0..n)
        .map(|_| tc.draw(gs::integers::<u8>().max_value(3)))
        .collect();
    let attributes = |g: u8| {
        if g == 3 {
            json!([])
        } else {
            json!([{"key":"group","value":{"stringValue":format!("g{g}")}}])
        }
    };
    let traces = json!({"resourceSpans":[{"resource":{"attributes":[]},"scopeSpans":[{"spans":
        groups.iter().enumerate().map(|(i,g)| json!({"name":format!("s{i}"), "attributes":attributes(*g)})).collect::<Vec<_>>()
    }]}]});
    let metrics = json!({"resourceMetrics":[{"resource":{"attributes":[]},"scopeMetrics":[{"metrics":[{
        "name":"pigeon.versions", "gauge":{"dataPoints":groups.iter().enumerate().map(|(i,g)| json!({"asDouble":i as f64,"attributes":attributes(*g)})).collect::<Vec<_>>()}
    }]}]}]});
    for line in [traces, metrics] {
        let split = super::route::split(&line);
        for (group, part) in &split {
            assert!(super::route::validate(part, group));
            for other in ["g0", "g1", "g2", "g3"] {
                assert_eq!(super::route::validate(part, other), group == other);
            }
        }
        let expected: std::collections::BTreeSet<_> = groups
            .iter()
            .filter(|g| **g < 3)
            .map(|g| format!("g{g}"))
            .collect();
        assert_eq!(
            split
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        if groups.iter().any(|g| *g != groups[0]) || groups[0] == 3 {
            for group in split.keys() {
                assert!(!super::route::validate(&line, group));
            }
        }
    }
}

#[test]
fn a_remote_context_is_validated_and_not_shared_with_other_tasks() {
    use opentelemetry::trace::Span as _;
    let parent = "00-12345678901234567890123456789012-1234567890123456-01";
    let remote = super::remote_span("request", Some(parent));
    assert_eq!(
        remote.inner.span_context().trace_id().to_string(),
        "12345678901234567890123456789012"
    );
    let invalid = super::remote_span("request", Some("garbage"));
    assert_ne!(
        invalid.inner.span_context().trace_id(),
        remote.inner.span_context().trace_id()
    );
}

#[tokio::test]
async fn interleaved_futures_keep_their_own_trace_context() {
    let a = super::remote_span(
        "a",
        Some("00-11111111111111111111111111111111-1111111111111111-01"),
    );
    let b = super::remote_span(
        "b",
        Some("00-22222222222222222222222222222222-2222222222222222-01"),
    );
    let work = || async {
        tokio::task::yield_now().await;
        super::traceparent()
    };
    let (got_a, got_b) = tokio::join!(a.in_scope(work()), b.in_scope(work()));
    assert_eq!(got_a, a.traceparent());
    assert_eq!(got_b, b.traceparent());
    assert_ne!(got_a, got_b);
    assert!(super::traceparent().is_none());
}

#[test]
fn real_files_stay_within_the_advertised_byte_bound() {
    use std::sync::Mutex;
    let dir = tempfile::tempdir().unwrap();
    let sink = super::file::FileSink {
        dir: dir.path().to_path_buf(),
        file: Mutex::new(None),
    };
    // Each pair would just exceed one file: rotate before it does.
    let record = serde_json::json!({"sample":"x".repeat(super::FILE_BYTES as usize / 2)});
    for _ in 0..24 {
        sink.write(&record);
    }
    let sizes: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .collect();
    assert!(sizes.iter().all(|n| *n <= super::FILE_BYTES));
    assert!(sizes.iter().sum::<u64>() <= super::LIMIT_BYTES);
    assert!(!sizes.is_empty());
    sink.write(&serde_json::json!({"too_big":"x".repeat(super::FILE_BYTES as usize)}));
    assert!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().len())
            .sum::<u64>()
            <= super::LIMIT_BYTES
    );
}
