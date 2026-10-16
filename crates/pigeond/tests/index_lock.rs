//! Real Git index-lock contention: wait only for atomic index updates,
//! never remove somebody else's lock, and keep the caller's deadline.
#![allow(clippy::unwrap_used, reason = "real Git lock fixtures")]

use std::time::{Duration, Instant};

use pigeon_git::GitEnv;
use pigeond::git::Git;

async fn repository() -> (tempfile::TempDir, Git, String) {
    let temp = tempfile::tempdir().unwrap();
    let git = Git::new(temp.path(), GitEnv::default());
    git.run(&["init", "--bare", "-q"]).await.unwrap();
    let blob = git
        .run_in(
            &["hash-object", "-w", "--stdin"],
            b"exact saved bytes\0\xff\n",
        )
        .await
        .unwrap()
        .trim()
        .to_owned();
    (temp, git, blob)
}

#[tokio::test]
async fn an_atomic_index_update_waits_for_a_real_transient_lock() {
    let (temp, git, blob) = repository().await;
    let lock = temp.path().join("index.lock");
    std::fs::write(&lock, b"another writer owns this lock\n").unwrap();
    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(
            std::fs::read(&lock).unwrap(),
            b"another writer owns this lock\n"
        );
        std::fs::remove_file(lock).unwrap();
    });
    let info = format!("100644,{blob},saved.txt");
    let result = git
        .run(&["update-index", "--add", "--cacheinfo", &info])
        .await;
    release.await.unwrap();
    result.unwrap();
    assert_eq!(git.index().await.unwrap()["saved.txt"].oid, blob);
    assert!(git.rev("refs/heads/main").await.is_none());
}

#[tokio::test]
async fn a_persistent_lock_is_preserved_and_the_operation_stops_within_its_budget() {
    let (temp, git, blob) = repository().await;
    let lock = temp.path().join("index.lock");
    let owner = b"persistent unrelated writer\n";
    std::fs::write(&lock, owner).unwrap();
    let info = format!("100644,{blob},saved.txt");
    let started = Instant::now();
    assert!(
        git.run_for(
            &["update-index", "--add", "--cacheinfo", &info],
            Duration::from_millis(200)
        )
        .await
        .is_err()
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(std::fs::read(&lock).unwrap(), owner);
    assert!(git.index().await.unwrap().is_empty());
}

#[tokio::test]
async fn write_tree_keeps_the_same_transient_lock_handling_after_centralization() {
    let (temp, git, blob) = repository().await;
    let info = format!("100644,{blob},saved.txt");
    git.run(&["update-index", "--add", "--cacheinfo", &info])
        .await
        .unwrap();
    let lock = temp.path().join("index.lock");
    std::fs::write(&lock, b"tree writer owns this lock\n").unwrap();
    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(
            std::fs::read(&lock).unwrap(),
            b"tree writer owns this lock\n"
        );
        std::fs::remove_file(lock).unwrap();
    });
    let result = git.run(&["write-tree"]).await;
    release.await.unwrap();
    let tree = result.unwrap();
    assert_eq!(git.tree(tree.trim()).await.unwrap()["saved.txt"].oid, blob);
    assert!(git.rev("refs/heads/main").await.is_none());
}
