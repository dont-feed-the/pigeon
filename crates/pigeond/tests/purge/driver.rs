//! Driver checks over actual signed Git histories and durable report refs.
//! A zero-day fixture isolates lifecycle ordering; it does not stand in
//! for observing a real month or a partitioned multi-machine erasure.

use super::*;
use pigeond::purge_file::{PurgeDoc, render};

async fn pending() -> (World, String, String) {
    pending_with_rule(None).await
}

async fn pending_with_rule(rule: Option<Rule>) -> (World, String, String) {
    let mut r = roster(1);
    if let Some(rule) = rule {
        r.folders[0].rule = rule;
    }
    r.retention_days = 0;
    let mut w = world_with_roster(r).await;
    let head = purge_upgrade::upgrade(&w.g).await;
    w.base = w.git.tree(&head).await.unwrap();
    pigeond::git::set_signer(key(3));
    let uuid = w.g.roster().unwrap().roster.devices[&dev(3)]
        .annex_uuid
        .clone()
        .unwrap();
    w.g.git.run(&["config", "annex.uuid", &uuid]).await.unwrap();
    let key = w.git.keys(&head).await.unwrap()["Shared/a.txt"].clone();
    let path = ".pigeon/decisions/purge-driver.toml".to_owned();
    let doc = PurgeDoc {
        format: 1,
        kind: "purge".into(),
        history: true,
        paths: vec!["Shared/".into()],
        base: head.clone(),
        keys: vec![],
        proposals: vec![],
        requested_by: "ana".into(),
        retention_end: pigeond::report_file::datetime(T0 - 1),
    };
    let mut tree = w.base.clone();
    tree.remove("Shared/a.txt");
    tree.insert(
        path.clone(),
        blob(&w.git, render(&doc).unwrap().as_bytes()).await,
    );
    let main = commit(&w.git, &tree, &[&head], -60, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&main))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&main));
    w.g.git
        .update_ref("refs/heads/main", &main, Some(&head))
        .await
        .unwrap();
    w.g.reload_roster().await;
    (w, key, path)
}

async fn complete(w: &World, content: &str) {
    assert!(w.g.git.purge_gate.policy().waiting);
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(
        w.g.git.purge_gate.policy().waiting,
        "a cold scan is not an empty release set"
    );
    assert!(w.g.lock().purge_seals.is_empty());
    pigeond::purge::prepare(&w.g).await;
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(!w.g.git.purge_gate.policy().waiting);
    assert!(!w.g.git.purge_gate.policy().released.contains(content));
    assert_eq!(w.g.lock().purge_seals.len(), 1);
    assert!(pigeond::report::publish(&w.g, &dev(3).0).await);
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(w.g.git.purge_gate.policy().released.contains(content));
    assert!(
        !pigeond::retention::keys(&w.g)
            .await
            .unwrap()
            .contains(content)
    );
}

#[tokio::test]
async fn an_exact_scan_and_signed_seal_are_required_and_a_restore_keeps_its_key() {
    let (w, content, _) = pending().await;
    complete(&w, &content).await;
    let history = pigeond::history::saved_versions(&w.g, &dev(3).0, "Shared/a.txt").await;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].state, pigeon_proto::api::State::Erased);
    let before = w.g.git.rev("refs/heads/main").await.unwrap();
    let mut tree = w.g.git.tree(&before).await.unwrap();
    tree.insert("Shared/restored.txt".into(), w.base["Shared/a.txt"].clone());
    let restored = commit(&w.git, &tree, &[&before], 2, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&restored))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&restored));
    w.g.git
        .update_ref("refs/heads/main", &restored, Some(&before))
        .await
        .unwrap();
    assert!(w.g.git.purge_gate.policy().waiting);
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(w.g.git.purge_gate.policy().waiting);
    pigeond::purge::prepare(&w.g).await;
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(!w.g.git.purge_gate.policy().waiting);
    assert!(!w.g.git.purge_gate.policy().released.contains(&content));
    assert!(
        pigeond::history::saved_versions(&w.g, &dev(3).0, "Shared/a.txt")
            .await
            .iter()
            .all(|v| v.state != pigeon_proto::api::State::Erased),
        "a fresh uncovered reference makes the historical key retained again"
    );
    assert!(
        pigeond::retention::keys(&w.g)
            .await
            .unwrap()
            .contains(&content)
    );
    // Even with an unchanged key, the new path is a real uncovered birth.
    assert_eq!(w.g.lock().purge_seals.len(), 1);
}

#[tokio::test]
async fn loss_of_receipts_does_not_forget_a_retained_signed_seal() {
    let (w, content, _) = pending().await;
    complete(&w, &content).await;
    std::fs::remove_file(w.g.root().join("purges.toml")).unwrap();
    let reopened = Group::open(&w.genesis, &w.g.git_dir, &GitEnv::default())
        .await
        .unwrap();
    assert!(reopened.git.purge_gate.policy().waiting);
    pigeond::purge::prepare(&reopened).await;
    pigeond::purge::round(&reopened, &dev(3).0, false).await;
    assert!(!reopened.git.purge_gate.policy().waiting);
    assert!(reopened.git.purge_gate.policy().released.contains(&content));
    assert_eq!(reopened.lock().purge_seals.len(), 1);
}

#[tokio::test]
async fn a_completed_epoch_survives_added_and_removed_backups_and_restarts() {
    let (w, content, _) = pending().await;
    complete(&w, &content).await;
    assert!(pigeond::purge_epoch::close(&w.g).await.unwrap());
    assert!(!pigeond::purge_epoch::close(&w.g).await.unwrap());
    let closed = w.g.git.rev("refs/heads/main").await.unwrap();
    assert_eq!(
        pigeond::purge_epoch::collect(&w.g, &closed)
            .await
            .unwrap()
            .len(),
        1
    );
    let mut tree = w.g.git.tree(&closed).await.unwrap();
    let mut roster = w.g.roster().unwrap();
    let mut added = roster.roster.devices[&dev(3)].clone();
    added.name = "new backup".into();
    added.annex_uuid = Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e04".into());
    roster.roster.devices.insert(dev(4), added);
    tree.insert(
        ROSTER.into(),
        blob(&w.git, roster_file::render(&roster).unwrap().as_bytes()).await,
    );
    let joined = commit(&w.git, &tree, &[&closed], 3, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&joined))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&joined));
    w.g.git
        .update_ref("refs/heads/main", &joined, Some(&closed))
        .await
        .unwrap();
    w.g.reload_roster().await;
    pigeond::purge::prepare(&w.g).await;
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(!w.g.git.purge_gate.policy().waiting);
    assert!(w.g.git.purge_gate.policy().released.contains(&content));
    let removed = roster.roster.devices.remove(&dev(3)).unwrap();
    roster.roster.removed.insert(dev(3), removed.annex_uuid);
    tree.insert(
        ROSTER.into(),
        blob(&w.git, roster_file::render(&roster).unwrap().as_bytes()).await,
    );
    let left = commit(&w.git, &tree, &[&joined], 4, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&left))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&left));
    w.g.git
        .update_ref("refs/heads/main", &left, Some(&joined))
        .await
        .unwrap();
    let reopened = Group::open(&w.genesis, &w.g.git_dir, &GitEnv::default())
        .await
        .unwrap();
    // A laptop can apply the completed proof without posing as a backup.
    pigeond::purge::prepare(&reopened).await;
    pigeond::purge::round(&reopened, &dev(1).0, false).await;
    assert!(!reopened.git.purge_gate.policy().waiting);
    assert!(reopened.git.purge_gate.policy().released.contains(&content));
}

#[tokio::test]
async fn an_unavailable_signed_report_waits_then_admits_after_proof_fetch() {
    let (w, content, _) = pending().await;
    complete(&w, &content).await;
    assert!(pigeond::purge_epoch::close(&w.g).await.unwrap());
    let main = w.g.git.rev("refs/heads/main").await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let repo = Git::new(dir.path(), GitEnv::default());
    repo.run(&["init", "--bare", "-q"]).await.unwrap();
    let source = w.g.git_dir.to_str().unwrap();
    repo.run(&["fetch", "-q", source, "+refs/heads/main:refs/heads/main"])
        .await
        .unwrap();
    let g = Group::open(&w.genesis, dir.path(), &GitEnv::default())
        .await
        .unwrap();
    merge::judge_history(&g, std::slice::from_ref(&main))
        .await
        .unwrap();
    assert!(!g.lock().engine.accepted(&main));
    assert!(matches!(
        g.lock().verdicts.get(&main),
        Some(pigeon_core::authority::Verdict::Unknown(_))
    ));
    let spec = pigeond::purge_epoch::refspec("fixture");
    repo.run(&["fetch", "-q", source, &spec]).await.unwrap();
    merge::judge_history(&g, std::slice::from_ref(&main))
        .await
        .unwrap();
    assert!(g.lock().engine.accepted(&main));
    let epochs = pigeond::purge_epoch::collect(&g, &main).await.unwrap();
    assert_eq!(epochs.len(), 1);
    for oid in epochs.values().flat_map(|e| e.doc.reports.values()) {
        assert_eq!(
            repo.rev(&format!("{}{}", pigeond::purge_epoch::PROOFS, oid))
                .await
                .as_ref(),
            Some(oid)
        );
    }
}

#[tokio::test]
async fn deciders_are_alerted_when_a_post_erasure_reference_has_no_known_copy() {
    missing_alert(false).await;
}

#[tokio::test]
async fn another_annex_extension_still_explains_missing_erased_contents() {
    missing_alert(true).await;
}

async fn missing_alert(alias: bool) {
    let (w, content, _) = pending().await;
    complete(&w, &content).await;
    assert!(pigeond::purge_epoch::close(&w.g).await.unwrap());
    let before = w.g.git.rev("refs/heads/main").await.unwrap();
    let mut tree = w.g.git.tree(&before).await.unwrap();
    let path = if alias {
        "Recovered/restored.bin"
    } else {
        "Recovered/restored.txt"
    };
    let restored_key = if alias {
        format!("{}.bin", content.trim_end_matches(".txt"))
    } else {
        content.clone()
    };
    let entry = if alias {
        blob(
            &w.git,
            format!("/annex/objects/{restored_key}\n").as_bytes(),
        )
        .await
    } else {
        w.base["Shared/a.txt"].clone()
    };
    tree.insert(path.into(), entry);
    let restored = commit(&w.git, &tree, &[&before], 2, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&restored))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&restored));
    w.g.git
        .update_ref("refs/heads/main", &restored, Some(&before))
        .await
        .unwrap();
    for restart in [false, true] {
        let reopened;
        let group = if restart {
            reopened = Group::open(&w.genesis, &w.g.git_dir, &GitEnv::default())
                .await
                .unwrap();
            &reopened
        } else {
            &w.g
        };
        pigeond::purge::prepare(group).await;
        pigeond::purge::round(group, &dev(3).0, false).await;
        assert!(!group.git.purge_gate.policy().waiting);
        assert!(
            !group
                .git
                .purge_gate
                .policy()
                .released
                .contains(&restored_key)
        );
        let report = pigeond::status::report(group, &dev(3).0, "", false).await;
        assert_eq!(report.state, pigeon_proto::api::State::Missing);
        assert_eq!(
            report.level,
            pigeon_proto::api::Level::NeedsYou,
            "restart={restart}: {report:?}"
        );
        assert_eq!(report.headline.id, "headline-erased-restoration-missing");
        assert!(report.versions.iter().any(|v| {
            v.path == path
                && v.state == pigeon_proto::api::State::Missing
                && v.reason
                    .as_ref()
                    .is_some_and(|m| m.id == "why-erased-restoration-missing")
        }));
        let member = pigeond::status::report(group, &dev(2).0, "", false).await;
        assert_ne!(
            member.level,
            pigeon_proto::api::Level::NeedsYou,
            "a member is not this path's decider"
        );
    }
}

async fn fresh_reference(w: &World, path: &str) {
    fresh_entry(w, path, w.base["Shared/a.txt"].clone()).await;
}

async fn fresh_entry(w: &World, path: &str, entry: Entry) {
    let before = w.g.git.rev("refs/heads/main").await.unwrap();
    let mut tree = w.g.git.tree(&before).await.unwrap();
    tree.insert(path.into(), entry);
    let head = commit(&w.git, &tree, &[&before], 3, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&head))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&head));
    w.g.git
        .update_ref("refs/heads/main", &head, Some(&before))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_pending_erasure_does_not_explain_an_unavailable_new_reference_as_erased() {
    let (w, content, _) = pending().await;
    fresh_reference(&w, "Recovered/restored.txt").await;
    pigeond::purge::prepare(&w.g).await;
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(!w.g.git.purge_gate.policy().waiting);
    assert!(!w.g.git.purge_gate.policy().released.contains(&content));
    let report = pigeond::status::report(&w.g, &dev(3).0, "", false).await;
    assert_eq!(report.state, pigeon_proto::api::State::Missing);
    assert_ne!(
        report.level,
        pigeon_proto::api::Level::NeedsYou,
        "{report:?}"
    );
    assert!(report.versions.iter().all(|v| {
        v.reason
            .as_ref()
            .is_none_or(|m| m.id != "why-erased-restoration-missing")
    }));
}

#[tokio::test]
async fn a_retained_alias_before_the_seals_prevents_the_erased_restoration_diagnosis() {
    let (w, content, _) = pending().await;
    let alias_key = format!("{}.bin", content.trim_end_matches(".txt"));
    let alias = blob(&w.git, format!("/annex/objects/{alias_key}\n").as_bytes()).await;
    fresh_entry(&w, "Outside/retained.bin", alias).await;
    pigeond::purge::prepare(&w.g).await;
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(pigeond::report::publish(&w.g, &dev(3).0).await);
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(w.g.git.purge_gate.policy().released.contains(&content));
    assert!(!w.g.git.purge_gate.policy().released.contains(&alias_key));
    fresh_reference(&w, "Recovered/restored.txt").await;
    pigeond::purge::prepare(&w.g).await;
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(!w.g.git.purge_gate.policy().waiting);
    assert!(!w.g.git.purge_gate.policy().released.contains(&content));
    let report = pigeond::status::report(&w.g, &dev(3).0, "", false).await;
    assert_eq!(report.state, pigeon_proto::api::State::Missing);
    assert_ne!(
        report.level,
        pigeon_proto::api::Level::NeedsYou,
        "{report:?}"
    );
    assert!(report.versions.iter().all(|v| {
        v.reason
            .as_ref()
            .is_none_or(|m| m.id != "why-erased-restoration-missing")
    }));
}

#[tokio::test]
async fn accepting_a_missing_restoration_suggestion_alerts_its_decider() {
    let (w, content, _) = pending_with_rule(Some(Rule::Owned(PersonId("ana".into())))).await;
    complete(&w, &content).await;
    let parent = w.g.git.rev("refs/heads/main").await.unwrap();
    let mut tree = w.g.git.tree(&parent).await.unwrap();
    tree.insert("Shared/a.txt".into(), w.base["Shared/a.txt"].clone());
    let proposal = commit(&w.git, &tree, &[&parent], 2, 2).await;
    w.git
        .update_ref(
            &pigeond::proposal::name(&dev(2).0, &proposal),
            &proposal,
            None,
        )
        .await
        .unwrap();
    pigeond::purge::prepare(&w.g).await;
    pigeond::purge::round(&w.g, &dev(3).0, false).await;
    assert!(!w.g.git.purge_gate.policy().released.contains(&content));
    let pending = pigeond::decide::pending(&w.g).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, proposal);
    let adopted = merge::adopt(&w.g, &parent, &proposal, None)
        .await
        .unwrap()
        .unwrap();
    w.g.git
        .update_ref("refs/heads/main", &adopted, Some(&parent))
        .await
        .unwrap();
    for restart in [false, true] {
        let reopened;
        let g = if restart {
            reopened = Group::open(&w.genesis, &w.g.git_dir, &GitEnv::default())
                .await
                .unwrap();
            &reopened
        } else {
            &w.g
        };
        pigeond::purge::prepare(g).await;
        pigeond::purge::round(g, &dev(3).0, false).await;
        assert!(!g.git.purge_gate.policy().released.contains(&content));
        let report = pigeond::status::report(g, &dev(3).0, "", false).await;
        assert_eq!(report.level, pigeon_proto::api::Level::NeedsYou);
        assert_eq!(report.headline.id, "headline-erased-restoration-missing");
        assert!(report.versions.iter().any(|v| {
            v.path == "Shared/a.txt"
                && v.state == pigeon_proto::api::State::Missing
                && v.reason
                    .as_ref()
                    .is_some_and(|m| m.id == "why-erased-restoration-missing")
        }));
        let member = pigeond::status::report(g, &dev(2).0, "", false).await;
        assert_ne!(member.level, pigeon_proto::api::Level::NeedsYou);
    }
}
