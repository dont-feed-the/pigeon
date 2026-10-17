//! Diagnostic events may record timings/counts and refusal rules, never
//! stream credentials or private bytes, even when debug logging is enabled
//! (ADR 0022). Capture real tracing events with a live subscriber.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing_subscriber::{Layer, layer::Context, prelude::*};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<BTreeMap<String, String>>>>);

struct Fields(BTreeMap<String, String>);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().into(), format!("{value:?}"));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }
}

impl<S: tracing::Subscriber> Layer<S> for Capture {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let mut fields = Fields(BTreeMap::new());
        event.record(&mut fields);
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(fields.0);
    }
}

#[test]
#[allow(clippy::unwrap_used, reason = "isolated tracing event privacy fixture")]
fn relay_counts_survive_without_credentials_or_file_fragments_in_debug_events() {
    let capture = Capture::default();
    let up = b"AUTH fixture-annex-password\nDATA 21\nprivate-file-fragment\n";
    let down = b"AUTH-SUCCESS fixture-server-token\nDATA 19\nprivate-return-body\n";
    tracing::subscriber::with_default(tracing_subscriber::registry().with(capture.clone()), || {
        let mut seen = super::Seen::new();
        // Exercise transport chunk boundaries as well as complete frames.
        for chunk in up.chunks(7) {
            seen.up(chunk);
        }
        for chunk in down.chunks(11) {
            seen.down(chunk);
        }
        seen.log("fixture-peer");
    });
    let events = capture.0.lock().unwrap();
    let event = events
        .iter()
        .find(|e| e.get("message").is_some_and(|m| m == "relay-ended"))
        .unwrap();
    assert_eq!(event["up"], up.len().to_string());
    assert_eq!(event["down"], down.len().to_string());
    assert_eq!(event["peer"], "fixture-peer");
    for forbidden in [
        "fixture-annex-password",
        "fixture-server-token",
        "private-file-fragment",
        "private-return-body",
    ] {
        assert!(
            !event.values().any(|v| v.contains(forbidden)),
            "relay diagnostics retained private stream bytes"
        );
    }
}

#[test]
#[allow(
    clippy::unwrap_used,
    reason = "isolated refusal diagnostic privacy fixture"
)]
fn refused_messages_keep_the_rule_without_echoing_attacker_controlled_bytes() {
    let capture = Capture::default();
    let line = b"CONNECT private-file-fragment fixture-annex-password";
    tracing::subscriber::with_default(tracing_subscriber::registry().with(capture.clone()), || {
        super::log_refusal("fixture-peer", pigeon_core::filter::Rule::Push, line);
    });
    let events = capture.0.lock().unwrap();
    let event = events
        .iter()
        .find(|e| e.get("message").is_some_and(|m| m == "message-refused"))
        .unwrap();
    assert_eq!(event["peer"], "fixture-peer");
    assert_eq!(event["rule"], "Push");
    assert_eq!(event["bytes"], line.len().to_string());
    assert!(
        !event
            .values()
            .any(|v| v.contains("private-file-fragment") || v.contains("fixture-annex-password")),
        "refusal diagnostics echoed private peer input"
    );
}
