//! Laws of portable names (05 §5.3–5.4; ADR 0012), as hegel-rust property
//! tests: folding is idempotent; `portable` accepts every conflict name of
//! a portable path, which `is_copy_of` recognises; a suggested name is
//! portable and differs from every sibling after folding; names that do
//! not collide never change in the catalogue; and fixed cases of §5.4.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]

use std::collections::BTreeMap;

use hegel::TestCase;
use hegel::generators as gs;
use pigeon_core::catalogue::{Engine, Meta, Outcome};
use pigeon_core::names::{
    MAX_COMPONENT, NotPortable, Stamp, clashes, component, conflict_name, fold, is_copy_of,
    portable, suggest, unit,
};

/// Letters that fold in interesting ways, and random text.
fn name(tc: &TestCase) -> String {
    const BITS: [&str; 12] = [
        "a", "A", "ß", "SS", "é", "e\u{301}", "K", "\u{212a}", ".", " ", "-", "x.txt",
    ];
    let mut s = String::new();
    for _ in 0..tc.draw(gs::integers::<u8>().max_value(4)) {
        if tc.draw(gs::booleans()) {
            s.push_str(BITS[usize::from(tc.draw(gs::integers::<u8>().max_value(11)))]);
        } else {
            s.push_str(&tc.draw(gs::text().max_size(6)));
        }
    }
    s
}

/// A portable path of one to three components.
fn portable_path(tc: &TestCase) -> String {
    let n = 1 + tc.draw(gs::integers::<u8>().max_value(2));
    let parts: Vec<String> = (0..n).map(|_| suggest(&name(tc), &[])).collect();
    parts.join("/")
}

fn stamp(tc: &TestCase) -> Stamp {
    Stamp {
        utc: tc.draw(gs::integers::<i64>().min_value(0).max_value(4_102_444_800)),
        offset: i32::from(tc.draw(gs::integers::<i16>().min_value(-720).max_value(840))),
    }
}

#[hegel::test(test_cases = 1000)]
fn folding_is_idempotent(tc: TestCase) {
    let s = name(&tc);
    assert_eq!(fold(&fold(&s)), fold(&s));
}

#[hegel::test(test_cases = 1000)]
fn every_conflict_name_of_a_portable_path_is_portable(tc: TestCase) {
    let path = portable_path(&tc);
    assert_eq!(portable(&path), Ok(()), "{path:?}");
    let who = name(&tc);
    let n = u32::from(tc.draw(gs::integers::<u8>().max_value(12)));
    let c = conflict_name(&path, &who, stamp(&tc), n.max(1));
    assert_eq!(portable(&c), Ok(()), "{c:?} of {path:?}");
    assert_ne!(fold(&c), fold(&path));
    assert_eq!(
        c.rsplit_once('/').map(|x| x.0),
        path.rsplit_once('/').map(|x| x.0),
        "same folder"
    );
    if path.rsplit('/').next().unwrap().len() + 40 < MAX_COMPONENT {
        assert!(is_copy_of(&c, &path), "{c:?} is a copy of {path:?}");
    }
}

#[hegel::test(test_cases = 1000)]
fn a_suggested_name_is_portable_and_new(tc: TestCase) {
    let wanted = name(&tc);
    let siblings: Vec<String> = (0..tc.draw(gs::integers::<u8>().max_value(4)))
        .map(|_| name(&tc))
        .collect();
    let mut siblings: Vec<&str> = siblings.iter().map(String::as_str).collect();
    if tc.draw(gs::booleans()) {
        siblings.push(&wanted);
    }
    let s = suggest(&wanted, &siblings);
    assert_eq!(component(&s), Ok(()), "{s:?} for {wanted:?}");
    assert!(
        siblings.iter().all(|x| fold(x) != fold(&s)),
        "{s:?} among {siblings:?}"
    );
    if component(&wanted).is_ok() && siblings.iter().all(|x| fold(x) != fold(&wanted)) {
        assert_eq!(s, wanted, "a portable new name is kept");
    }
}

#[hegel::test(test_cases = 1000)]
fn names_that_do_not_collide_never_change(tc: TestCase) {
    let mut tree = BTreeMap::new();
    for i in 0..tc.draw(gs::integers::<u8>().max_value(5)) {
        tree.insert(portable_path(&tc), format!("k{i}"));
    }
    let paths: Vec<&str> = tree.keys().map(String::as_str).collect();
    if !clashes(paths.iter().copied()).is_empty() {
        return;
    }
    let units: Vec<&str> = paths.iter().map(|p| unit(p)).collect();
    if units.iter().any(|u| {
        tree.keys()
            .any(|p| p.starts_with(&format!("{u}/")) && !p.starts_with(*u))
    }) {
        return;
    }
    let mut engine: Engine<String> = Engine::new();
    let meta = Meta {
        id: "g".to_owned(),
        parents: vec![],
        device: "G".to_owned(),
        who: "Alice".to_owned(),
        person: pigeon_core::roster::PersonId("alice".to_owned()),
        when: Stamp { utc: 0, offset: 0 },
        resolves: vec![],
    };
    assert_eq!(
        engine.record(&meta, &tree, &|_, _| true),
        Outcome::Contribution
    );
    assert_eq!(engine.catalogue(&["g".to_owned()]).tree, tree);
}

#[test]
fn the_cases_of_section_5_4() {
    for bad in [
        "CON",
        "con.txt",
        "Lpt9.tar.gz",
        "a.",
        "a ",
        "a:b",
        "a\u{7}",
        "e\u{301}",
        "",
        ".GIT",
    ] {
        assert!(component(bad).is_err(), "{bad:?}");
    }
    assert_eq!(suggest(&format!("{}. x", "é".repeat(200)), &[]).len(), 246);
    assert_eq!(suggest("a:b?.txt", &["A-B-.TXT"]), "a-b- 2.txt");
    assert_eq!(suggest("nul.txt ", &[]), "_nul.txt");
    for good in [
        "CONSOLE",
        "a.b",
        "é",
        "a (Bob, 2026-10-13 14h02).txt",
        ".gitignore",
        "x".repeat(255).as_str(),
    ] {
        assert_eq!(component(good), Ok(()), "{good:?}");
    }
    assert_eq!(component(&"x".repeat(256)), Err(NotPortable::Length));
    assert_eq!(portable(".pigeon/roster.toml"), Ok(()));
    assert_eq!(portable(".Pigeon/roster.toml"), Err(NotPortable::Own));
    assert_eq!(portable(".pigeon"), Err(NotPortable::Own));
    assert_eq!(portable("a/.git/config"), Err(NotPortable::Own));
    assert_eq!(fold("Straße"), fold("STRASSE"));
    assert_eq!(fold("\u{212a}"), fold("k"));
    assert_eq!(
        clashes(["team/Photo.jpg", "team/photo.jpg"]),
        vec!["team/photo.jpg".to_owned()]
    );
    assert_eq!(
        clashes(["team/strasse", "team/Straße/a.txt"]),
        vec!["team/strasse".to_owned()]
    );
    assert_eq!(unit("team/R.pages/data/p.bin"), "team/R.pages");
    assert_eq!(unit("team/R.PAGES"), "team/R.PAGES");
    assert_eq!(unit("a/b.app/c.key/x"), "a/b.app");
}

#[test]
fn conflict_names_carry_the_recorded_time() {
    let at = |utc, offset| Stamp { utc, offset };
    assert_eq!(at(0, 0).label(), "1970-01-01 00h00");
    assert_eq!(
        at(1_791_849_600 + 14 * 3600 + 120, 0).label(),
        "2026-10-13 14h02"
    );
    assert_eq!(
        at(1_791_849_600 + 14 * 3600 + 120, 120).label(),
        "2026-10-13 16h02"
    );
    assert_eq!(at(1_791_849_600 + 3600, -300).label(), "2026-10-12 20h00");
    assert_eq!(at(1_709_164_800, 0).label(), "2024-02-29 00h00");
    let when = at(1_791_849_600 + 14 * 3600 + 120, 0);
    assert_eq!(
        conflict_name("team/a.txt", "Bob", when, 1),
        "team/a (Bob, 2026-10-13 14h02).txt"
    );
    assert_eq!(
        conflict_name("team/a.txt", "Bob", when, 3),
        "team/a (Bob, 2026-10-13 14h02 3).txt"
    );
    assert_eq!(
        conflict_name("team/R.pages", "Bob", when, 1),
        "team/R (Bob, 2026-10-13 14h02).pages"
    );
    assert_eq!(
        conflict_name(".bashrc", "A: (B)", when, 1),
        ".bashrc (A- -B-, 2026-10-13 14h02)"
    );
}
