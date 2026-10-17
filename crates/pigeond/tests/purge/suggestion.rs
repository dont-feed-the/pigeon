//! Suggestion-only erasure must preserve accepted references sharing its
//! exact bytes. The accepted proposal base is independently verified;
//! an older guessed base cannot broaden the released references.

use super::*;
use pigeond::purge_file::PurgeDoc;

#[tokio::test]
async fn erasing_a_reverted_suggestion_preserves_the_same_bytes_in_accepted_history() {
    let mut r = roster(1);
    r.folders[0].rule = Rule::Owned(PersonId("ana".into()));
    let mut w = world_with_roster(r).await;
    let upgraded = purge_upgrade::upgrade(&w.g).await;
    w.base = w.git.tree(&upgraded).await.unwrap();
    let mut current = w.base.clone();
    current.insert("Shared/a.txt".into(), pointer(&w.git, 99).await);
    let base = commit(&w.git, &current, &[&upgraded], 1, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&base))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&base));
    let mut proposed = current.clone();
    proposed.insert("Shared/a.txt".into(), w.base["Shared/a.txt"].clone());
    let proposal = commit(&w.git, &proposed, &[&base], 2, 2).await;
    let name = pigeond::proposal::name(&dev(2).0, &proposal);
    w.git.update_ref(&name, &proposal, None).await.unwrap();
    let path = ".pigeon/decisions/purge-suggestion.toml";
    let mut doc = PurgeDoc {
        format: 1,
        kind: "purge".into(),
        history: false,
        paths: vec!["Shared/a.txt".into()],
        base: base.clone(),
        keys: vec![],
        proposals: vec![proposal.clone()],
        requested_by: "ana".into(),
        retention_end: pigeond::report_file::datetime(T0 + 3 + 30 * 86_400),
    };
    let mut tree = current.clone();
    tree.insert(
        path.into(),
        blob(
            &w.git,
            pigeond::purge_file::render(&doc).unwrap().as_bytes(),
        )
        .await,
    );
    let decision = commit(&w.git, &tree, &[&base], 3, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&decision))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&decision));
    let mut index = pigeond::purge_index::Index::default();
    index
        .extend(&w.git, &[decision, proposal.clone()])
        .await
        .unwrap();
    let ancestors: BTreeMap<_, _> = [&base, &proposal]
        .into_iter()
        .map(|tip| (tip.clone(), index.ancestors(tip).unwrap()))
        .collect();
    let contains =
        |tip: &String, birth: &String| ancestors.get(tip).is_some_and(|set| set.contains(birth));
    let accepted = index
        .references
        .iter()
        .find(|r| r.path == "Shared/a.txt" && r.birth == w.genesis)
        .unwrap();
    let suggested = index
        .references
        .iter()
        .find(|r| r.path == "Shared/a.txt" && r.birth == proposal)
        .unwrap();
    assert_eq!(accepted.key, suggested.key);
    assert!(!doc.decision().covers(accepted, &contains));
    assert!(doc.decision().covers(suggested, &contains));
    // A syntactically valid older base would falsely include accepted
    // history between that base and the proposal. Admission refuses it.
    doc.base = w.genesis.clone();
    let mut bad_tree = current;
    bad_tree.insert(
        path.into(),
        blob(
            &w.git,
            pigeond::purge_file::render(&doc).unwrap().as_bytes(),
        )
        .await,
    );
    let bad = commit(&w.git, &bad_tree, &[&base], 3, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&bad))
        .await
        .unwrap();
    assert!(!w.g.lock().engine.accepted(&bad));
    assert_eq!(
        w.g.lock().verdicts.get(&bad),
        Some(&pigeon_core::authority::Verdict::Unknown("purge-history"))
    );
}
