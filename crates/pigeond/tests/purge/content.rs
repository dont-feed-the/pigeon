//! Physical forced-drop boundary with actual git-annex objects. Signed
//! pre-sealed historical fixture data isolates consumption and erasure;
//! it does not claim that a real 30-day receiver wait elapsed in this test.

use super::*;

async fn signed(l: &Laptop, tree: &Tree, parents: &[&str], at: i64, signer: u8) -> String {
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
}
