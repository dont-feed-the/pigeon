//! Atomic index commands tolerate a short foreign lock (13 §13.6),
//! bounded by the existing retry window and caller deadline. The owner
//! alone removes its lock; a persistent lock is preserved byte for byte.
#![allow(clippy::unwrap_used, reason = "real Git index-lock fixture")]

use pigeon_git::{GitEnv, Repo};
use std::time::{Duration, Instant};

async fn fixture() -> (tempfile::TempDir, Repo, String) {
    let root = tempfile::tempdir().unwrap();
    let repo = Repo::new(root.path(), GitEnv::default());
    repo.run(&["init", "-q"], None, Duration::from_secs(5))
        .await
        .unwrap();
    let tree = repo
        .run(&["mktree"], Some(b""), Duration::from_secs(5))
        .await
        .unwrap()
        .text()
        .trim()
        .to_owned();
    (root, repo, tree)
}

#[tokio::test]
async fn read_tree_waits_for_the_lock_owner_then_updates_the_index() {
    let (root, repo, tree) = fixture().await;
    let lock = root.path().join(".git/index.lock");
    std::fs::write(&lock, b"foreign lock").unwrap();
    let owner = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(std::fs::read(&lock).unwrap(), b"foreign lock");
        std::fs::remove_file(lock).unwrap();
    });
    repo.run(
        &["read-tree", "--reset", &tree],
        None,
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    owner.join().unwrap();
    assert!(root.path().join(".git/index").is_file());
}

#[tokio::test]
async fn read_tree_preserves_a_persistent_lock_and_respects_the_callers_deadline() {
    let (root, repo, tree) = fixture().await;
    let lock = root.path().join(".git/index.lock");
    std::fs::write(&lock, b"foreign lock").unwrap();
    let start = Instant::now();
    assert!(
        repo.run(
            &["read-tree", "--reset", &tree],
            None,
            Duration::from_millis(200)
        )
        .await
        .is_err()
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(std::fs::read(lock).unwrap(), b"foreign lock");
    assert!(!root.path().join(".git/index").exists());
}
