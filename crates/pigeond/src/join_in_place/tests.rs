//! Index adoption keeps dirty and untracked user bytes (09 §9.9). A new
//! annex pointer must not turn unpublished edits into a permanent replay
//! failure, and absent content stays hidden rather than becoming deletion.
#![allow(clippy::unwrap_used, reason = "tests")]

use crate::{git::Git, group::Group};
use pigeon_git::GitEnv;
use std::os::unix::fs::MetadataExt;

async fn indexed(git: &Git, path: &str, bytes: &[u8]) {
    let blob = git
        .run_in(&["hash-object", "-w", "--stdin"], bytes)
        .await
        .unwrap();
    git.run(&[
        "update-index",
        "--add",
        "--cacheinfo",
        "100644",
        blob.trim(),
        path,
    ])
    .await
    .unwrap();
}

async fn root(git: &Git, message: &str) -> String {
    let tree = git.run(&["write-tree"]).await.unwrap();
    git.run(&["commit-tree", tree.trim(), "-m", message])
        .await
        .unwrap()
        .trim()
        .into()
}

#[tokio::test]
async fn changing_pointers_keeps_dirty_bytes_and_new_files_while_updating_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let git = Git::new(dir.path(), GitEnv::default());
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "Fixture"]).await.unwrap();
    git.run(&["config", "user.email", "fixture@example.invalid"])
        .await
        .unwrap();
    let pointer = |n: u8| {
        format!(
            "/annex/objects/SHA256E-s1--{}.txt\n",
            format!("{n:02x}").repeat(32)
        )
    };
    indexed(&git, "edited.txt", pointer(1).as_bytes()).await;
    let old = root(&git, "old").await;
    git.update_ref("refs/heads/main", &old, None).await.unwrap();
    indexed(&git, "edited.txt", pointer(2).as_bytes()).await;
    indexed(&git, "missing.txt", pointer(3).as_bytes()).await;
    indexed(
        &git,
        pigeon_core::authority::ROSTER,
        b"new verified control file\n",
    )
    .await;
    let new = root(&git, "new").await;
    git.run(&["read-tree", &old]).await.unwrap();
    let dirty = b"unpublished\r\n\0binary edits\xff";
    std::fs::write(dir.path().join("edited.txt"), dirty).unwrap();
    std::fs::write(dir.path().join("draft.bin"), b"untracked\0draft").unwrap();
    let inode = std::fs::metadata(dir.path().join("edited.txt"))
        .unwrap()
        .ino();
    let g = Group::open(&old, dir.path(), &GitEnv::default())
        .await
        .unwrap();
    super::take_tree(&g, &new).await.unwrap();
    assert_eq!(std::fs::read(dir.path().join("edited.txt")).unwrap(), dirty);
    assert_eq!(
        std::fs::metadata(dir.path().join("edited.txt"))
            .unwrap()
            .ino(),
        inode
    );
    assert_eq!(
        std::fs::read(dir.path().join("draft.bin")).unwrap(),
        b"untracked\0draft"
    );
    assert_eq!(
        git.blob(":edited.txt").await.unwrap(),
        pointer(2).as_bytes()
    );
    assert!(!dir.path().join("missing.txt").exists());
    assert!(
        git.run(&["ls-files", "-v", "--", "missing.txt"])
            .await
            .unwrap()
            .starts_with("S ")
    );
    assert_eq!(
        std::fs::read(dir.path().join(pigeon_core::authority::ROSTER)).unwrap(),
        b"new verified control file\n"
    );
    // Replay has the same result without rewriting the dirty file.
    super::take_tree(&g, &new).await.unwrap();
    assert_eq!(
        std::fs::metadata(dir.path().join("edited.txt"))
            .unwrap()
            .ino(),
        inode
    );
}
