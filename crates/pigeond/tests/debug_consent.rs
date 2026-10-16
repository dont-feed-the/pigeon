//! Real peer diagnostic disclosure (12 §12.2): nothing before local
//! consent, one authenticated group part after it, no other-group canary,
//! and no second use of the grant. Bundles remain local private files.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;
use common::*;
use pigeon_proto::Response;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

fn problem(responses: &[Response]) -> Option<&str> {
    responses.iter().find_map(|r| match r {
        Response::Problem(p) => Some(p.code.as_str()),
        _ => None,
    })
}

#[test]
fn a_peer_can_share_one_group_part_only_with_local_consent() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let a = start(&root, sockets.path(), "alice");
    let b = start(&root, sockets.path(), "bob");
    let fa = root.join("files-a");
    std::fs::create_dir_all(&fa).unwrap();
    std::fs::write(fa.join("private-name.txt"), b"hello\n").unwrap();
    let created = pigeon(
        &a,
        &[
            "create",
            "Family",
            fa.to_str().unwrap(),
            "--k",
            "1",
            "--name",
            "Alice",
        ],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    let invitation = pigeon(&a, &["invite", "Bob"]);
    let ticket = said(&invitation, "invitation", "code").unwrap();
    let joined = pigeon(
        &b,
        &[
            "join",
            &ticket,
            root.join("files-b").to_str().unwrap(),
            "--name",
            "Bob",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    let unasked = pigeon(&a, &["debug", "bundle", "--from", "Bob"]);
    assert_eq!(problem(&unasked), Some("needs-confirmation"));
    let denied = pigeon(&a, &["debug", "bundle", "--from", "Bob", "--yes"]);
    assert_eq!(problem(&denied), Some("debug-not-allowed"), "{denied:?}");
    // A fresh local consent must be explicit too.
    assert_eq!(
        problem(&pigeon(&b, &["debug", "allow", "Alice"])),
        Some("needs-confirmation")
    );
    let log = b.home.join("state/logs").join(format!(
        "pigeond-{}.ndjson",
        pigeond::util::date(pigeond::util::now())
    ));
    let mut f = std::fs::OpenOptions::new().append(true).open(log).unwrap();
    writeln!(
        f,
        "{{\"group\":\"other-group\",\"private\":\"never-send-this-canary\"}}"
    )
    .unwrap();
    let granted = pigeon(&b, &["debug", "allow", "Alice", "--yes"]);
    assert!(
        said(&granted, "debug-allowed", "name").is_some(),
        "{granted:?}"
    );
    let received = pigeon(&a, &["debug", "bundle", "--from", "Bob", "--yes"]);
    let path = said(&received, "bundle-written", "path").unwrap_or_else(|| panic!("{received:?}"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("never-send-this-canary"));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["peer_parts"].as_array().unwrap().len(), 1);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let again = pigeon(&a, &["debug", "bundle", "--from", "Bob", "--yes"]);
    assert_eq!(problem(&again), Some("debug-not-allowed"), "{again:?}");
    // A long-running peer's telemetry must not consume a fresh grant and
    // then refuse solely because its last 500 batches exceed the wire cap.
    let group = value["peer_parts"][0]["group"].as_str().unwrap();
    let telemetry = b.home.join("state/telemetry");
    let big = serde_json::json!({"resourceSpans":[{"scopeSpans":[{"spans":[{
        "name":"scoped-positive-control", "attributes":[
            {"key":"group","value":{"stringValue":group}},
            {"key":"payload","value":{"stringValue":"x".repeat(160_000)}}]
    }]}]}]});
    let line = serde_json::to_string(&big).unwrap();
    std::fs::write(
        telemetry.join("pigeond-99999999999999999999.jsonl"),
        format!("{}\n", line).repeat(20),
    )
    .unwrap();
    assert!(
        said(
            &pigeon(&b, &["debug", "allow", "Alice", "--yes"]),
            "debug-allowed",
            "name"
        )
        .is_some()
    );
    let received = pigeon(&a, &["debug", "bundle", "--from", "Bob", "--yes"]);
    let path = said(&received, "bundle-written", "path").unwrap_or_else(|| panic!("{received:?}"));
    let text = std::fs::read_to_string(path).unwrap();
    let second: serde_json::Value = serde_json::from_str(&text).unwrap();
    let part = &second["peer_parts"][0];
    assert!(serde_json::to_vec(part).unwrap().len() < 2 * 1024 * 1024);
    assert!(part["omitted"]["telemetry"].as_u64().unwrap() > 0);
    assert!(part.to_string().contains("scoped-positive-control"));
    assert!(!text.contains("never-send-this-canary"));
    assert_eq!(
        problem(&pigeon(&a, &["debug", "bundle", "--from", "Bob", "--yes"])),
        Some("debug-not-allowed")
    );
    assert_eq!(
        std::fs::read_dir(b.home.join("state/bundles"))
            .map(|i| i.count())
            .unwrap_or(0),
        0
    );
}
