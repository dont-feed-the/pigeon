//! Recovery keys and cached annex tokens stay out of diagnostic formatting.

#[test]
fn a_recovery_sheets_debug_hides_the_key_but_its_codec_retains_it() {
    let sheet = crate::recovery::Sheet {
        v: 1,
        group: "diagnostic-group".into(),
        name: "fixture-group".into(),
        k: 2,
        person: "fixture-person".into(),
        label: String::new(),
        key: "abcdef0123456789".repeat(4),
        custodians: Vec::new(),
    };
    for text in [format!("{sheet:?}"), format!("{sheet:#?}")] {
        assert!(
            !text.contains("abcdef0123456789"),
            "recovery key appeared in Debug"
        );
        assert!(text.contains("diagnostic-group"));
    }
    let restored =
        crate::recovery::decode(&crate::recovery::encode(&sheet)).expect("fixture sheet");
    assert!(
        restored == sheet,
        "Debug redaction changed the recovery codec"
    );
}

#[test]
fn cached_group_diagnostics_do_not_format_the_annex_token() {
    let known = crate::group::Known {
        token: Some("synthetic-cached-authority".into()),
        roster_commit: Some("fixture-commit".into()),
        ..Default::default()
    };
    for text in [format!("{known:?}"), format!("{known:#?}")] {
        assert!(
            !text.contains("synthetic-cached"),
            "cached authority appeared in Debug"
        );
        assert!(text.contains("fixture-commit"));
    }
    assert!(
        known
            .token
            .as_ref()
            .is_some_and(|t| t == "synthetic-cached-authority")
    );
}
