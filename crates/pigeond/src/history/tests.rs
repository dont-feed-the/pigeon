//! Bare read-only catalogue paths do not need real parent folders and
//! cannot escape the repository through `..` (09 §9.9, 10 §10.2).
#![allow(clippy::unwrap_used, reason = "tests")]

use super::*;

#[tokio::test]
async fn bare_history_accepts_missing_nested_paths_and_rejects_escapes() {
    let dir = tempfile::tempdir().unwrap();
    let git = crate::git::Git::new(dir.path(), pigeon_git::GitEnv::default());
    git.run(&["init", "--bare", "-q", "-b", "main"])
        .await
        .unwrap();
    let g = Group::open("fixture", dir.path(), &pigeon_git::GitEnv::default())
        .await
        .unwrap();
    assert_eq!(
        relative(&g, &g.git_dir.join("deleted/sub/file.txt")).as_deref(),
        Some("deleted/sub/file.txt")
    );
    assert_eq!(relative(&g, &g.git_dir.join("../outside")), None);
    assert_eq!(relative(&g, &g.git_dir.join("sub/../../outside")), None);
    assert_eq!(relative(&g, &g.git_dir), None);
    assert_eq!(relative(&g, Path::new("/outside/file.txt")), None);
}
