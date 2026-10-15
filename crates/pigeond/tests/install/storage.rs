//! Storage boundary regressions: failed capture, durable aside release,
//! recovery-name reuse, and complete retained-history placement/status.

use super::*;

use pigeon_core::journal::{Begin, End, Record};

fn seed_recovered(l: &Laptop, bytes: &[u8]) {
    let seq = 1;
    let aside = l.g.files.aside(seq);
    std::fs::write(&aside, bytes).unwrap();
    let b = Begin {
        seq,
        path: "a.txt".into(),
        old_key: None,
        new_key: None,
        stat: None,
        stage: None,
        aside: aside.to_string_lossy().into(),
    };
    for r in [
        Record::Begin(b),
        Record::End(seq, End::Recovered),
        Record::Kept(seq, 0),
    ] {
        pigeond::journal_file::append(&l.g.files.install_journal(), &r).unwrap();
    }
}

#[tokio::test]
async fn a_crash_before_preserving_cannot_adopt_a_colliding_file() {
    let Some(l) = laptop().await else { return };
    seed_recovered(&l, b"save still at the aside");
    let collision = l.g.files.recovered().join("1");
    std::fs::write(&collision, b"an earlier saved file").unwrap();
    pigeond::journal_file::append(
        &l.g.files.install_journal(),
        &Record::Preserved(1, collision.to_string_lossy().into()),
    )
    .unwrap();
    pigeond::asides::release_at(&l.g, &l.t, pigeond::asides::KEEP_SECONDS).await;
    assert_eq!(std::fs::read(&collision).unwrap(), b"an earlier saved file");
    assert_eq!(
        std::fs::read(l.g.files.recovered().join("1-1")).unwrap(),
        b"save still at the aside"
    );
    assert!(pigeond::journal_file::read(&l.g.files.install_journal()).is_empty());
}

#[tokio::test]
async fn a_missing_preserved_file_does_not_discard_its_recovery_intent() {
    let Some(l) = laptop().await else { return };
    seed_recovered(&l, b"save temporarily unavailable");
    let kept = l.g.files.recovered().join("1");
    pigeond::journal_file::append(
        &l.g.files.install_journal(),
        &Record::Preserved(1, kept.to_string_lossy().into()),
    )
    .unwrap();
    std::fs::remove_file(l.g.files.aside(1)).unwrap();
    pigeond::asides::release_at(&l.g, &l.t, pigeond::asides::KEEP_SECONDS).await;
    let entries =
        pigeon_core::journal::entries(&pigeond::journal_file::read(&l.g.files.install_journal()));
    assert!(!entries[&1].released);
    std::fs::write(&kept, b"save temporarily unavailable").unwrap();
    pigeond::asides::release_at(&l.g, &l.t, pigeond::asides::KEEP_SECONDS).await;
    assert!(pigeond::journal_file::read(&l.g.files.install_journal()).is_empty());
}

#[tokio::test]
async fn recovered_saves_survive_a_second_recovery_batch() {
    let Some(l) = laptop().await else { return };
    seed_recovered(&l, b"first irreplaceable save");
    pigeond::evict::recover(&l.g, &l.t).await;
    pigeond::asides::release_at(&l.g, &l.t, pigeond::asides::KEEP_SECONDS).await;
    assert!(
        std::fs::read(l.g.files.install_journal())
            .unwrap()
            .is_empty()
    );
    seed_recovered(&l, b"second irreplaceable save");
    pigeond::evict::recover(&l.g, &l.t).await;
    pigeond::asides::release_at(&l.g, &l.t, pigeond::asides::KEEP_SECONDS).await;
    storage_trace(
        "Archive",
        serde_json::json!({
            "saved": [bytes_hash(b"first irreplaceable save"), bytes_hash(b"second irreplaceable save")],
            "present": recovered_hashes(&l),
        }),
    );
    let kept: Vec<Vec<u8>> = std::fs::read_dir(l.g.files.recovered())
        .unwrap()
        .map(|e| std::fs::read(e.unwrap().path()).unwrap())
        .collect();
    assert!(
        kept.contains(&b"first irreplaceable save".to_vec()),
        "the second recovery overwrote the first saved content; remaining: {kept:?}"
    );
}

#[tokio::test]
async fn recovery_preserves_an_aside_changed_after_install_proof() {
    let Some(l) = laptop().await else { return };
    write(&l, "a.txt", b"old");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let aside = l.g.files.aside(1);
    std::fs::rename(l.wt.join("a.txt"), &aside).unwrap();
    let stat = pigeon_sys::fs::lstat(&aside).unwrap().unwrap();
    let mut handle = std::fs::OpenOptions::new()
        .write(true)
        .open(&aside)
        .unwrap();
    let b = Begin {
        seq: 1,
        path: "a.txt".into(),
        old_key: Some(
            "SHA256E-s3--cba06b5736faf67e54b07b561eae94395e774c517a7d910a54369e1263ccfbd4".into(),
        ),
        new_key: Some("new-key".into()),
        stat: Some(pigeon_core::journal::Stat {
            dev: stat.id.dev,
            ino: stat.id.ino,
            size: stat.size,
            mtime_ns: i128::from(stat.mtime_ns),
        }),
        stage: Some(l.g.files.stage(1).to_string_lossy().into()),
        aside: aside.to_string_lossy().into(),
    };
    // Crash state: rename-in happened, but the end was not appended.
    write(&l, "a.txt", b"installed");
    for r in [Record::Begin(b), Record::Proved(vec![1])] {
        pigeond::journal_file::append(&l.g.files.install_journal(), &r).unwrap();
    }
    use std::io::Write;
    handle.write_all(b"new user save via old handle").unwrap();
    handle.sync_all().unwrap();
    pigeond::evict::recover(&l.g, &l.t).await;
    let kept: Vec<Vec<u8>> = std::fs::read_dir(l.g.files.recovered())
        .unwrap()
        .map(|e| std::fs::read(e.unwrap().path()).unwrap())
        .collect();
    assert!(
        aside.exists() || kept.contains(&b"new user save via old handle".to_vec()),
        "recovery unlinked the changed aside while its writer still held it open"
    );
    let now = pigeond::util::now() + pigeond::asides::KEEP_SECONDS;
    pigeond::asides::release_at(&l.g, &l.t, now).await;
    let saved_hash = file_hash(&aside);
    let journal_entries =
        pigeon_core::journal::entries(&pigeond::journal_file::read(&l.g.files.install_journal()));
    storage_trace(
        "Release",
        serde_json::json!({
            "saved": [saved_hash], "present": [file_hash(&aside)], "written": true,
            "released": journal_entries[&1].released,
        }),
    );
    assert!(
        aside.exists(),
        "an open writer blocks release even after the keep period"
    );
    drop(handle);
    pigeond::asides::release_at(&l.g, &l.t, now).await;
    assert!(!aside.exists());
    let journal_entries =
        pigeon_core::journal::entries(&pigeond::journal_file::read(&l.g.files.install_journal()));
    storage_trace(
        "Release",
        serde_json::json!({
            "saved": [saved_hash], "present": recovered_hashes(&l), "written": false,
            "released": journal_entries.get(&1).is_none_or(|e| e.released),
        }),
    );
    let keys = pigeond::retention::keys(&l.g).await.unwrap();
    let expected = pigeond::net::sync::sha256_file(&l.g.files.recovered().join("1")).unwrap();
    assert!(
        keys.iter()
            .any(|k| pigeond::net::sync::key_sha256(k) == Some(expected.as_str())),
        "the recovered save has a durable signed reference"
    );
    let main = l.git.rev("refs/heads/main").await;
    pigeond::asides::release_at(&l.g, &l.t, now).await;
    assert_eq!(
        l.git.rev("refs/heads/main").await,
        main,
        "release replay creates no duplicate version"
    );
}

#[tokio::test]
async fn failed_capture_does_not_let_install_replace_the_uncaptured_save() {
    let Some(mut l) = laptop().await else { return };
    write(&l, "a.txt", b"one\n");
    let c1 = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    write(&l, "a.txt", b"my uncommitted save\n");
    // Inject a failing commit-tree, after annex add has staged the save.
    let bin = l.dir.path().join("failing-git");
    std::fs::create_dir(&bin).unwrap();
    let git = bin.join("git");
    std::fs::write(&git, "#!/bin/sh\nif [ \"$1\" = commit-tree ]; then exit 73; fi\nexec \"$PIGEON_REVIEW_REAL_GIT\" \"$@\"\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
    let real_git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("git"))
        .find(|p| p.is_file())
        .unwrap();
    l.g.git
        .repo
        .env
        .extra
        .push(("PIGEON_REVIEW_REAL_GIT".into(), real_git.into_os_string()));
    l.g.git.repo.env.path_first.insert(0, bin);
    let failed = capture::capture(&l.g, &dev(1).0, &l.t).await;
    assert!(failed.is_err(), "the capture must have failed: {failed:?}");
    l.g.git.repo.env.path_first.clear();
    assert_eq!(
        l.git.rev("refs/heads/main").await.as_deref(),
        Some(c1.as_str())
    );
    assert!(pigeond::capture_file::read(&l.g).is_some());
    assert!(
        capture::dirty(&l.g).await.unwrap().is_empty(),
        "annex add made the index look clean"
    );
    let mut tree = l.git.tree(&c1).await.unwrap();
    tree.insert(
        "a.txt".into(),
        version(&l, "a.txt", b"Ben's incoming version\n", true).await,
    );
    bens(&l, &tree, &c1, 60).await;
    let saved_hash = file_hash(&l.wt.join("a.txt"));
    install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    storage_trace(
        "Capture",
        serde_json::json!({
            "pending": pigeond::capture_file::pending(&l.g), "live": file_hash(&l.wt.join("a.txt")),
            "saved": saved_hash, "captured": false,
        }),
    );
    assert_eq!(
        read(&l, "a.txt"),
        "my uncommitted save\n",
        "install replaced the save from the failed capture before undo_interrupted ran"
    );
}

#[tokio::test]
async fn placement_copies_historical_captured_versions_too() {
    let Some(l) = laptop().await else { return };
    let remote = l.dir.path().join("custodian.git");
    std::fs::create_dir(&remote).unwrap();
    let rg = Git::new(&remote, GitEnv::default());
    rg.run(&["init", "--bare", "-q"]).await.unwrap();
    rg.run(&["annex", "init", "backup"]).await.unwrap();
    let uuid = rg.config("annex.uuid").await.unwrap();
    let mut doc = l.g.roster().unwrap();
    doc.roster.k = 1;
    doc.roster.devices.get_mut(&dev(3)).unwrap().annex_uuid = Some(uuid);
    pigeond::change::commit_roster(&l.g, &doc, "pigeon: configure test custodian", &l.t)
        .await
        .unwrap();
    l.g.git
        .run(&["remote", "add", &dev(3).0, remote.to_str().unwrap()])
        .await
        .unwrap();
    write(&l, "a.txt", b"first version while offline\n");
    let first = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let old_key = l.git.keys(&first).await.unwrap()["a.txt"].clone();
    write(&l, "a.txt", b"second version while offline\n");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let main = l.git.rev("refs/heads/main").await.unwrap();
    let current_key = l.git.keys(&main).await.unwrap()["a.txt"].clone();
    l.git
        .run(&[
            "annex",
            "copy",
            "--json",
            "--to",
            &dev(3).0,
            "--key",
            &current_key,
        ])
        .await
        .unwrap();
    l.git
        .update_ref(&format!("refs/pigeon/peers/{}/main", dev(3).0), &main, None)
        .await
        .unwrap();
    l.g.lock().heard.insert(dev(3).0, pigeond::util::now());
    let current = pigeond::status::observe(&l.g, &dev(1).0, "").await;
    assert!(
        current
            .iter()
            .all(|o| matches!(o.state, pigeon_core::state::State::Protected(_)))
    );
    let report = pigeond::status::report(&l.g, &dev(1).0, "", false).await;
    assert_ne!(
        report.level,
        pigeon_proto::api::Level::Safe,
        "unprotected history must affect the headline"
    );
    let made = pigeond::custody::place(&l.g, &dev(1).0).await;
    assert!(made > 0, "the current version must have been transferred");
    assert!(
        rg.run(&["annex", "contentlocation", &old_key])
            .await
            .is_ok(),
        "placement omitted a captured historical version even though its local object is present"
    );
    let report = pigeond::status::report(&l.g, &dev(1).0, "", false).await;
    assert_eq!(report.level, pigeon_proto::api::Level::Safe);
    // Losing this custodian endangers historical content too.
    let mut doc = l.g.roster().unwrap();
    let removed = doc.roster.devices.remove(&dev(3)).unwrap();
    doc.roster.removed.insert(dev(3), removed.annex_uuid);
    pigeond::change::commit_roster(&l.g, &doc, "pigeon: remove backup", &l.t)
        .await
        .unwrap();
    // A failed durable endangered record must not hide this custodian's
    // location log before the next repair can retry it.
    let endangered = l.g.root().join("endangered.json");
    std::fs::create_dir(&endangered).unwrap();
    assert_eq!(pigeond::repair::repair(&l.g, &dev(1).0).await, 0);
    std::fs::remove_dir(&endangered).unwrap();
    assert_eq!(pigeond::repair::repair(&l.g, &dev(1).0).await, 1);
    assert!(pigeond::admission::endangered(&l.g).contains(&old_key));
}

#[tokio::test]
async fn unreadable_capture_intent_blocks_all_disk_writers() {
    let Some(l) = laptop().await else { return };
    write(&l, "a.txt", b"my save\n");
    std::fs::write(l.g.root().join("capture.json"), b"{torn").unwrap();
    assert!(capture::recover(&l.g).await.is_err());
    assert!(pigeond::capture_file::pending(&l.g));
    assert_eq!(
        install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap(),
        Installed::default()
    );
    assert!(
        pigeond::evict::free(&l.g, &dev(1).0, "", &l.t)
            .await
            .is_err()
    );
    assert!(pigeond::evict::keep(&l.g, &l.t).await.is_err());
    assert_eq!(pigeond::thin::rethin(&l.g, &dev(1).0, &l.t).await, 0);
    assert_eq!(read(&l, "a.txt"), "my save\n");
}

#[tokio::test]
async fn retained_keys_equal_all_reachable_trees_after_deletion_and_proposals() {
    let Some(l) = laptop().await else { return };
    let mut tips = Vec::new();
    for bytes in [b"first".as_slice(), b"second", b"third"] {
        write(&l, "a.txt", bytes);
        tips.push(
            capture::capture(&l.g, &dev(1).0, &l.t)
                .await
                .unwrap()
                .commit
                .unwrap(),
        );
    }
    std::fs::remove_file(l.wt.join("a.txt")).unwrap();
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let main = l.git.rev("refs/heads/main").await.unwrap();
    let mut tree = l.git.tree(&main).await.unwrap();
    tree.insert(
        "proposal.txt".into(),
        version(&l, "proposal.txt", b"suggested", true).await,
    );
    let p = pigeond::proposal::write(&l.g, &dev(1).0, &main, &tree)
        .await
        .unwrap();
    tips.push(p);
    let mut expected = std::collections::BTreeSet::new();
    for c in tips {
        expected.extend(l.git.keys(&c).await.unwrap().into_values());
    }
    let enumerated = pigeond::retention::keys(&l.g).await.unwrap();
    storage_trace(
        "Retention",
        serde_json::json!({"expected": expected, "enumerated": enumerated}),
    );
    assert_eq!(enumerated, expected);
}

fn storage_trace(action: &str, vars: serde_json::Value) {
    let Some(root) = std::env::var_os("PIGEON_STORAGE_TRACE_DIR") else {
        return;
    };
    let dir = PathBuf::from(root).join(action);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("trace.ndjson");
    let previous = std::fs::read_to_string(&path).unwrap_or_default();
    let event = serde_json::json!({
        "spec": "StorageBoundary", "device": "disk", "clock": previous.lines().count() + 1,
        "action": action, "vars": vars,
    });
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{event}").unwrap();
}

fn file_hash(path: &std::path::Path) -> String {
    pigeond::net::sync::sha256_file(path).unwrap()
}

fn recovered_hashes(l: &Laptop) -> Vec<String> {
    std::fs::read_dir(l.g.files.recovered())
        .unwrap()
        .map(|e| file_hash(&e.unwrap().path()))
        .collect()
}

fn bytes_hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[tokio::test]
async fn status_uses_one_ref_snapshot_for_current_files_and_history() {
    let Some(mut l) = laptop().await else { return };
    let before = l.git.rev("refs/heads/main").await.unwrap();
    let mut tree = l.git.tree(&before).await.unwrap();
    tree.insert(
        "b.txt".into(),
        version(&l, "b.txt", b"newly captured\n", true).await,
    );
    let tree = merge::write_tree(&l.git, &tree).await.unwrap();
    let next =
        l.g.git
            .commit_tree(&tree, &[&before], "pigeon: captured meanwhile")
            .await
            .unwrap();
    let bin = l.dir.path().join("advance-main");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("git");
    // Advance main at the ref-listing boundary. Reading current paths
    // before that boundary and history after it used to yield Local with
    // an empty path list, and could omit intermediate historical versions.
    std::fs::write(&wrapper, "#!/bin/sh\nif [ \"$1\" = for-each-ref ] && [ ! -e \"$PIGEON_REVIEW_SWITCHED\" ]; then\n  \"$PIGEON_REVIEW_REAL_GIT\" update-ref refs/heads/main \"$PIGEON_REVIEW_NEXT\" || exit\n  touch \"$PIGEON_REVIEW_SWITCHED\"\nfi\nexec \"$PIGEON_REVIEW_REAL_GIT\" \"$@\"\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let real = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("git"))
        .find(|p| p.is_file())
        .unwrap();
    let marker = l.dir.path().join("switched");
    l.g.git.repo.env.extra.extend([
        ("PIGEON_REVIEW_REAL_GIT".into(), real.into_os_string()),
        ("PIGEON_REVIEW_NEXT".into(), next.into()),
        (
            "PIGEON_REVIEW_SWITCHED".into(),
            marker.clone().into_os_string(),
        ),
    ]);
    l.g.git.repo.env.path_first.insert(0, bin);
    let report = pigeond::status::report(&l.g, &dev(1).0, "", false).await;
    assert!(marker.exists());
    assert_eq!(report.state, pigeon_proto::api::State::Local);
    assert_eq!(
        report
            .versions
            .iter()
            .map(|v| v.path.as_str())
            .collect::<Vec<_>>(),
        ["b.txt"]
    );
}

#[tokio::test]
async fn a_failed_recovered_commit_keeps_the_file_and_retries() {
    let Some(mut l) = laptop().await else { return };
    seed_recovered(&l, b"saved while the daemon was down");
    let bin = l.dir.path().join("fail-recovery-commit");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("git");
    std::fs::write(&wrapper, "#!/bin/sh\nif [ \"$1\" = commit-tree ]; then exit 73; fi\nexec \"$PIGEON_REVIEW_REAL_GIT\" \"$@\"\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let real = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("git"))
        .find(|p| p.is_file())
        .unwrap();
    l.g.git
        .repo
        .env
        .extra
        .push(("PIGEON_REVIEW_REAL_GIT".into(), real.into_os_string()));
    l.g.git.repo.env.path_first.insert(0, bin);
    pigeond::asides::release_at(&l.g, &l.t, pigeond::asides::KEEP_SECONDS).await;
    let journal = pigeond::journal_file::read(&l.g.files.install_journal());
    let entries = pigeon_core::journal::entries(&journal);
    let entry = &entries[&1];
    assert!(
        !entry.released,
        "a failed capture cannot release its journal entry"
    );
    let kept = PathBuf::from(entry.preserved.as_ref().unwrap());
    assert_eq!(
        std::fs::read(&kept).unwrap(),
        b"saved while the daemon was down"
    );
    l.g.git.repo.env.path_first.clear();
    pigeond::asides::release_at(&l.g, &l.t, pigeond::asides::KEEP_SECONDS).await;
    assert!(pigeond::journal_file::read(&l.g.files.install_journal()).is_empty());
    let key = file_hash(&kept);
    assert!(
        pigeond::retention::keys(&l.g)
            .await
            .unwrap()
            .iter()
            .any(|k| pigeond::net::sync::key_sha256(k) == Some(key.as_str()))
    );
}
