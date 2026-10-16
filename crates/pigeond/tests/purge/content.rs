//! Physical forced-drop boundary with actual git-annex objects. Signed
//! pre-sealed historical fixture data isolates consumption and erasure;
//! it does not claim that a real 30-day receiver wait elapsed in this test.

use super::*;

pub(super) async fn signed(
    l: &Laptop,
    tree: &Tree,
    parents: &[&str],
    at: i64,
    signer: u8,
) -> String {
    let root = merge::write_tree(&l.git, tree).await.unwrap();
    let mut text = format!("tree {root}\n");
    for parent in parents {
        text.push_str(&format!("parent {parent}\n"));
    }
    text.push_str(&format!("author pigeon <pigeon@localhost> {at} +0000\ncommitter pigeon <pigeon@localhost> {at} +0000\n\npigeon: fixture\n"));
    let bytes = sig::sign(text.as_bytes(), &key(signer)).unwrap();
    l.git
        .run_in(&["hash-object", "-t", "commit", "-w", "--stdin"], &bytes)
        .await
        .unwrap()
        .trim()
        .to_owned()
}

async fn entry(l: &Laptop, bytes: &[u8]) -> Entry {
    let id = l
        .git
        .run_in(&["hash-object", "-w", "--stdin"], bytes)
        .await
        .unwrap();
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: id.trim().into(),
    }
}

#[tokio::test]
async fn only_fully_released_historical_objects_drop_and_private_bytes_stay() {
    let Some(l) = laptop().await else { return };
    let genesis = purge_upgrade::upgrade(&l.g).await;
    let old_bytes = b"exact historical bytes to erase\x00\xff\r\n";
    let old_entry = version(&l, "erased.txt", old_bytes, true).await;
    let kept_bytes = b"different current bytes must stay\n";
    let kept_entry = version(&l, "kept.txt", kept_bytes, true).await;
    let mut tree = l.g.git.tree(&genesis).await.unwrap();
    tree.insert("Shared/erased.txt".into(), old_entry.clone());
    tree.insert("Shared/kept.txt".into(), kept_entry);
    let original = signed(&l, &tree, &[&genesis], T0 - 70 * 86_400, 1).await;
    let doc = pigeond::purge_file::PurgeDoc {
        format: 1,
        kind: "purge".into(),
        history: true,
        paths: vec!["Shared/erased.txt".into()],
        base: original.clone(),
        keys: vec![],
        proposals: vec![],
        requested_by: "ana".into(),
        retention_end: pigeond::report_file::datetime(T0 - 30 * 86_400),
    };
    tree.remove("Shared/erased.txt");
    tree.insert(
        ".pigeon/decisions/purge-physical.toml".into(),
        entry(&l, pigeond::purge_file::render(&doc).unwrap().as_bytes()).await,
    );
    let decision = signed(&l, &tree, &[&original], T0 - 61 * 86_400, 1).await;
    merge::judge_history(&l.g, std::slice::from_ref(&decision))
        .await
        .unwrap();
    assert!(l.g.lock().engine.accepted(&decision));
    let old_key = l.git.keys(&original).await.unwrap()["Shared/erased.txt"].clone();
    let kept_key = l.git.keys(&original).await.unwrap()["Shared/kept.txt"].clone();
    let old_object = pigeond::custody::present(&l.g, &old_key).await.unwrap();
    let kept_object = pigeond::custody::present(&l.g, &kept_key).await.unwrap();
    assert_eq!(std::fs::read(&old_object).unwrap(), old_bytes);
    // A provider's canonical pool is a separate hard link. Dropping the
    // annex name alone would leave these exact bytes retained forever.
    let store = l.wt.parent().unwrap().join("provider-pool");
    std::fs::create_dir_all(&store).unwrap();
    let pool_path = |group: &str, key: &str| {
        let hash = pigeond::net::sync::key_sha256(key).unwrap();
        store.join(group).join(&hash[..2]).join(hash)
    };
    let old_pool = pool_path(&l.g.id, &old_key);
    let kept_pool = pool_path(&l.g.id, &kept_key);
    let independent_pool = pool_path(&"b".repeat(40), &old_key);
    for (target, source) in [
        (&old_pool, &old_object),
        (&kept_pool, &kept_object),
        (&independent_pool, &old_object),
    ] {
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::hard_link(source, target).unwrap();
    }
    let registration = serde_json::json!({ "format": 1, "store": store });
    std::fs::write(
        l.g.root().join("shared-provider.json"),
        serde_json::to_vec(&registration).unwrap(),
    )
    .unwrap();
    let private = b"private, staged and never published\n";
    pigeond::change::stage_file(&l.git, Some(&l.wt), "private.txt", private)
        .await
        .unwrap();
    let private_entry = l.git.index().await.unwrap()["private.txt"].clone();
    l.g.git
        .update_ref("refs/heads/main", &decision, Some(&genesis))
        .await
        .unwrap();
    l.g.reload_roster().await;
    pigeond::purge::prepare(&l.g).await;
    pigeond::purge::round(&l.g, &dev(1).0, true).await;
    assert!(
        old_object.exists(),
        "missing seal must retain the real bytes"
    );
    assert!(
        pigeond::history::saved_versions(&l.g, &dev(1).0, "Shared/erased.txt")
            .await
            .iter()
            .all(|v| v.state != pigeon_proto::api::State::Erased),
        "a pending erasure is not an erased version"
    );
    let active = pigeond::purge_state::active(&l.g, &decision).await.unwrap();
    let id = active.keys().next().unwrap().clone();
    let mut report = pigeond::report::build(&l.g, &dev(3).0).await;
    report.custody = Some(pigeond::report_file::Custody {
        repository: "ok".into(),
    });
    report.purge = vec![pigeond::report_file::Purge {
        decision: id,
        main: decision.clone(),
        held: vec![],
    }];
    let reports = [(
        "status.toml".into(),
        entry(
            &l,
            pigeond::report_file::render(&report).unwrap().as_bytes(),
        )
        .await,
    )]
    .into();
    let witness = signed(&l, &reports, &[], pigeond::util::now(), 3).await;
    l.g.git
        .update_ref(&format!("refs/pigeon/status/{}", dev(3).0), &witness, None)
        .await
        .unwrap();
    pigeond::purge::round(&l.g, &dev(1).0, true).await;
    assert!(l.g.git.purge_gate.policy().released.contains(&old_key));
    assert!(
        !old_object.exists(),
        "fully qualified purge must physically drop its object"
    );
    assert!(pigeond::custody::present(&l.g, &old_key).await.is_none());
    let history = pigeond::history::saved_versions(&l.g, &dev(1).0, "Shared/erased.txt").await;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].state, pigeon_proto::api::State::Erased);
    assert!(
        !old_pool.exists(),
        "qualified erasure must release the pool name too"
    );
    assert_eq!(std::fs::read(&kept_pool).unwrap(), kept_bytes);
    assert_eq!(std::fs::read(&independent_pool).unwrap(), old_bytes);
    // Cleanup remains idempotent if annex has already lost its own name.
    std::fs::hard_link(&independent_pool, &old_pool).unwrap();
    pigeond::purge::round(&l.g, &dev(1).0, true).await;
    assert!(!old_pool.exists());
    assert_eq!(std::fs::read(&kept_object).unwrap(), kept_bytes);
    assert_eq!(std::fs::read(l.wt.join("private.txt")).unwrap(), private);
    assert_eq!(l.git.index().await.unwrap()["private.txt"], private_entry);
    assert!(
        !pigeond::retention::keys(&l.g)
            .await
            .unwrap()
            .contains(&old_key)
    );
    assert!(
        pigeond::retention::keys(&l.g)
            .await
            .unwrap()
            .contains(&kept_key)
    );
    // Extensions distinguish annex keys, while the pool shares their
    // hash. A fresh uncovered .bin reference must retain those same bytes.
    let alias = version(&l, "restored.bin", old_bytes, true).await;
    let mut tree = l.git.tree(&decision).await.unwrap();
    tree.insert("Shared/restored.bin".into(), alias);
    let restored = signed(&l, &tree, &[&decision], pigeond::util::now(), 1).await;
    merge::judge_history(&l.g, std::slice::from_ref(&restored))
        .await
        .unwrap();
    assert!(l.g.lock().engine.accepted(&restored));
    l.g.git
        .update_ref("refs/heads/main", &restored, Some(&decision))
        .await
        .unwrap();
    let alias_key = l.git.keys(&restored).await.unwrap()["Shared/restored.bin"].clone();
    assert_ne!(alias_key, old_key);
    assert_eq!(
        pigeond::net::sync::key_sha256(&alias_key),
        pigeond::net::sync::key_sha256(&old_key)
    );
    std::fs::hard_link(&independent_pool, &old_pool).unwrap();
    pigeond::purge::prepare(&l.g).await;
    pigeond::purge::round(&l.g, &dev(1).0, true).await;
    assert!(l.g.git.purge_gate.policy().released.contains(&old_key));
    assert!(!l.g.git.purge_gate.policy().released.contains(&alias_key));
    assert_eq!(std::fs::read(&old_pool).unwrap(), old_bytes);
    assert!(pigeond::custody::present(&l.g, &alias_key).await.is_some());

    // A real new signed authority domain retains completed erasure in
    // its old archive, without copying old votes into its new genesis.
    assert!(pigeond::purge_epoch::close(&l.g).await.unwrap());
    let old_main = l.g.git.rev("refs/heads/main").await.unwrap();
    let tree = pigeond::refound::new_tree(&l.g, &old_main, &dev(1))
        .await
        .unwrap();
    assert!(
        tree.keys()
            .all(|p| !p.starts_with(pigeond::purge_file::PREFIX)
                && !p.starts_with(pigeond::purge_epoch::PREFIX)
                && !p.starts_with(pigeond::admission_file::PREFIX))
    );
    let tree = merge::write_tree(&l.git, &tree).await.unwrap();
    let message = format!(
        "pigeon: refound install\n\nPigeon-Refound: {} {old_main}",
        l.g.id
    );
    let new = l.git.commit_tree(&tree, &[], &message).await.unwrap();
    let archive = pigeond::refound::archive_ref(&l.g.id);
    l.git.update_ref(&archive, &old_main, None).await.unwrap();
    l.git
        .update_ref("refs/heads/main", &new, Some(&old_main))
        .await
        .unwrap();
    for restart in [false, true] {
        let group = Group::open(&new, &l.wt, &GitEnv::default()).await.unwrap();
        merge::judge_history(&group, std::slice::from_ref(&new))
            .await
            .unwrap();
        assert!(group.lock().engine.accepted(&new));
        assert_eq!(group.roster().unwrap().roster.rules, 1);
        assert!(
            pigeond::purge_state::active(&group, &new)
                .await
                .unwrap()
                .is_empty()
        );
        pigeond::purge::prepare(&group).await;
        pigeond::purge::round(&group, &dev(1).0, true).await;
        let policy = group.git.purge_gate.policy();
        assert!(
            !policy.waiting,
            "refounded archive stayed unverified, restart={restart}"
        );
        assert!(policy.released.contains(&old_key));
        assert!(!policy.released.contains(&alias_key));
        assert!(
            !pigeond::retention::keys(&group)
                .await
                .unwrap()
                .contains(&old_key)
        );
        let alias_object = pigeond::custody::present(&group, &alias_key).await.unwrap();
        assert_eq!(std::fs::read(alias_object).unwrap(), old_bytes);
        assert_eq!(std::fs::read(&kept_object).unwrap(), kept_bytes);
        assert_eq!(std::fs::read(l.wt.join("private.txt")).unwrap(), private);
        assert_eq!(l.git.index().await.unwrap()["private.txt"], private_entry);
        if restart {
            l.git.run(&["update-ref", "-d", &archive]).await.unwrap();
            pigeond::purge::round(&group, &dev(1).0, true).await;
            assert!(
                group.git.purge_gate.policy().waiting,
                "a cached proof ignored its removed archive anchor"
            );
            assert_eq!(std::fs::read(&kept_object).unwrap(), kept_bytes);
        }
    }
}

#[tokio::test]
async fn confirmed_erasure_deletes_clean_current_files_and_preserves_private_saves() {
    use pigeon_proto::erase::{Answer, Selection};
    let Some(l) = laptop().await else { return };
    let upgraded = purge_upgrade::upgrade(&l.g).await;
    let clean_bytes = b"clean file reviewed for erasing\n";
    let dirty_bytes = b"accepted bytes before private save\n";
    let clean = version(&l, "clean.txt", clean_bytes, true).await;
    let dirty = version(&l, "dirty.txt", dirty_bytes, true).await;
    let mut tree = l.g.git.tree(&upgraded).await.unwrap();
    tree.insert("Shared/clean.txt".into(), clean);
    tree.insert("Shared/dirty.txt".into(), dirty.clone());
    tree.insert("Shared/staged.txt".into(), dirty);
    let original = signed(&l, &tree, &[&upgraded], T0 + 100, 1).await;
    merge::judge_history(&l.g, std::slice::from_ref(&original))
        .await
        .unwrap();
    assert!(l.g.lock().engine.accepted(&original));
    l.g.git
        .update_ref("refs/heads/main", &original, Some(&upgraded))
        .await
        .unwrap();
    install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(
        std::fs::read(l.wt.join("Shared/clean.txt")).unwrap(),
        clean_bytes
    );
    let private = b"new private edit must never be erased or published\n";
    std::fs::write(l.wt.join("Shared/dirty.txt"), private).unwrap();
    pigeond::change::stage_file(
        &l.git,
        Some(&l.wt),
        "staged-private.txt",
        b"private staged data\n",
    )
    .await
    .unwrap();
    let staged = l.git.index().await.unwrap()["staged-private.txt"].clone();
    pigeond::change::stage_file(
        &l.git,
        Some(&l.wt),
        "Shared/staged.txt",
        b"selected private staged data\n",
    )
    .await
    .unwrap();
    let selected_staged = l.git.index().await.unwrap()["Shared/staged.txt"].clone();
    let selection = Selection::Path(l.wt.join("Shared").to_string_lossy().into());
    let plan = pigeond::erase_preview::plan(&l.g, &dev(1).0, &selection)
        .await
        .unwrap();
    assert_eq!(plan.preview.current_files, 3);
    let answer = pigeond::erase_commit::submit(&l.g, &dev(1).0, plan, &l.t)
        .await
        .unwrap();
    assert!(matches!(
        answer,
        Answer::Submitted {
            needs_approval: false,
            ..
        }
    ));
    assert!(!l.wt.join("Shared/clean.txt").exists());
    assert_eq!(
        std::fs::read(l.wt.join("Shared/dirty.txt")).unwrap(),
        private
    );
    assert_eq!(l.git.index().await.unwrap()["staged-private.txt"], staged);
    assert_eq!(
        l.git.index().await.unwrap()["Shared/staged.txt"],
        selected_staged
    );
    assert_eq!(
        std::fs::read(l.wt.join("Shared/staged.txt")).unwrap(),
        b"selected private staged data\n"
    );
    let head = l.g.git.rev("refs/heads/main").await.unwrap();
    let accepted = l.g.git.tree(&head).await.unwrap();
    assert!(!accepted.contains_key("Shared/clean.txt"));
    assert!(!accepted.contains_key("Shared/dirty.txt"));
    assert!(!accepted.contains_key("staged-private.txt"));
    let decisions = pigeond::purge_state::active(&l.g, &head).await.unwrap();
    assert_eq!(decisions.len(), 1);
    assert!(decisions.values().all(|a| a.doc.base == original));
    assert!(!l.g.root().join("erase-install.toml").exists());
}
