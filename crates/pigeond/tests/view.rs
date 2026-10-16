//! `pigeon view PATH [--version N | --group]` (07 §7.6, ADR 0035): a
//! read-only copy of a version, outside the folder, named after it.
//!
//! Needs what `tests/slice.rs` needs.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use std::os::unix::fs::PermissionsExt;

use common::{SERIAL, ask, dirs, eventually_within, pigeon, said, start, until};
use pigeon_proto::api::State;
use pigeon_proto::{Request, Response};

#[test]
fn a_version_opens_as_a_read_only_copy_outside_the_folder() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let a1 = start(&root, sockets.path(), "a1");
    let all = [&a1];
    let fa = root.join("ana");
    std::fs::create_dir_all(&fa).unwrap();
    std::fs::write(fa.join("tarte.txt"), "first\n").unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    until(&a1, State::Local, &all);
    std::fs::write(fa.join("tarte.txt"), "second\n").unwrap();
    let file = fa.join("tarte.txt");
    let file = file.to_str().unwrap();
    let copy = |args: &[&str]| -> Option<String> {
        let mut a = vec!["view", file];
        a.extend_from_slice(args);
        said(&pigeon(&a1, &a), "view-ready", "path")
    };
    eventually_within(
        120,
        || copy(&[]).is_some_and(|p| std::fs::read_to_string(p).is_ok_and(|t| t == "second\n")),
        "the second version was never captured",
        &all,
    );
    let first = copy(&["--version", "1"]).unwrap();
    assert!(first.ends_with("tarte (version 1).txt"), "{first}");
    assert!(!first.starts_with(f), "outside the folder: {first}");
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "first\n");
    let mode = std::fs::metadata(&first).unwrap().permissions().mode();
    assert_eq!(mode & 0o222, 0, "read-only");
    let group = copy(&["--group"]).unwrap();
    assert!(group.ends_with("tarte (group's version).txt"), "{group}");
    assert_eq!(std::fs::read_to_string(&group).unwrap(), "second\n");
    assert_eq!(std::fs::read_to_string(file).unwrap(), "second\n");
    let none = pigeon(&a1, &["view", file, "--version", "9"]);
    assert!(
        none.iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "no-such-version")),
        "{none:?}"
    );
}

/// `ReadText`, the web's README: the start of the accepted version, read
/// from this computer, and nothing outside the group's annexed files.
#[test]
fn read_text_answers_the_start_of_an_accepted_file_and_nothing_else() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let a1 = start(&root, sockets.path(), "a1");
    let all = [&a1];
    let fa = root.join("ana");
    std::fs::create_dir_all(fa.join("docs")).unwrap();
    std::fs::write(fa.join("docs/README.md"), "# Docs\n\nHello, world.\n").unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    until(&a1, State::Local, &all);
    let read = |path: String, limit: u64| ask(&a1, &Request::ReadText { path, limit });
    let readme = format!("{f}/docs/README.md");
    eventually_within(
        120,
        || {
            read(readme.clone(), 1000).iter().any(|r| {
                matches!(r, Response::Text { text, size: 22, truncated: false }
                    if text == "# Docs\n\nHello, world.\n")
            })
        },
        "the README was never readable",
        &all,
    );
    let listed = ask(
        &a1,
        &Request::Files {
            folder: format!("{f}/docs"),
        },
    );
    assert!(
        matches!(listed.as_slice(), [Response::Files { files, .. }]
            if files.iter().any(|e| e.path == "docs/README.md"
                && e.size == Some(22) && e.modified.is_some())),
        "the listing has the README's size and time: {listed:?}"
    );
    let start = read(readme.clone(), 6);
    assert!(
        matches!(start.as_slice(), [Response::Text { text, size: 22, truncated: true }] if text == "# Docs"),
        "{start:?}"
    );
    for refused in [format!("{f}/.pigeon/roster.toml"), format!("{f}/docs")] {
        let answer = read(refused, 1000);
        assert!(
            matches!(answer.as_slice(), [Response::Problem(p)] if p.code == "no-such-version"),
            "{answer:?}"
        );
    }
    let outside = read(
        root.join("elsewhere.txt").to_string_lossy().into_owned(),
        10,
    );
    assert!(
        matches!(outside.as_slice(), [Response::Problem(p)] if p.code == "not-in-group"),
        "{outside:?}"
    );
}
