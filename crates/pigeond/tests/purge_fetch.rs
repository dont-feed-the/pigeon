//! A quarantined real Git fetch cannot change retained refs, even when a
//! remote's configured refmap names local branches. Adoption is separate.
#![allow(clippy::unwrap_used, reason = "real Git quarantine fixture")]

use std::time::Duration;

use pigeon_core::filter::ContentPolicy;
use pigeon_git::GitEnv;
use pigeond::git::Git;

async fn repository(path: &std::path::Path, message: &str) -> (Git, String) {
    std::fs::create_dir_all(path).unwrap();
    let git = Git::new(path, GitEnv::default());
    git.run(&["init", "--bare", "-q"]).await.unwrap();
    git.run(&["config", "user.name", "quarantine fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "fixture@example.invalid"])
        .await
        .unwrap();
    let tree = git.run(&["write-tree"]).await.unwrap();
    let head = git.commit_tree(tree.trim(), &[], message).await.unwrap();
    git.update_ref("refs/heads/main", &head, None)
        .await
        .unwrap();
    (git, head)
}

#[tokio::test]
async fn metadata_fetch_keeps_the_retained_reading_and_ignores_configured_refmaps() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.git");
    let (remote, new) = repository(&source, "incoming history").await;
    remote
        .update_ref("refs/heads/extra", &new, None)
        .await
        .unwrap();
    let (local, old) = repository(&temp.path().join("local.git"), "accepted history").await;
    local
        .run(&["config", "remote.peer.url", source.to_str().unwrap()])
        .await
        .unwrap();
    local
        .run(&["config", "remote.peer.fetch", "+refs/heads/*:refs/heads/*"])
        .await
        .unwrap();
    let gate = &local.purge_gate;
    gate.arm();
    assert!(gate.publish(gate.revision().unwrap(), ContentPolicy::default()));
    let erase = gate.erasure().await;
    let args = [
        "fetch",
        "-q",
        "--no-tags",
        "--no-write-fetch-head",
        "--refmap=",
        "peer",
        "+refs/heads/*:refs/pigeon/peers/peer/*",
    ];
    tokio::time::timeout(Duration::from_secs(10), local.run(&args))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        local.rev("refs/heads/main").await.as_deref(),
        Some(old.as_str())
    );
    assert!(local.rev("refs/heads/extra").await.is_none());
    assert_eq!(
        local.rev("refs/pigeon/peers/peer/main").await.as_deref(),
        Some(new.as_str())
    );
    assert!(!gate.policy().waiting);
    drop(erase);
    local
        .update_ref("refs/heads/main", &new, Some(&old))
        .await
        .unwrap();
    assert!(
        gate.policy().waiting,
        "adoption must invalidate content authorization"
    );
}
