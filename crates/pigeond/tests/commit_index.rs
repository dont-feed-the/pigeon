//! Index contention while committing a roster change (09 §9.7): a lock
//! that clears permits the commit, while a held lock leaves main untouched.
#![allow(clippy::unwrap_used, reason = "tests")]

use std::time::Duration;

use pigeon_git::GitEnv;
use pigeond::change::{commit_index, stage_file};
use pigeond::git::Git;

async fn staged() -> (tempfile::TempDir, Git) {
    let dir = tempfile::tempdir().unwrap();
    let git = Git::new(dir.path(), GitEnv::default());
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "Index test"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "index@example.invalid"])
        .await
        .unwrap();
    stage_file(&git, None, "roster", b"intended\n")
        .await
        .unwrap();
    (dir, git)
}

#[tokio::test]
async fn a_temporary_index_lock_delays_the_commit_without_losing_the_staged_tree() {
    let (dir, git) = staged().await;
    let lock = dir.path().join(".git/index.lock");
    std::fs::write(&lock, b"another writer").unwrap();
    let commit = commit_index(&git, None, "roster");
    tokio::pin!(commit);
    assert!(
        tokio::time::timeout(Duration::from_millis(150), &mut commit)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&lock).unwrap(), b"another writer");
    assert_eq!(git.rev("refs/heads/main").await, None);
    std::fs::remove_file(&lock).unwrap();
    let oid = tokio::time::timeout(Duration::from_secs(10), &mut commit)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        git.rev("refs/heads/main").await.as_deref(),
        Some(oid.as_str())
    );
    assert_eq!(git.blob("main:roster").await.unwrap(), b"intended\n");
    assert_eq!(
        git.run(&["rev-list", "--count", "main"])
            .await
            .unwrap()
            .trim(),
        "1"
    );
}

#[tokio::test]
async fn a_persistent_index_lock_is_reported_without_removing_it_or_moving_main() {
    let (dir, git) = staged().await;
    let lock = dir.path().join(".git/index.lock");
    std::fs::write(&lock, b"another writer").unwrap();
    let result = tokio::time::timeout(Duration::from_secs(10), commit_index(&git, None, "roster"))
        .await
        .unwrap();
    assert!(matches!(result, Err(pigeon_git::Error::Failed { .. })));
    assert_eq!(std::fs::read(&lock).unwrap(), b"another writer");
    assert_eq!(git.rev("refs/heads/main").await, None);
}
