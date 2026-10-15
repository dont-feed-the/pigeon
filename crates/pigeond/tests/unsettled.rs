//! A step that fails leaves its transition as a crash would, and the next
//! round settles it before capture (07 §7.5). The S9 schedule that found it:
//! `free` moved a file aside, then found `index.lock` and stopped before the
//! skip-worktree bit; capture read the missing file as the person's deletion
//! and committed it once the lock went (slice run 38739932675, macOS).
//!
//! Needs git-annex on PATH, and `pigeon` and `git-annex-p2p-iroh` built next
//! to `pigeond`. Skipped without git-annex unless `PIGEON_REQUIRE_GIT_ANNEX`
//! is set, as in CI.
#![cfg(feature = "lab")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
#![allow(clippy::print_stderr, reason = "tests")]

mod common;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use common::{SERIAL, dirs, eventually, join, logs, pigeon, said, start, until};
use pigeon_proto::api::State;

/// `git args…` in `dir`, its output.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn a_free_that_finds_a_lock_deletes_nothing() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let s = sockets.path();
    let (lap, c1) = (start(&root, s, "lap"), start(&root, s, "c1"));
    let all = [&lap, &c1];
    let folder = root.join("folder");
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    std::fs::write(folder.join("sub/a.txt"), b"kept, never deleted\n").unwrap();
    let f = folder.to_str().unwrap();
    let created = pigeon(&lap, &["create", "Family", f, "--k", "1", "--name", "lap"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&lap, &c1, &all);
    until(&lap, State::Protected, &all);

    // A fresh lock, as a git call that a crash killed leaves: pigeond
    // removes only stale ones, so every git call that writes the index
    // fails until the test removes it.
    let lock = folder.join(".git/index.lock");
    std::fs::write(&lock, b"").unwrap();
    let sub = folder.join("sub");
    let freed = pigeon(&lap, &["free", sub.to_str().unwrap()]);
    assert_eq!(
        said(&freed, "free-result", "safe").as_deref(),
        Some("1"),
        "{freed:?}"
    );
    eventually(
        || !sub.join("a.txt").exists(),
        "a.txt was never moved aside",
        &all,
    );
    std::thread::sleep(Duration::from_secs(5));
    std::fs::remove_file(&lock).unwrap();
    eventually(
        || git(&folder, &["ls-files", "-v", "--", "sub/a.txt"]).starts_with("S "),
        "a.txt was never hidden",
        &all,
    );
    std::thread::sleep(Duration::from_secs(3));
    let deleted = git(
        &folder,
        &["log", "--diff-filter=D", "--name-only", "--format=", "main"],
    );
    assert!(
        deleted.trim().is_empty(),
        "main deleted {deleted}\n{}",
        logs(&all)
    );
    let tree = git(&folder, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(
        tree.lines().any(|l| l == "sub/a.txt"),
        "{tree}\n{}",
        logs(&all)
    );
    assert!(!sub.join("a.txt").exists(), "a.txt came back as a file");
}
