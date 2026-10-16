//! Laws of `pigeon export` (09 §9.9, ADR 0047; pigeon-core `export`): the
//! manifest, key checks, safe paths, pointers and free names, over strings
//! drawn from small alphabets that hold the troublesome characters.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeSet;
use std::path::{Component, Path};

use common::{flag, index, int, pick};
use hegel::TestCase;
use pigeon_core::export::{
    Row, free_name, key_sha256, link_key, manifest, matches, pointer_key, read_manifest, safe,
};

fn text(tc: &TestCase, alphabet: &[char], max: usize) -> String {
    (0..index(tc, max + 1))
        .map(|_| pick(tc, alphabet))
        .collect()
}

const ANY: [char; 10] = ['a', 'é', ' ', '\t', '\n', '\r', '\\', 't', '/', '.'];
const NAME: [char; 5] = ['a', 'b', '.', 'é', ' '];
const HEX: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn hex(tc: &TestCase) -> String {
    (0..64).map(|_| pick(tc, &HEX)).collect()
}

fn row(tc: &TestCase) -> Row {
    Row {
        path: text(tc, &ANY, 8),
        size: u64::try_from(int(tc, 0, i64::MAX)).unwrap(),
        sha256: text(tc, &ANY, 4),
        commit: text(tc, &ANY, 4),
        author: text(tc, &ANY, 6),
        date: text(tc, &ANY, 4),
    }
}

#[hegel::test(test_cases = 1000)]
fn a_manifest_reads_back_as_written(tc: TestCase) {
    let rows: Vec<Row> = (0..index(&tc, 5)).map(|_| row(&tc)).collect();
    let text = manifest(&rows);
    assert_eq!(text.lines().count(), rows.len() + 1);
    assert_eq!(read_manifest(&text), Some(rows));
}

#[hegel::test(test_cases = 1000)]
fn content_matches_a_key_exactly_when_its_size_and_digest_are_the_keys(tc: TestCase) {
    let (h, size) = (hex(&tc), u64::try_from(int(&tc, 0, 1 << 40)).unwrap());
    let ext = pick(&tc, &["", ".txt", ".tar.gz"]);
    let key = if flag(&tc) {
        format!("SHA256E-s{size}--{h}{ext}")
    } else {
        format!("SHA256-s{size}--{h}")
    };
    assert_eq!(key_sha256(&key).as_deref(), Some(h.as_str()));
    let other = if flag(&tc) { h.clone() } else { hex(&tc) };
    let got = if flag(&tc) { size } else { size + 1 };
    assert_eq!(matches(&key, got, &other), other == h && got == size);
    assert!(matches(&key, size, &h.to_ascii_uppercase()));
    let foreign = format!("MD5E-s{size}--{h}{ext}");
    assert!(!matches(&foreign, size, &h));
}

#[hegel::test(test_cases = 1000)]
fn a_safe_path_stays_below_the_export_directory(tc: TestCase) {
    let parts: Vec<String> = (0..=index(&tc, 4))
        .map(|_| match index(&tc, 6) {
            0 => pick(&tc, &["..", ".", "", ".git", ".GIT"]).to_owned(),
            _ => text(&tc, &NAME, 4),
        })
        .collect();
    let mut path = parts.join("/");
    if index(&tc, 8) == 0 {
        path.insert(0, '/');
    }
    let bad = |c: &String| c.is_empty() || c == "." || c == ".." || c.eq_ignore_ascii_case(".git");
    let expected = !path.starts_with('/') && !parts.iter().any(bad);
    assert_eq!(safe(&path), expected, "{path:?}");
    if safe(&path) {
        let dir = Path::new("/export");
        let at = dir.join(&path);
        assert!(at.starts_with(dir));
        assert!(
            at.components()
                .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
        );
    }
}

#[hegel::test(test_cases = 1000)]
fn a_pointer_gives_back_its_key(tc: TestCase) {
    let key = format!("SHA256E-s{}--{}.txt", int(&tc, 0, 1 << 30), hex(&tc));
    let blob = format!("/annex/objects/{key}\n");
    assert_eq!(pointer_key(blob.as_bytes()).as_deref(), Some(key.as_str()));
    let link = format!(
        "{}.git/annex/objects/Xx/Yy/{key}/{key}",
        "../".repeat(index(&tc, 4))
    );
    assert_eq!(link_key(&link).as_deref(), Some(key.as_str()));
    let plain = text(&tc, &ANY, 12);
    assert_eq!(pointer_key(plain.as_bytes()), None);
}

#[hegel::test(test_cases = 1000)]
fn a_free_name_is_never_a_tree_path_nor_a_folder_of_one(tc: TestCase) {
    let names = [
        "MANIFEST.tsv",
        "MANIFEST.tsv-2",
        ".deleted",
        ".deleted-2",
        "a",
    ];
    let taken: BTreeSet<String> = (0..index(&tc, 6))
        .map(|_| {
            let top = pick(&tc, &names);
            let top = if flag(&tc) {
                top.to_lowercase()
            } else {
                top.to_owned()
            };
            if flag(&tc) { top } else { format!("{top}/x") }
        })
        .collect();
    let base = pick(&tc, &["MANIFEST.tsv", ".deleted"]);
    let name = free_name(base, &taken);
    assert!(name.starts_with(base));
    for p in &taken {
        let folded_path = pigeon_core::names::fold(p);
        let folded_name = pigeon_core::names::fold(&name);
        assert!(
            folded_path != folded_name && !folded_path.starts_with(&format!("{folded_name}/")),
            "{p} {name}"
        );
    }
}
