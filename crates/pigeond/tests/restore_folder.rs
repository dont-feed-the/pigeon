//! Real deleted-folder recovery (10 §10.1): coherent snapshots, nested
//! paths, binary bytes, executable modes and newer files/history survive.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "integration fixtures")]
mod common;
use common::*;
use pigeon_proto::{Arg, Response};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn versions(d: &Daemon, path: &Path) -> Vec<pigeon_proto::api::VersionStatus> {
    pigeon(d, &["history", path.to_str().unwrap()])
        .into_iter()
        .find_map(|r| match r {
            Response::History { versions } => Some(versions),
            _ => None,
        })
        .unwrap_or_default()
}
fn count(d: &Daemon, path: &Path) -> usize {
    versions(d, path).len()
}

#[test]
fn deleted_folder_and_time_snapshot_keep_newer_files_and_executable_mode() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let folder = root.join("Family");
    let gone = folder.join("Memories");
    std::fs::create_dir_all(gone.join("deep")).unwrap();
    let a = gone.join("a.txt");
    let b = gone.join("deep/b.bin");
    let script = gone.join("run.sh");
    std::fs::write(&a, b"caf\xc3\xa9\r\nlast line").unwrap();
    std::fs::write(&b, b"first\0\xff\r\n").unwrap();
    std::fs::write(&script, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let answer = pigeon(
        &lap,
        &[
            "create",
            "Family",
            folder.to_str().unwrap(),
            "--k",
            "1",
            "--name",
            "laptop",
        ],
    );
    assert!(said(&answer, "created", "group").is_some(), "{answer:?}");
    eventually_within(
        60,
        || count(&lap, &a) == 1 && count(&lap, &b) == 1 && count(&lap, &script) == 1,
        "initial snapshot missing",
        &[&lap],
    );
    let first_at = versions(&lap, &b)[0].saved_at.clone();
    std::thread::sleep(std::time::Duration::from_secs(2));
    std::fs::write(&b, b"second\0bytes\xff").unwrap();
    eventually_within(
        60,
        || count(&lap, &b) == 2,
        "second snapshot missing",
        &[&lap],
    );
    std::fs::remove_dir_all(&gone).unwrap();
    let batch = || {
        pigeon(&lap, &["held"]).into_iter().find_map(|r| match r {
            Response::Held { held } => held.first().map(|h| h.id.clone()),
            _ => None,
        })
    };
    eventually_within(
        60,
        || batch().is_some(),
        "whole-folder deletion not held",
        &[&lap],
    );
    let shared = pigeon(&lap, &["held", &batch().unwrap(), "--share", "--yes"]);
    assert!(
        !shared.iter().any(|r| matches!(r, Response::Problem(_))),
        "{shared:?}"
    );
    eventually_within(
        60,
        || {
            pigeon(&lap, &["deleted"])
                .iter()
                .any(|r| matches!(r, Response::Deleted { files, .. } if files.len() == 3))
        },
        "deletion not accepted",
        &[&lap],
    );
    let restored = pigeon(&lap, &["restore", gone.to_str().unwrap()]);
    assert!(
        !restored.iter().any(|r| matches!(r, Response::Problem(_))),
        "{restored:?}"
    );
    assert!(
        said(&restored, "restore-folder-result", "path").is_some(),
        "{restored:?}"
    );
    assert_eq!(std::fs::read(&a).unwrap(), b"caf\xc3\xa9\r\nlast line");
    assert_eq!(std::fs::read(&b).unwrap(), b"second\0bytes\xff");
    assert_ne!(
        std::fs::metadata(&script).unwrap().permissions().mode() & 0o111,
        0
    );

    // A mode-only save is a distinct version, with unchanged content.
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o644)).unwrap();
    eventually_within(
        60,
        || count(&lap, &script) == 2,
        "mode-only version missing",
        &[&lap],
    );
    let executable = pigeon(
        &lap,
        &["restore", script.to_str().unwrap(), "--version", "1"],
    );
    assert!(
        !executable.iter().any(|r| matches!(r, Response::Problem(_))),
        "{executable:?}"
    );
    assert_ne!(
        std::fs::metadata(&script).unwrap().permissions().mode() & 0o111,
        0
    );
    assert_eq!(std::fs::read(&script).unwrap(), b"#!/bin/sh\nexit 0\n");

    // Explicitly restoring the earlier moment keeps unmentioned new files.
    std::fs::write(gone.join("newer-only"), b"keep this new file").unwrap();
    let earlier = pigeon(
        &lap,
        &["restore", gone.to_str().unwrap(), "--at", &first_at],
    );
    assert!(
        !earlier.iter().any(|r| matches!(r, Response::Problem(_))),
        "{earlier:?}"
    );
    assert_eq!(std::fs::read(&b).unwrap(), b"first\0\xff\r\n");
    assert_eq!(
        std::fs::read(gone.join("newer-only")).unwrap(),
        b"keep this new file"
    );
    let newer = pigeon(&lap, &["view", b.to_str().unwrap(), "--version", "2"]);
    let view = said(&newer, "view-ready", "path").unwrap();
    assert_eq!(std::fs::read(view).unwrap(), b"second\0bytes\xff");

    // An open writer wins. Partial completion must say which files remain.
    let writer = std::fs::OpenOptions::new().write(true).open(&a).unwrap();
    std::fs::write(&a, b"unpublished editor bytes\0").unwrap();
    let partial = pigeon(
        &lap,
        &["restore", gone.to_str().unwrap(), "--at", &first_at],
    );
    assert!(
        partial
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "restore-folder-incomplete" && p.message.args.get("skipped") == Some(&Arg::Number(1)))),
        "{partial:?}"
    );
    assert_eq!(std::fs::read(&a).unwrap(), b"unpublished editor bytes\0");
    drop(writer);

    // A symlink in a deleted path cannot redirect a restore out of the group.
    let outside = root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("b.bin"), b"outside sentinel").unwrap();
    std::fs::remove_dir_all(gone.join("deep")).unwrap();
    std::os::unix::fs::symlink(&outside, gone.join("deep")).unwrap();
    let rejected = pigeon(&lap, &["restore", b.to_str().unwrap(), "--version", "1"]);
    assert!(
        rejected.iter().any(|r| matches!(r, Response::Problem(_))),
        "{rejected:?}"
    );
    assert_eq!(
        std::fs::read(outside.join("b.bin")).unwrap(),
        b"outside sentinel"
    );
}
