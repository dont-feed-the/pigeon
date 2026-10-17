//! The invitation and join schemas are closed (ADR 0048). Unknown fields
//! never silently change admission or the refounding a joiner accepts.
#![allow(clippy::unwrap_used, reason = "closed-schema wire fixtures")]

use super::*;
use serde_json::{Value, json};

fn invitation() -> Invitation {
    Invitation {
        v: INVITATION_VERSION,
        group: "group".into(),
        name: "Family".into(),
        k: 2,
        admitters: vec![Admitter {
            device: "admitter".into(),
            person: "Ana".into(),
            addresses: vec![],
        }],
        secret: "00".repeat(32),
        relay: None,
        refounded: Some(Refounded {
            from: "previous-group".into(),
            by: "Ana".into(),
            at: "2026-10-17T00:00:00Z".into(),
        }),
    }
}

fn rejects_invitation_at(path: &[&str]) {
    let known = invitation();
    assert_eq!(decode(&encode(&known)), Some(known.clone()));
    let mut value = serde_json::to_value(&known).unwrap();
    let mut target = &mut value;
    for part in path {
        target = if *part == "0" {
            &mut target[0]
        } else {
            &mut target[*part]
        };
    }
    target["future_authority_rule"] = Value::Bool(true);
    let code = format!(
        "pgn2-{}",
        BASE32_NOPAD
            .encode(&serde_json::to_vec(&value).unwrap())
            .to_ascii_lowercase()
    );
    assert!(
        decode(&code).is_none(),
        "unknown invitation schema was interpreted"
    );
}

#[test]
fn unknown_invitation_fields_are_not_understood() {
    rejects_invitation_at(&[]);
}
#[test]
fn unknown_admitter_fields_are_not_understood() {
    rejects_invitation_at(&["admitters", "0"]);
}
#[test]
fn unknown_refounding_fields_are_not_understood() {
    rejects_invitation_at(&["refounded"]);
}

#[test]
fn unknown_join_fields_are_not_interpreted_as_known_authority() {
    let mut value = json!({"v": JOIN_VERSION, "invitation_hash": "hash", "proof": "synthetic-proof", "device_name": "laptop", "role": "laptop", "annex_uuid": "uuid"});
    let known: Join = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        serde_json::from_slice::<Join>(&serde_json::to_vec(&known).unwrap()).unwrap(),
        known
    );
    value["future_authority_rule"] = Value::Bool(true);
    assert!(serde_json::from_value::<Join>(value).is_err());
}

#[test]
fn unknown_join_answer_fields_are_not_interpreted_as_a_known_welcome() {
    for mut value in [
        json!({"type":"Welcome", "commit":"commit", "annex_uuid":"uuid"}),
        json!({"type":"Refused", "code":"invite-unknown"}),
    ] {
        let known: JoinAnswer = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            serde_json::from_slice::<JoinAnswer>(&serde_json::to_vec(&known).unwrap()).unwrap(),
            known
        );
        value["future_authority_rule"] = Value::Bool(true);
        assert!(serde_json::from_value::<JoinAnswer>(value).is_err());
    }
}
