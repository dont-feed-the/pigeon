//! Admission metadata laws: closed current schema, canonical request ids,
//! and byte-bound immutable witnesses (proposed ADR 0081).
#![allow(clippy::unwrap_used, reason = "admission format fixtures")]

use hegel::{TestCase, generators as gs};
use pigeon_core::authority::Unread;
use pigeond::admission_file::{Request, Witness};
use pigeond::report_file::AdmissionKind;

#[hegel::test(test_cases = 1000)]
fn admission_metadata_roundtrips_and_unknown_fields_cannot_be_acknowledged(tc: TestCase) {
    let hex = |n| pigeond::util::hex(&tc.draw(gs::binary().min_size(n).max_size(n)));
    let request = Request {
        format: 1,
        kind: AdmissionKind::Upgrade,
        base: hex(20),
        device: None,
    };
    let path = request.path().unwrap();
    let text = request.render().unwrap();
    assert_eq!(Request::parse(&path, &text), Ok(request.clone()));
    assert_eq!(request.id().unwrap().len(), 64);
    assert!(matches!(
        Request::parse(&path, &("future = true\n".to_owned() + &text)),
        Err(Unread::Unknown(_))
    ));
    assert!(matches!(
        Request::parse(&path, &text.replace("upgrade", "future")),
        Err(Unread::Unknown(_))
    ));
    for request in [
        request,
        Request {
            format: 1,
            kind: AdmissionKind::Custodian,
            base: hex(20),
            device: Some(hex(32)),
        },
    ] {
        let witness = Witness {
            format: 1,
            request,
            main: hex(20),
            reports: [(hex(32), hex(20))].into(),
        };
        let (path, text) = witness.render().unwrap();
        assert_eq!(Witness::parse(&path, &text), Ok(witness.clone()));
        assert!(Witness::parse(&path, &(text.clone() + "\n")).is_err());
        let mut future: toml_edit::DocumentMut = text.parse().unwrap();
        future["request"]["future"] = toml_edit::value(true);
        assert!(matches!(
            Witness::parse(&path, &future.to_string()),
            Err(Unread::Unknown(_))
        ));
        assert!(matches!(
            Witness::parse(&path, &text.replacen("format = 1", "format = 2", 1)),
            Err(Unread::Unknown(_))
        ));
    }
}
