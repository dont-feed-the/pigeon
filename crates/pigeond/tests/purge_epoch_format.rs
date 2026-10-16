//! Completion-witness format law (proposed ADR 0078): closed schema,
//! byte-bound names, deterministic maps and exact raw-report identities.
#![allow(clippy::unwrap_used, reason = "completion format fixtures")]

use hegel::{TestCase, generators as gs};
use pigeon_core::authority::Unread;
use pigeond::purge_epoch::{Doc, parse, render};

#[hegel::test(test_cases = 1000)]
fn a_completion_witness_roundtrips_and_never_accepts_changed_bytes_under_its_old_name(
    tc: TestCase,
) {
    let hex = |n| pigeond::util::hex(&tc.draw(gs::binary().min_size(n).max_size(n)));
    let doc = Doc {
        format: 1,
        kind: "purge-epoch".into(),
        decision: hex(32),
        main: hex(20),
        reports: [(hex(32), hex(20))].into(),
    };
    let (path, text) = render(&doc).unwrap();
    assert_eq!(parse(&path, &text), Ok(doc.clone()));
    assert_eq!(render(&doc).unwrap(), (path.clone(), text.clone()));
    assert!(parse(&path, &(text.clone() + "\n")).is_err());
    assert!(matches!(
        parse(&path, &text.replacen("format = 1", "format = 2", 1)),
        Err(Unread::Unknown(_))
    ));
    assert!(matches!(
        parse(&path, &("future = true\n".to_owned() + &text)),
        Err(Unread::Unknown(_))
    ));
    let empty = Doc {
        reports: Default::default(),
        ..doc
    };
    let (path, text) = render(&empty).unwrap();
    assert!(matches!(parse(&path, &text), Err(Unread::Malformed(_))));
}
