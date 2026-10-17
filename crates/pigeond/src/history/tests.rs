//! Bare and sparse catalogue paths do not need real parent folders and
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

#[tokio::test]
async fn sparse_catalogue_paths_allow_absent_parents_without_following_links_or_escapes() {
    let dir = tempfile::tempdir().unwrap();
    let git = crate::git::Git::new(dir.path(), pigeon_git::GitEnv::default());
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    let mut g = Group::open("fixture", dir.path(), &pigeon_git::GitEnv::default())
        .await
        .unwrap();
    g.shared = Some(crate::config::BridgeStorage {
        store: dir.path().join("pool"),
        source_socket: dir.path().join("source.sock"),
    });
    let root = g.worktree.as_ref().unwrap();
    for path in ["Photos", "Photos/holiday.jpg", "Photos/nested/file.txt"] {
        assert_eq!(relative(&g, &root.join(path)).as_deref(), Some(path));
    }
    for path in [
        "../outside",
        "Photos/../../outside",
        ".git/config",
        ".pigeon/private",
        ".gitattributes",
    ] {
        assert_eq!(relative(&g, &root.join(path)), None, "{path}");
    }
    std::os::unix::fs::symlink(dir.path(), root.join("link")).unwrap();
    assert_eq!(relative(&g, &root.join("link/missing/file.txt")), None);
    let alias_dir = tempfile::tempdir().unwrap();
    let alias = alias_dir.path().join("files");
    std::os::unix::fs::symlink(root, &alias).unwrap();
    assert_eq!(
        relative(&g, &alias.join("missing/file.txt")).as_deref(),
        Some("missing/file.txt")
    );
    assert_eq!(relative(&g, &alias.join("link/missing/file.txt")), None);
    std::fs::create_dir_all(root.join("nested/.git")).unwrap();
    assert_eq!(relative(&g, &root.join("nested/missing/file.txt")), None);
}
