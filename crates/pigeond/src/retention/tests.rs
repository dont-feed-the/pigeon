//! Historical protection follows shared retained history, including sibling
//! merges. Equal bytes on an unrelated branch do not prove the same history.
#![allow(clippy::unwrap_used, reason = "native immutable Git DAG fixture")]

use super::*;
use crate::git::Git;

async fn commit(git: &Git, parents: &[&str], key: &str, label: &str) -> String {
    let blob = git
        .run_in(
            &["hash-object", "-w", "--stdin"],
            format!("/annex/objects/{key}\n").as_bytes(),
        )
        .await
        .unwrap();
    let tree = git
        .run_in(
            &["mktree"],
            format!("100644 blob {}\trecipe.txt\n", blob.trim()).as_bytes(),
        )
        .await
        .unwrap();
    git.commit_tree(tree.trim(), parents, label).await.unwrap()
}

async fn fixture() -> (tempfile::TempDir, Git, String, String) {
    let temp = tempfile::tempdir().unwrap();
    let git = Git::new(temp.path(), pigeon_git::GitEnv::default());
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "Retained history"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "history@pigeon"])
        .await
        .unwrap();
    let old = format!("SHA256E-s11--{}.txt", "a".repeat(64));
    let new = format!("SHA256E-s13--{}.txt", "b".repeat(64));
    (temp, git, old, new)
}

#[tokio::test]
async fn sibling_merges_prove_their_shared_overwritten_version() {
    let (_temp, git, old, new) = fixture().await;
    let first = commit(&git, &[], &old, "original capture").await;
    let second = commit(&git, &[&first], &new, "later capture").await;
    let left = commit(&git, &[&first, &second], &new, "laptop merge").await;
    let right = commit(&git, &[&first, &second], &new, "backup merge").await;
    assert_ne!(left, right);
    assert!(!git.contains(&right, &left).await);
    let retained = git
        .retained_keys(std::slice::from_ref(&left))
        .await
        .unwrap();
    let named = named_keys(&git, &[(&left, retained)], &right, false)
        .await
        .unwrap();
    assert!(
        named.contains(&old),
        "a sibling merge retains the original capture"
    );
    assert!(named.contains(&new));
}

#[tokio::test]
async fn a_peer_without_the_capture_cannot_prove_its_retention() {
    let (_temp, git, old, new) = fixture().await;
    let first = commit(&git, &[], &old, "original capture").await;
    let second = commit(&git, &[&first], &new, "local capture only").await;
    let retained = git
        .retained_keys(std::slice::from_ref(&second))
        .await
        .unwrap();
    let named = named_keys(&git, &[(&second, retained)], &first, false)
        .await
        .unwrap();
    assert!(!named.contains(&new));
}

#[tokio::test]
async fn unrelated_identical_bytes_do_not_prove_a_shared_reference() {
    let (_temp, git, old, _) = fixture().await;
    let left = commit(&git, &[], &old, "our capture").await;
    let right = commit(&git, &[], &old, "unrelated capture").await;
    let retained = git
        .retained_keys(std::slice::from_ref(&left))
        .await
        .unwrap();
    assert!(
        named_keys(&git, &[(&left, retained)], &right, false)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn every_common_ancestor_counts_and_unreadable_ancestry_stays_unknown() {
    let (_temp, git, old, new) = fixture().await;
    let base = commit(&git, &[], &old, "shared original").await;
    let a = commit(&git, &[&base], &old, "one branch").await;
    let b = commit(&git, &[&base], &new, "another branch").await;
    let left = commit(&git, &[&a, &b], &new, "laptop merge").await;
    let right = commit(&git, &[&b, &a], &new, "backup merge").await;
    let retained = git
        .retained_keys(std::slice::from_ref(&left))
        .await
        .unwrap();
    let ours = [(&left, retained)];
    let named = named_keys(&git, &ours, &right, false).await.unwrap();
    assert!(named.contains(&old) && named.contains(&new));
    assert!(
        named_keys(&git, &ours, &"f".repeat(40), false)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn purge_history_keeps_fresh_references_from_borrowing_an_old_capture() {
    let (_temp, git, old, new) = fixture().await;
    let first = commit(&git, &[], &old, "original capture").await;
    let other = commit(&git, &[&first], &new, "overwrite before erasure").await;
    let fresh = commit(&git, &[&other], &old, "fresh reference after erasure").await;
    let tip = commit(&git, &[&fresh], &new, "fresh reference is now historical").await;
    let retained = git.retained_keys(std::slice::from_ref(&tip)).await.unwrap();
    let ours = [(&tip, retained)];
    // Shared old history alone is not an acknowledgement of the new birth.
    assert!(
        named_keys(&git, &ours, &first, false)
            .await
            .unwrap()
            .contains(&old)
    );
    assert!(
        !named_keys(&git, &ours, &first, true)
            .await
            .unwrap()
            .contains(&old)
    );
    assert!(
        named_keys(&git, &ours, &tip, true)
            .await
            .unwrap()
            .contains(&old)
    );
}
