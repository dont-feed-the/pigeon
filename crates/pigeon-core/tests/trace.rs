//! Laws of trace events (12 §12.3; pigeon-core `trace`, feature `tla-trace`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(feature = "tla-trace")]
mod common;

use common::{flag, int};
use hegel::TestCase;
use pigeon_core::trace::{Event, Lamport, Roster, Value};

fn text(tc: &TestCase) -> String {
    (0..int(tc, 0, 6))
        .map(|_| char::from(int(tc, 0x20, 0x7e) as u8))
        .collect()
}

fn value(tc: &TestCase, depth: u32) -> Value {
    match int(tc, 0, if depth == 0 { 2 } else { 4 }) {
        0 => Value::Int(int(tc, -1_000_000, 1_000_000)),
        1 => Value::Bool(flag(tc)),
        2 => Value::Str(text(tc)),
        3 => Value::List((0..int(tc, 0, 3)).map(|_| value(tc, depth - 1)).collect()),
        _ => Value::Record(
            (0..int(tc, 1, 3))
                .map(|i| (format!("f{i}"), value(tc, depth - 1)))
                .collect(),
        ),
    }
}

fn json(v: &Value) -> serde_json::Value {
    match v {
        Value::Int(n) => (*n).into(),
        Value::Bool(b) => (*b).into(),
        Value::Str(s) => s.clone().into(),
        Value::List(xs) => xs.iter().map(json).collect(),
        Value::Record(fs) => {
            serde_json::Value::Object(fs.iter().map(|(k, v)| (k.clone(), json(v))).collect())
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn a_line_parses_back_to_its_event(tc: TestCase) {
    let vars: Vec<(String, Value)> = (0..int(&tc, 0, 3))
        .map(|i| (format!("v{i}"), value(&tc, 3)))
        .collect();
    let device = text(&tc);
    let clock = int(&tc, 0, 1 << 40) as u64;
    let e = Event::new(Roster::Commit, device.clone(), clock, vars.clone());
    let line = e.to_ndjson();
    assert!(!line.contains('\n') && line.is_ascii());
    let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
    let expected = serde_json::json!({
        "spec": "Roster",
        "device": device,
        "clock": clock,
        "action": "Commit",
        "vars": serde_json::Value::Object(vars.iter().map(|(k, v)| (k.clone(), json(v))).collect()),
    });
    assert_eq!(parsed, expected);
}

#[hegel::test(test_cases = 1000)]
fn the_lamport_clock_orders_receives_after_sends(tc: TestCase) {
    let mut a = Lamport(int(&tc, 0, 100) as u64);
    let mut b = Lamport(int(&tc, 0, 100) as u64);
    let mut last_b = b.0;
    for _ in 0..int(&tc, 1, 10) {
        if flag(&tc) {
            let sent = a.tick();
            let got = b.receive(sent);
            assert!(got > sent);
        } else {
            b.tick();
        }
        assert!(b.0 > last_b);
        last_b = b.0;
    }
}
