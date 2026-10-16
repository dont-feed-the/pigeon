//! Purge format and P4 boundaries (08 §8.6): round trips preserve exact
//! scope and deadline; unknown schemas and unsafe paths never become a
//! decision. Admission must separately establish authority and ancestry.
#![allow(clippy::unwrap_used, reason = "format fixtures")]

use hegel::{TestCase, generators as gs};
use pigeon_core::authority::Unread;
use pigeond::purge_file::{PurgeDoc, parse, render};

fn doc() -> PurgeDoc {
    PurgeDoc {
        format: 1,
        kind: "purge".into(),
        paths: vec!["Photos/old-scans/".into()],
        base: "a".repeat(40),
        keys: Vec::new(),
        proposals: Vec::new(),
        requested_by: "alice".into(),
        retention_end: "2026-11-16T00:00:00Z".parse().unwrap(),
    }
}

#[hegel::test(test_cases = 1000)]
fn a_decision_round_trips_without_extending_its_scope(tc: TestCase) {
    let n = tc.draw(gs::integers::<u32>());
    let mut d = doc();
    d.paths = vec![format!("Photos-{n}/été/")];
    let at = tc.draw(gs::integers::<i64>().min_value(0).max_value(4_000_000_000));
    d.retention_end = pigeond::report_file::datetime(at);
    if tc.draw(gs::booleans()) {
        d.keys.push(format!("SHA256E-s{n}--{}.jpg", "f".repeat(64)));
        d.proposals.push(format!("{n:040x}"));
    }
    assert_eq!(parse(&render(&d).unwrap()), Ok(d.clone()));
    assert_eq!(d.end(), Some(at));
    assert!(d.decision().paths.contains(&format!("Photos-{n}/été")));
    assert!(!d.decision().paths.contains("Photos"));
}

#[test]
fn an_unknown_schema_or_kind_is_kept_unknown() {
    let text = render(&doc()).unwrap();
    assert!(matches!(
        parse(&text.replace("format = 1", "format = 2")),
        Err(Unread::Unknown(_))
    ));
    assert!(matches!(
        parse(&(text.clone() + "future_field = true\n")),
        Err(Unread::Unknown(_))
    ));
    assert!(matches!(
        parse(&text.replace("\"purge\"", "\"future\"")),
        Err(Unread::Unknown(_))
    ));
}

#[test]
fn malformed_metadata_absolute_and_traversing_scopes_are_refused() {
    for path in [
        "../Photos",
        "/Photos",
        ".pigeon/roster.toml",
        ".git/config",
        "Photos//",
        "Photos/../Docs",
    ] {
        let mut d = doc();
        d.paths = vec![path.into()];
        assert!(
            matches!(parse(&render(&d).unwrap()), Err(Unread::Malformed(_))),
            "{path}"
        );
    }
    let mut d = doc();
    d.paths = vec!["Photos".into(), "Photos/".into()];
    assert!(parse(&render(&d).unwrap()).is_err());
    d = doc();
    d.keys.push("fake-s4--anything".into());
    assert!(parse(&render(&d).unwrap()).is_err());
    d = doc();
    d.base = "HEAD~1".into();
    assert!(parse(&render(&d).unwrap()).is_err());
    d = doc();
    d.retention_end = "2026-11-16T00:00:00+02:00".parse().unwrap();
    assert!(parse(&render(&d).unwrap()).is_err());
}

#[test]
fn the_original_retention_period_cannot_be_shortened() {
    let d = doc();
    let period = 30 * 86_400;
    let end = d.end().unwrap();
    assert!(d.valid_end(end - period, period as u64));
    assert!(!d.valid_end(end - period + 1, period as u64));
    assert!(!d.valid_end(i64::MAX, u64::MAX));
}
