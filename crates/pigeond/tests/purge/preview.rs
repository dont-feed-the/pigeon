//! Real signed-history erasure previews: no refs, index or private state
//! change on read; stale consent and removed-ref cache entries are tested
//! against Git, not a duplicate of the preview implementation.

use super::*;
use pigeon_proto::erase::Selection;

async fn refs(g: &Group) -> String {
    g.git
        .run(&["for-each-ref", "--format=%(refname) %(objectname)"])
        .await
        .unwrap()
}

#[tokio::test]
async fn erase_preview_is_read_only_and_a_new_reference_invalidates_consent() {
    let mut r = roster(1);
    r.folders[0].rule = Rule::Owned(PersonId("ana".into()));
    let w = world_with_roster(r).await;
    let main = purge_upgrade::upgrade(&w.g).await;
    let selected = Selection::Path(w.g.git_dir.join("Shared/a.txt").to_string_lossy().into());
    let before = refs(&w.g).await;
    let index = w.git.index().await.unwrap();
    let plan = pigeond::erase_preview::plan(&w.g, &dev(1).0, &selected)
        .await
        .unwrap();
    assert_eq!(plan.preview.current_files, 1);
    assert_eq!(plan.preview.versions, 1);
    assert_eq!(plan.preview.bytes, 1);
    assert_eq!(plan.preview.shared_versions, 0);
    assert!(plan.preview.readiness.ready);
    assert_eq!(before, refs(&w.g).await);
    assert_eq!(index, w.git.index().await.unwrap());
    assert!(
        pigeond::erase_preview::unchanged(&w.g, &plan)
            .await
            .unwrap()
    );
    let mut proposal = w.git.tree(&main).await.unwrap();
    proposal.insert("Shared/a.txt".into(), pointer(&w.git, 88).await);
    let suggested = commit(&w.git, &proposal, &[&main], 1, 2).await;
    let name = pigeond::proposal::name(&dev(2).0, &suggested);
    w.git.update_ref(&name, &suggested, None).await.unwrap();
    assert!(
        !pigeond::erase_preview::unchanged(&w.g, &plan)
            .await
            .unwrap()
    );
    let fresh = pigeond::erase_preview::plan(&w.g, &dev(1).0, &selected)
        .await
        .unwrap();
    assert_ne!(fresh.preview.token, plan.preview.token);
    // Incoming suggestions are not silently added to a path erasure.
    assert_eq!(fresh.preview.versions, 1);
    assert!(fresh.documents.iter().all(|doc| doc.proposals.is_empty()));
    let read = refs(&w.g).await;
    let suggestion = pigeond::erase_preview::plan(
        &w.g,
        &dev(1).0,
        &Selection::Suggestion(suggested[..8].into()),
    )
    .await
    .unwrap();
    assert_eq!(suggestion.preview.current_files, 0);
    assert_eq!(suggestion.preview.versions, 1);
    assert!(suggestion.documents.iter().all(|doc| !doc.history));
    assert_eq!(read, refs(&w.g).await);
}

#[tokio::test]
async fn erased_path_preview_counts_shared_bytes_but_ignores_orphaned_cache_births() {
    let w = world().await;
    let main = purge_upgrade::upgrade(&w.g).await;
    let original = w.git.tree(&main).await.unwrap();
    let mut tree = original.clone();
    tree.insert("Shared/other.txt".into(), original["Shared/a.txt"].clone());
    let added = commit(&w.git, &tree, &[&main], 1, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&added))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&added));
    w.git
        .update_ref("refs/heads/main", &added, Some(&main))
        .await
        .unwrap();
    let selected = Selection::Path(w.g.git_dir.join("Shared/a.txt").to_string_lossy().into());
    let shared = pigeond::erase_preview::plan(&w.g, &dev(1).0, &selected)
        .await
        .unwrap();
    assert_eq!(shared.preview.shared_versions, 1);
    assert_eq!(shared.preview.retained_elsewhere, vec!["Shared/other.txt"]);
    // Populate the append-only cache, then remove the only ref retaining
    // this fork. It must not claim shared content in the new reading.
    w.g.git.purge_gate.arm();
    pigeond::purge::prepare(&w.g).await;
    w.git
        .update_ref("refs/heads/main", &main, Some(&added))
        .await
        .unwrap();
    let fresh = pigeond::erase_preview::plan(&w.g, &dev(1).0, &selected)
        .await
        .unwrap();
    assert_eq!(fresh.preview.shared_versions, 0);
    assert!(fresh.preview.retained_elsewhere.is_empty());
    for path in ["../outside", ".git/config", ".pigeon/roster.toml"] {
        let invalid = Selection::Path(w.g.git_dir.join(path).to_string_lossy().into());
        assert!(
            pigeond::erase_preview::plan(&w.g, &dev(1).0, &invalid)
                .await
                .is_err()
        );
    }
}
