//! Recovery-sheet schemas are closed and the last candidate cannot fall
//! back to an earlier authority (ADR 0048, 09 §9.9). Codes stay private.
#![allow(clippy::unwrap_used, reason = "isolated recovery codec fixtures")]

use super::*;
use serde_json::{Value, json};

fn sheet() -> Sheet {
    Sheet {
        v: 1,
        group: "a".repeat(40),
        name: "Family".into(),
        k: 2,
        person: "admin".into(),
        label: String::new(),
        key: "00".repeat(32),
        custodians: vec![SheetCustodian {
            device: "b".repeat(64),
            name: "backup".into(),
            addresses: vec!["127.0.0.1:1234".into()],
        }],
    }
}

fn code(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap();
    let body = BASE32_NOPAD.encode(&bytes).to_ascii_lowercase();
    let sum = sha256_hex(body.as_bytes());
    format!("pgr1-{body}-{}", &sum[..8])
}

#[test]
fn unknown_sheet_fields_are_not_silently_ignored() {
    let mut value = serde_json::to_value(sheet()).unwrap();
    value["future-authority"] = json!(true);
    assert!(
        decode(&code(&value)).is_none(),
        "unknown top-level field was accepted"
    );
}

#[test]
fn unknown_backup_fields_are_not_silently_ignored() {
    let mut value = serde_json::to_value(sheet()).unwrap();
    value["custodians"][0]["future-role"] = json!("reader");
    assert!(
        decode(&code(&value)).is_none(),
        "unknown nested field was accepted"
    );
}

#[test]
fn a_later_unreadable_sheet_does_not_select_an_earlier_one() {
    let valid = encode(&sheet());
    let unreadable = "pgr1-not-a-complete-code";
    let pasted = format!("Earlier sheet: {valid}\nUse this one: {unreadable}");
    assert!(
        decode(&pasted).is_none(),
        "an earlier recovery authority was selected"
    );
}

#[test]
fn a_later_future_sheet_does_not_select_an_earlier_one() {
    let valid = encode(&sheet());
    let mut future = serde_json::to_value(sheet()).unwrap();
    future["v"] = json!(2);
    let pasted = format!("Earlier sheet: {valid}\nUse this one: {}", code(&future));
    assert!(
        decode(&pasted).is_none(),
        "a later unknown version was skipped"
    );
    let future_family = valid.replacen("pgr1-", "pgr2-", 1);
    assert!(
        decode(&format!("{valid}\n{future_family}")).is_none(),
        "a future code family was skipped"
    );
}

#[test]
fn text_larger_than_the_existing_ipc_frame_is_refused() {
    let mut value = sheet();
    value.name = "x".repeat(pigeon_proto::wire::MAX_LINE);
    assert!(
        decode(&encode(&value)).is_none(),
        "an oversized standalone input was accepted"
    );
}

#[test]
fn an_input_exactly_at_the_wire_bound_keeps_its_supported_code() {
    let expected = sheet();
    let mut text = encode(&expected);
    text.push_str(&" ".repeat(pigeon_proto::wire::MAX_LINE - text.len()));
    assert_eq!(decode(&text), Some(expected));
}

#[hegel::test(test_cases = 1000)]
fn supported_fields_and_legacy_labels_roundtrip_without_change(tc: hegel::TestCase) {
    use hegel::generators as gs;
    let mut expected = sheet();
    expected.name = format!("Photos é {}", tc.draw(gs::integers::<u64>()));
    expected.person = format!("person-{}", tc.draw(gs::integers::<u16>()));
    expected.k = u32::from(tc.draw(gs::integers::<u8>().min_value(1).max_value(8)));
    if tc.draw(gs::integers::<u8>().max_value(1)) == 1 {
        expected.label = "2026-10-18-abc123".into();
    }
    expected.custodians = (0..tc.draw(gs::integers::<u8>().max_value(8)))
        .map(|n| SheetCustodian {
            device: format!("{n:064x}"),
            name: format!("backup-{n}"),
            addresses: vec![format!(
                "127.0.0.1:{}",
                tc.draw(gs::integers::<u16>().min_value(1))
            )],
        })
        .collect();
    let mut earlier = sheet();
    earlier.name = "Earlier group".into();
    let pasted = format!(
        "Earlier: {}\nFuture example: pgr2-unreadable\nCurrent: {}\nKeep this private.",
        encode(&earlier),
        encode(&expected)
    );
    assert_eq!(decode(&pasted), Some(expected));
}

#[hegel::test(test_cases = 1000)]
fn an_unsupported_last_candidate_never_falls_back_to_a_valid_earlier_sheet(tc: hegel::TestCase) {
    use hegel::generators as gs;
    let valid = encode(&sheet());
    let mut value = serde_json::to_value(sheet()).unwrap();
    let candidate = match tc.draw(gs::integers::<u8>().max_value(7)) {
        0 => {
            value[format!("future-{}", tc.draw(gs::integers::<u32>()))] = json!(true);
            code(&value)
        }
        1 => {
            value["custodians"][0]["future-authority"] = json!(false);
            code(&value)
        }
        2 => {
            value["v"] = json!(2);
            code(&value)
        }
        3 => valid.replacen("pgr1-", "pgr2-", 1),
        4 => "pgr1-not-a-complete-code".into(),
        5 => "pgr1".into(),
        6 => "PGR1-incomplete".into(),
        _ => "pgrX-future".into(),
    };
    assert!(
        decode(&format!("Old: {valid}\nCurrent: {candidate}")).is_none(),
        "unsupported last candidate selected earlier authority"
    );
}

#[test]
fn a_later_damaged_code_prefix_does_not_select_an_earlier_sheet() {
    let valid = encode(&sheet());
    for damaged in ["pgr1", "PGR1-incomplete", "pgrX-future", "pgr-incomplete"] {
        assert!(
            decode(&format!("Earlier: {valid}\nCurrent: {damaged}")).is_none(),
            "damaged prefix selected earlier recovery authority"
        );
    }
}
