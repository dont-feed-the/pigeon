//! Laws of holding back a new path at capture (05 §5.4), as hegel-rust
//! property tests: a path is held back exactly when it is not portable or
//! respells a name already taken; the suggested name is portable and
//! differs, after folding, from every name taken beside it.
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]

use std::collections::BTreeMap;

use hegel::TestCase;
use hegel::generators as gs;
use pigeon_core::names::{component, fold, hold_back, portable, suggest, take_prefixes};

fn part(tc: &TestCase) -> String {
    const BITS: [&str; 12] = [
        "a", "A", "ß", "SS", "K", "\u{212a}", ".", " ", ":", "CON", "x.txt", "X.TXT",
    ];
    let n = 1 + tc.draw(gs::integers::<u8>().max_value(3));
    (0..n)
        .map(|_| BITS[usize::from(tc.draw(gs::integers::<u8>().max_value(11)))])
        .collect()
}

fn path(tc: &TestCase) -> String {
    let n = 1 + tc.draw(gs::integers::<u8>().max_value(2));
    (0..n).map(|_| part(tc)).collect::<Vec<_>>().join("/")
}

/// The names `taken` has directly under `parent`.
fn under(taken: &BTreeMap<String, String>, parent: &str) -> Vec<String> {
    taken
        .values()
        .filter_map(|s| s.strip_prefix(parent))
        .filter(|rest| !rest.is_empty() && !rest.contains('/'))
        .map(str::to_owned)
        .collect()
}

#[hegel::test(test_cases = 1000)]
fn a_held_back_path_gets_a_portable_name_beside_it(tc: TestCase) {
    let mut taken = BTreeMap::new();
    for _ in 0..tc.draw(gs::integers::<u8>().max_value(4)) {
        let p: Vec<String> = path(&tc).split('/').map(|c| suggest(c, &[])).collect();
        take_prefixes(&mut taken, &p.join("/"));
    }
    let p = path(&tc);
    let held = hold_back(&p, &taken, |parent| under(&taken, parent));
    let respelt = {
        let mut end = 0;
        p.split('/').enumerate().any(|(i, c)| {
            end += c.len() + usize::from(i > 0);
            taken.get(&fold(&p[..end])).is_some_and(|s| *s != p[..end])
        })
    };
    let pigeons = fold(p.split('/').next().unwrap()) == ".pigeon";
    assert_eq!(
        held.is_some(),
        portable(&p).is_err() || respelt || pigeons,
        "{p:?} {taken:?} {held:?}"
    );
    let Some(h) = held else { return };
    assert!(p.starts_with(&h.from), "{p:?} {h:?}");
    let to = h.to.expect("short paths get a name");
    let (parent, name) = to
        .rsplit_once('/')
        .map_or(("", to.as_str()), |(a, b)| (a, b));
    assert_eq!(component(name), Ok(()), "{to:?}");
    assert_ne!(fold(name), ".pigeon");
    let parent = if parent.is_empty() {
        String::new()
    } else {
        format!("{parent}/")
    };
    assert!(
        under(&taken, &parent).iter().all(|s| fold(s) != fold(name)),
        "{to:?} {taken:?}"
    );
}

#[test]
fn fixed_cases() {
    let mut taken = BTreeMap::new();
    take_prefixes(&mut taken, "Photos/cat.jpg");
    let none = |_: &str| Vec::new();
    assert_eq!(hold_back("Photos/dog.jpg", &taken, none), None);
    let h = hold_back("photos/dog.jpg", &taken, none).unwrap();
    assert_eq!(
        (h.from.as_str(), h.to.as_deref()),
        ("photos", Some("photos 2"))
    );
    let h = hold_back("Photos/a:b.txt", &taken, none).unwrap();
    assert_eq!(h.to.as_deref(), Some("Photos/a-b.txt"));
    let h = hold_back("CON.txt", &taken, none).unwrap();
    assert_eq!(h.to.as_deref(), Some("_CON.txt"));
}
