//! Native scheduling must use actual signed introduction dates rather
//! than treating every retained key as introduced at time zero.

use super::*;
use pigeon_core::queue::Work;

#[tokio::test]
async fn newer_unprotected_content_precedes_an_older_smaller_key() {
    let Some(l) = laptop_fixture(true).await else {
        return;
    };
    let genesis = l.git.rev("refs/heads/main").await.unwrap();
    let mut tree = l.git.tree(&genesis).await.unwrap();
    let old = version(&l, "old.txt", b"a", true).await;
    let new = version(
        &l,
        "new.txt",
        b"a larger recent save for the same backup",
        true,
    )
    .await;
    tree.insert("old.txt".into(), old);
    let first = bens(&l, &tree, &genesis, 60).await;
    tree.insert("new.txt".into(), new);
    let second = bens(&l, &tree, &first, 120).await;
    let current = l.git.keys(&second).await.unwrap();
    let keys = current.values().cloned().collect();
    let items = pigeond::admission::items(&l.g, &dev(1).0, &keys).await;
    let ordered: Vec<_> = items
        .into_iter()
        .map(|(work, _)| match work {
            Work::Protect { key, .. } => key,
            _ => panic!("not protection work"),
        })
        .collect();
    assert_eq!(ordered.len(), 2);
    assert_eq!(
        ordered[0], current["new.txt"],
        "newer content waited behind an older small key"
    );
}

async fn commit_at(l: &Laptop, tree: &Tree, parents: &[&str], at: i64) -> String {
    let root = merge::write_tree(&l.git, tree).await.unwrap();
    let parents = parents
        .iter()
        .map(|p| format!("parent {p}\n"))
        .collect::<String>();
    let t = T0 + at;
    let body = format!(
        "tree {root}\n{parents}author pigeon <pigeon@localhost> {t} +0200\ncommitter pigeon <pigeon@localhost> {t} +0200\n\npigeon: fixture\n"
    );
    let signed = sig::sign(body.as_bytes(), &key(2)).unwrap();
    let id = l
        .git
        .run_in(&["hash-object", "-t", "commit", "-w", "--stdin"], &signed)
        .await
        .unwrap()
        .trim()
        .to_owned();
    let main = l.git.rev("refs/heads/main").await;
    l.git
        .update_ref("refs/heads/main", &id, main.as_deref())
        .await
        .unwrap();
    id
}

async fn dates(
    l: &Laptop,
    keys: &std::collections::BTreeSet<String>,
) -> BTreeMap<String, pigeon_core::time::Time> {
    pigeond::admission::items(&l.g, &dev(1).0, keys)
        .await
        .into_iter()
        .map(|(work, _)| match work {
            Work::Protect {
                key, introduced, ..
            } => (key, introduced),
            _ => panic!("not protection work"),
        })
        .collect()
}

#[tokio::test]
async fn merges_preserve_birth_dates_and_removed_proposals_cannot_leave_cached_priority() {
    use pigeon_core::time::Time;
    let Some(l) = laptop_fixture(true).await else {
        return;
    };
    let genesis = l.git.rev("refs/heads/main").await.unwrap();
    let mut tree = l.git.tree(&genesis).await.unwrap();
    let old = version(&l, "old.txt", b"a", true).await;
    let new = version(&l, "new.txt", b"a larger recent save", true).await;
    tree.insert("old.txt".into(), old.clone());
    let first = commit_at(&l, &tree, &[&genesis], 60).await;
    let older_tree = tree.clone();
    tree.insert("new.txt".into(), new.clone());
    let second = commit_at(&l, &tree, &[&first], 120).await;
    let current = l.git.keys(&second).await.unwrap();
    let keys = current.values().cloned().collect();
    let original: BTreeMap<_, _> = [
        (current["old.txt"].clone(), Time(T0 + 60)),
        (current["new.txt"].clone(), Time(T0 + 120)),
    ]
    .into();
    assert_eq!(dates(&l, &keys).await, original);
    let branch = commit_at(&l, &older_tree, &[&first], 180).await;
    // The first parent lacks new.txt; the second already contains it.
    let merged = commit_at(&l, &tree, &[&branch, &second], 1000).await;
    assert_eq!(
        dates(&l, &keys).await,
        original,
        "merge fabricated recent work"
    );
    tree.insert("restored-copy.txt".into(), old);
    let restored = commit_at(&l, &tree, &[&merged], 2000).await;
    let mut expected = original;
    expected.insert(current["old.txt"].clone(), Time(T0 + 2000));
    assert_eq!(dates(&l, &keys).await, expected);
    let mut proposed = tree.clone();
    proposed.insert("proposal-copy.txt".into(), new);
    let proposal = commit_at(&l, &proposed, &[&restored], 3000).await;
    let name = format!("refs/pigeon/proposals/{}/{proposal}", dev(2).0);
    l.git.update_ref(&name, &proposal, None).await.unwrap();
    l.git
        .update_ref("refs/heads/main", &restored, Some(&proposal))
        .await
        .unwrap();
    let mut with_proposal = expected.clone();
    with_proposal.insert(current["new.txt"].clone(), Time(T0 + 3000));
    assert_eq!(dates(&l, &keys).await, with_proposal);
    l.git
        .run(&["update-ref", "-d", &name, &proposal])
        .await
        .unwrap();
    assert_eq!(
        dates(&l, &keys).await,
        expected,
        "removed proposal left stale ordering evidence"
    );
}

#[tokio::test]
async fn an_unreadable_new_tip_does_not_cache_invented_dates() {
    use pigeon_core::time::Time;
    let Some(l) = laptop_fixture(true).await else {
        return;
    };
    let genesis = l.git.rev("refs/heads/main").await.unwrap();
    let mut tree = l.git.tree(&genesis).await.unwrap();
    tree.insert("old.txt".into(), version(&l, "old.txt", b"a", true).await);
    let first = commit_at(&l, &tree, &[&genesis], 60).await;
    let original = l.git.keys(&first).await.unwrap();
    assert_eq!(
        dates(&l, &original.values().cloned().collect()).await[&original["old.txt"]],
        Time(T0 + 60)
    );
    tree.insert(
        "new.txt".into(),
        version(&l, "new.txt", b"new bytes", true).await,
    );
    let second = commit_at(&l, &tree, &[&first], 120).await;
    let current = l.git.keys(&second).await.unwrap();
    let keys = current.values().cloned().collect();
    let object =
        l.g.git_dir
            .join("objects")
            .join(&second[..2])
            .join(&second[2..]);
    let stored = std::fs::read(&object).unwrap();
    std::fs::remove_file(&object).unwrap();
    let incomplete = dates(&l, &keys).await;
    std::fs::write(&object, stored).unwrap();
    assert!(
        incomplete.values().all(|t| *t == Time(0)),
        "an incomplete reading invented an introduction date"
    );
    let repaired = dates(&l, &keys).await;
    assert_eq!(repaired[&current["old.txt"]], Time(T0 + 60));
    assert_eq!(repaired[&current["new.txt"]], Time(T0 + 120));
}
