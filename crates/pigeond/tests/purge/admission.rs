//! Real signed acknowledgements, external receipts, restored repository
//! rollback and missing-receipt recovery (proposed ADR 0075).

use super::*;
use pigeond::admission_file::Request;
use pigeond::admission_round::Store;
use pigeond::report_file::AdmissionKind;

async fn report(w: &World, main: &str, request: &Request, n: u8) -> String {
    let mut report = pigeond::report::build(&w.g, &dev(n).0).await;
    report.main = Some(main.into());
    report.custody = (n >= 3).then(|| pigeond::report_file::Custody {
        repository: "ok".into(),
    });
    report.checking = n == 4;
    report.acknowledgements = vec![pigeond::report_file::AdmissionAck {
        kind: request.kind,
        request: request.id().unwrap(),
        protocol: 2,
    }];
    let tree = [(
        "status.toml".into(),
        blob(
            &w.git,
            pigeond::report_file::render(&report).unwrap().as_bytes(),
        )
        .await,
    )]
    .into();
    let oid = commit(&w.git, &tree, &[], i64::from(n), n).await;
    w.g.git
        .run(&[
            "update-ref",
            &format!("refs/pigeon/status/{}", dev(n).0),
            &oid,
        ])
        .await
        .unwrap();
    oid
}

async fn move_main(w: &World, main: &str, parent: &str) {
    merge::judge_history(&w.g, &[main.into()]).await.unwrap();
    assert!(
        w.g.lock().engine.accepted(main),
        "{:?}",
        w.g.lock().verdicts.get(main)
    );
    w.g.git
        .update_ref("refs/heads/main", main, Some(parent))
        .await
        .unwrap();
    w.g.reload_roster().await;
}

#[tokio::test]
async fn an_upgrade_and_new_backup_require_exact_signed_causal_quorums() {
    let w = world().await;
    pigeond::git::set_signer(key(3));
    let upgrade = Request {
        format: 1,
        kind: AdmissionKind::Upgrade,
        base: w.genesis.clone(),
        device: None,
    };
    let mut tree = w.base.clone();
    tree.insert(
        upgrade.path().unwrap(),
        blob(&w.git, upgrade.render().unwrap().as_bytes()).await,
    );
    let requested = commit(&w.git, &tree, &[&w.genesis], 1, 1).await;
    move_main(&w, &requested, &w.genesis).await;
    // A roster edit alone cannot assert that every participant upgraded.
    let mut r = w.g.roster().unwrap();
    r.roster.rules = 2;
    let mut unproved = tree.clone();
    unproved.insert(
        ROSTER.into(),
        blob(&w.git, roster_file::render(&r).unwrap().as_bytes()).await,
    );
    let premature = commit(&w.git, &unproved, &[&requested], 2, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&premature))
        .await
        .unwrap();
    assert!(!w.g.lock().engine.accepted(&premature));
    assert!(matches!(
        w.g.lock().verdicts.get(&premature),
        Some(pigeon_core::authority::Verdict::Unknown(_))
    ));
    report(&w, &requested, &upgrade, 1).await;
    report(&w, &requested, &upgrade, 3).await;
    assert!(
        !pigeond::admission_publish::round(&w.g).await.unwrap(),
        "one laptop still has not acknowledged"
    );
    report(&w, &requested, &upgrade, 2).await;
    assert!(pigeond::admission_publish::round(&w.g).await.unwrap());
    assert_eq!(w.g.roster().unwrap().roster.rules, 2);
    let upgraded = w.g.git.rev("refs/heads/main").await.unwrap();
    assert_eq!(
        pigeond::membership::read(&w.g, &upgraded)
            .await
            .unwrap()
            .eligible,
        [dev(3).0.clone()].into()
    );
    let mut r = w.g.roster().unwrap();
    let mut backup = r.roster.devices[&dev(3)].clone();
    backup.name = "new backup".into();
    backup.annex_uuid = Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e04".into());
    r.roster.devices.insert(dev(4), backup);
    let mut tree = w.g.git.tree(&upgraded).await.unwrap();
    tree.insert(
        ROSTER.into(),
        blob(&w.git, roster_file::render(&r).unwrap().as_bytes()).await,
    );
    let joined = commit(&w.git, &tree, &[&upgraded], 4, 1).await;
    move_main(&w, &joined, &upgraded).await;
    let state = pigeond::membership::read(&w.g, &joined).await.unwrap();
    assert_eq!(state.pending.len(), 1);
    assert!(!state.eligible.contains(&dev(4).0));
    assert!(pigeond::membership::pending(&w.g, &dev(4).0));
    let request = &state.pending[&dev(4).0];
    assert_eq!(request.base, joined);
    report(&w, &joined, request, 3).await;
    assert!(
        !pigeond::admission_publish::round(&w.g).await.unwrap(),
        "new backup must acknowledge its own capability"
    );
    report(&w, &joined, request, 4).await;
    assert!(pigeond::admission_publish::round(&w.g).await.unwrap());
    let admitted = w.g.git.rev("refs/heads/main").await.unwrap();
    let state = pigeond::membership::read(&w.g, &admitted).await.unwrap();
    assert!(state.pending.is_empty());
    assert_eq!(state.eligible, [dev(3).0.clone(), dev(4).0.clone()].into());
    let state_dir = tempfile::tempdir().unwrap();
    Store::open(state_dir.path().join("acks.toml"))
        .round(&w.g, &dev(3).0)
        .await;
    assert!(w.g.lock().admission_error.is_none());
    assert!(!pigeond::membership::pending(&w.g, &dev(4).0));
}

#[tokio::test]
async fn restoring_a_repository_or_losing_its_receipts_cannot_forget_a_signed_fence() {
    let w = world().await;
    pigeond::git::set_signer(key(3));
    let state = tempfile::tempdir().unwrap();
    let path = state.path().join("admission-acks.toml");
    let store = Store::open(path.clone());
    let request = Request {
        format: 1,
        kind: AdmissionKind::Upgrade,
        base: w.genesis.clone(),
        device: None,
    };
    let mut tree = w.base.clone();
    tree.insert(
        request.path().unwrap(),
        blob(&w.git, request.render().unwrap().as_bytes()).await,
    );
    let proposed = commit(&w.git, &tree, &[&w.genesis], 1, 1).await;
    merge::judge_history(&w.g, std::slice::from_ref(&proposed))
        .await
        .unwrap();
    assert!(w.g.lock().engine.accepted(&proposed));
    w.g.git
        .update_ref("refs/heads/main", &proposed, Some(&w.genesis))
        .await
        .unwrap();
    w.g.reload_roster().await;
    store.round(&w.g, &dev(3).0).await;
    assert!(w.g.lock().admission_error.is_none());
    assert_eq!(w.g.lock().admission_acknowledgements.len(), 1);
    assert_eq!(
        w.g.lock().admission_fence.as_deref(),
        Some(proposed.as_str())
    );
    assert!(pigeond::report::publish(&w.g, &dev(3).0).await);
    let ledger = pigeond::admission_receipts::Ledger::load(&path).unwrap();
    assert_eq!(ledger.acknowledgements(&w.genesis).len(), 1);
    assert_eq!(ledger.watermark(&w.genesis), Some(proposed.as_str()));
    // Restore the repository without rolling back the daemon's private
    // state. A signed old report is not permission to forget the fence.
    w.g.git
        .update_ref("refs/heads/main", &w.genesis, Some(&proposed))
        .await
        .unwrap();
    w.g.reload_roster().await;
    store.round(&w.g, &dev(3).0).await;
    assert!(w.g.lock().admission_error.is_some());
    assert!(w.g.lock().admission_acknowledgements.is_empty());
    assert!(w.g.git.purge_gate.policy().waiting);
    assert!(!pigeond::report::publish(&w.g, &dev(3).0).await);
    // A missing receipt file is recovered from the separately rooted full
    // signed report. An unsigned cache or ref name cannot substitute for it.
    std::fs::remove_file(&path).unwrap();
    let resumed = Store::open(path.clone());
    let g = Group::open(&w.genesis, &w.g.git_dir, &GitEnv::default())
        .await
        .unwrap();
    resumed.round(&g, &dev(3).0).await;
    assert!(g.lock().admission_error.is_some());
    assert!(g.git.purge_gate.policy().waiting);
    let ledger = pigeond::admission_receipts::Ledger::load(&path).unwrap();
    assert_eq!(ledger.watermark(&w.genesis), Some(proposed.as_str()));
    assert_eq!(ledger.acknowledgements(&w.genesis).len(), 1);
    // Ordinary metadata recovery clears the wait without a fresh choice.
    g.git
        .update_ref("refs/heads/main", &proposed, Some(&w.genesis))
        .await
        .unwrap();
    g.reload_roster().await;
    resumed.round(&g, &dev(3).0).await;
    assert!(g.lock().admission_error.is_none());
    assert_eq!(g.lock().admission_acknowledgements.len(), 1);
}
