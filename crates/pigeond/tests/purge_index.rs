//! Real Git reference births (08 §8.6): copies, restores and proposals
//! introduce references; merges inheriting any parent's key do not.
//! Changed pointer encoding and executable mode are not fresh references.
#![allow(clippy::unwrap_used, reason = "immutable Git history fixture")]

use std::collections::BTreeMap;

use pigeon_core::authority::{Entry, Tree};
use pigeon_git::GitEnv;
use pigeond::git::Git;
use pigeond::purge_index::Index;

fn key(n: u8) -> String {
    format!("SHA256E-s3--{n:064x}.txt")
}

async fn entry(git: &Git, key: &str, newline: bool) -> Entry {
    let bytes = format!("/annex/objects/{key}{}", if newline { "\n" } else { "" });
    let oid = git
        .run_in(&["hash-object", "-w", "--stdin"], bytes.as_bytes())
        .await
        .unwrap();
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: oid.trim().into(),
    }
}

async fn commit(git: &Git, tree: &Tree, parents: &[&str], label: &str) -> String {
    let tree = pigeond::merge::write_tree(git, tree).await.unwrap();
    git.commit_tree(&tree, parents, label).await.unwrap()
}

#[tokio::test]
async fn births_follow_all_parents_paths_and_keys_not_blob_identity() {
    let dir = tempfile::tempdir().unwrap();
    let git = Git::new(dir.path(), GitEnv::default());
    git.run(&["init", "--bare", "-q"]).await.unwrap();
    git.run(&["config", "user.name", "purge fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "purge@example.invalid"])
        .await
        .unwrap();
    let first = entry(&git, &key(1), true).await;
    let second = entry(&git, &key(2), true).await;
    let mut tree: Tree = [
        ("Photos/a.txt".into(), first.clone()),
        ("Photos/copy.txt".into(), first.clone()),
    ]
    .into();
    let root = commit(&git, &tree, &[], "original and copy").await;
    tree.insert("Photos/a.txt".into(), second.clone());
    let changed = commit(&git, &tree, &[&root], "new version").await;
    let mut proposal = git.tree(&root).await.unwrap();
    proposal.insert("Outside/[literal].txt".into(), first.clone());
    proposal.insert("Photos/a.txt".into(), entry(&git, &key(1), false).await);
    let proposed = commit(
        &git,
        &proposal,
        &[&root],
        "proposal copy and equivalent encoding",
    )
    .await;
    tree.remove("Photos/copy.txt");
    let deleted = commit(&git, &tree, &[&changed], "delete old copy").await;
    tree.insert("Photos/a.txt".into(), first);
    let restored = commit(&git, &tree, &[&deleted], "restore original").await;
    let mut index = Index::default();
    index
        .extend(&git, &[restored.clone(), proposed.clone()])
        .await
        .unwrap();
    let identities: BTreeMap<_, _> = index
        .references
        .iter()
        .map(|r| ((r.birth.clone(), r.path.clone()), r.key.clone()))
        .collect();
    assert_eq!(identities.len(), 5, "{identities:?}");
    assert_eq!(
        identities[&(restored.clone(), "Photos/a.txt".into())],
        key(1)
    );
    assert_eq!(
        identities[&(proposed.clone(), "Outside/[literal].txt".into())],
        key(1)
    );
    assert!(!identities.contains_key(&(proposed.clone(), "Photos/a.txt".into())));
    // The first parent has key 2; the other parent already has key 1.
    // A merge selecting key 1 is not a new restore, despite its diff.
    let mut merged_tree = git.tree(&proposed).await.unwrap();
    merged_tree.get_mut("Photos/a.txt").unwrap().mode = "100755".into();
    let merged = commit(
        &git,
        &merged_tree,
        &[&changed, &proposed],
        "inherit second parent",
    )
    .await;
    index
        .extend(&git, &[restored.clone(), merged.clone()])
        .await
        .unwrap();
    assert_eq!(index.references.len(), 5);
    let ancestors = index.ancestors(&merged).unwrap();
    assert!(
        ancestors.contains(&root) && ancestors.contains(&proposed) && ancestors.contains(&changed)
    );
    assert!(!ancestors.contains(&restored));
    let previous = index.tips.clone();
    assert!(index.extend(&git, &["f".repeat(40)]).await.is_err());
    assert_eq!(
        index.tips, previous,
        "a failed extension cannot assert completeness"
    );
}
