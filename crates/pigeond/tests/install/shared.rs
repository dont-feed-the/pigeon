//! Accepted sparse files follow their merged catalogue and conflict copies;
//! only a real pending working file can keep a local divergence at the old name.

use super::*;

#[tokio::test]
async fn sparse_accepted_conflict_does_not_create_a_missing_local_overlay() {
    let Some(mut l) = laptop().await else { return };
    write(&l, "a.txt", b"one\n");
    let base = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&base).await.unwrap();
    let theirs = version(&l, "a.txt", b"two\n", true).await;
    tree.insert("a.txt".into(), theirs.clone());
    bens(&l, &tree, &base, 60).await;
    write(&l, "a.txt", b"mine\n");
    let merged = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let tree = l.git.tree(&merged).await.unwrap();
    let (copy, mine) = tree
        .iter()
        .find(|(p, _)| pigeon_core::names::is_copy_of(p, "a.txt"))
        .unwrap();
    assert_eq!(l.git.index().await.unwrap().get("a.txt"), Some(mine));
    // The accepted upload was compacted before its competing history arrived.
    l.g.shared = Some(pigeond::config::BridgeStorage {
        store: l.dir.path().join("pool"),
        source_socket: l.dir.path().join("offline.sock"),
    });
    std::fs::remove_file(l.wt.join("a.txt")).unwrap();
    l.git
        .run(&["update-index", "--skip-worktree", "--", "a.txt"])
        .await
        .unwrap();
    install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert!(
        pigeond::conflicts_file::read(&l.g)
            .unwrap()
            .paths
            .is_empty(),
        "An absent accepted file has no live pending save to preserve"
    );
    let index = l.git.index().await.unwrap();
    assert_eq!(index.get("a.txt"), Some(&theirs));
    assert_eq!(index.get(copy), Some(mine));
    assert!(!l.wt.join("a.txt").exists());
    assert!(!l.wt.join(copy).exists());
    assert_eq!(
        capture::capture(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .commit,
        None
    );
}
