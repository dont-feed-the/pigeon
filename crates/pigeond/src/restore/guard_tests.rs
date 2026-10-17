//! Deterministic interleavings of the real capture and restore drivers. Saves,
//! deletions and executable-bit changes during a yielded scan remain visible
//! and indexed. No elapsed retention interval or filesystem race is simulated.
#![allow(clippy::unwrap_used, reason = "real git-annex restore fixture")]

use super::*;
use crate::{capture, git::Git, group::Group, install::Put, trace::Tracer};
use std::os::unix::fs::PermissionsExt;

struct Fixture {
    _temp: tempfile::TempDir,
    g: Group,
    trace: Tracer,
    old: String,
    current: String,
}

async fn fixture() -> Option<Fixture> {
    let temp = tempfile::tempdir().unwrap();
    let env = pigeon_git::GitEnv::default();
    let git = Git::new(temp.path(), env.clone());
    if git.run(&["annex", "version", "--raw"]).await.is_err() {
        assert!(std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none());
        return None;
    }
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "Restore fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "restore@pigeon"])
        .await
        .unwrap();
    git.run(&["annex", "init", "restore"]).await.unwrap();
    let g = Group::open(&"a".repeat(40), temp.path(), &env)
        .await
        .unwrap();
    crate::settings::check(&g).await;
    let trace = Tracer::new(temp.path().join("traces"), "restore-fixture");
    let file = temp.path().join("notes.bin");
    std::fs::write(&file, b"requested saved version\0\xff\r\n").unwrap();
    capture::capture(&g, "restore-fixture", &trace)
        .await
        .unwrap();
    let old = g.git.keys("refs/heads/main").await.unwrap()["notes.bin"].clone();
    std::fs::write(&file, b"visible latest version\0\xfe\r\n").unwrap();
    capture::capture(&g, "restore-fixture", &trace)
        .await
        .unwrap();
    let current = g.git.keys("refs/heads/main").await.unwrap()["notes.bin"].clone();
    assert_ne!(old, current);
    Some(Fixture {
        _temp: temp,
        g,
        trace,
        old,
        current,
    })
}

async fn index(f: &Fixture) -> String {
    f.g.git.run(&["write-tree"]).await.unwrap()
}

#[tokio::test]
async fn a_newer_captured_save_is_not_the_restores_replacement_baseline() {
    let Some(f) = fixture().await else { return };
    let file = f.g.worktree.as_ref().unwrap().join("notes.bin");
    let before = Before::read(&file, Some(&f.current), &f.old).unwrap();
    assert!(before.clear(&file));
    let private = b"new private save made during the cold scan\0\xfd\n";
    std::fs::write(&file, private).unwrap();
    capture::capture(&f.g, "restore-fixture", &f.trace)
        .await
        .unwrap();
    let indexed = index(&f).await;
    assert_ne!(
        f.g.git.keys("refs/heads/main").await.unwrap()["notes.bin"],
        f.current
    );
    let result = crate::install::put_restore(
        &f.g,
        "notes.bin",
        &f.old,
        before.source(),
        "100644",
        &f.trace,
    )
    .await
    .unwrap();
    assert_eq!(result, Put::Raced);
    assert_eq!(std::fs::read(&file).unwrap(), private);
    assert_eq!(index(&f).await, indexed);
    assert!(verified(&file, &f.old, "100644").is_none());
    let reopened = Group::open(
        &f.g.id,
        f.g.worktree.as_ref().unwrap(),
        &pigeon_git::GitEnv::default(),
    )
    .await
    .unwrap();
    assert_eq!(reopened.git.run(&["write-tree"]).await.unwrap(), indexed);
    assert_eq!(std::fs::read(&file).unwrap(), private);
}

#[tokio::test]
async fn a_captured_deletion_during_the_scan_is_not_undone() {
    let Some(f) = fixture().await else { return };
    let file = f.g.worktree.as_ref().unwrap().join("notes.bin");
    let before = Before::read(&file, Some(&f.current), &f.old).unwrap();
    std::fs::remove_file(&file).unwrap();
    capture::capture(&f.g, "restore-fixture", &f.trace)
        .await
        .unwrap();
    let indexed = index(&f).await;
    assert!(!f.g.git.index().await.unwrap().contains_key("notes.bin"));
    let result = crate::install::put_restore(
        &f.g,
        "notes.bin",
        &f.old,
        before.source(),
        "100644",
        &f.trace,
    )
    .await
    .unwrap();
    assert_eq!(result, Put::Raced);
    assert!(!file.exists());
    assert_eq!(index(&f).await, indexed);
}

#[tokio::test]
async fn an_executable_bit_change_survives_even_when_the_bytes_stay_the_same() {
    let Some(f) = fixture().await else { return };
    let file = f.g.worktree.as_ref().unwrap().join("notes.bin");
    let before = Before::read(&file, Some(&f.current), &f.old).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    capture::capture(&f.g, "restore-fixture", &f.trace)
        .await
        .unwrap();
    let indexed = index(&f).await;
    let result = crate::install::put_restore(
        &f.g,
        "notes.bin",
        &f.old,
        before.source(),
        "100644",
        &f.trace,
    )
    .await
    .unwrap();
    assert_eq!(result, Put::Raced);
    assert_eq!(
        std::fs::read(&file).unwrap(),
        b"visible latest version\0\xfe\r\n"
    );
    assert_ne!(file.metadata().unwrap().permissions().mode() & 0o100, 0);
    assert_eq!(index(&f).await, indexed);
}

#[tokio::test]
async fn an_unchanged_baseline_installs_and_only_exact_bytes_and_mode_can_bind() {
    let Some(f) = fixture().await else { return };
    let file = f.g.worktree.as_ref().unwrap().join("notes.bin");
    let before = Before::read(&file, Some(&f.current), &f.old).unwrap();
    let result = crate::install::put_restore(
        &f.g,
        "notes.bin",
        &f.old,
        before.source(),
        "100644",
        &f.trace,
    )
    .await
    .unwrap();
    assert!(matches!(result, Put::Done(_)));
    assert!(verified(&file, &f.old, "100644").is_some());
    assert!(verified(&file, &f.old, "100755").is_none());
    std::fs::write(&file, b"a later private edit").unwrap();
    assert!(verified(&file, &f.old, "100644").is_none());
}
