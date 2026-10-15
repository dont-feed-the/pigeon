//! Observed driver runs for CaptureInstall: real crash/restart boundaries,
//! rollback, roll-forward, deletion and capture of a changed kept inode.

use super::*;
use std::os::unix::process::ExitStatusExt;

#[tokio::test]
#[ignore = "subprocess entered by driver_traces_replay_crashes"]
async fn install_trace_child() {
    let wt = PathBuf::from(std::env::var_os("PIGEON_TRACE_TEST_WT").unwrap());
    let id = std::env::var("PIGEON_TRACE_TEST_GROUP").unwrap();
    let g = Group::open(&id, &wt, &GitEnv::default()).await.unwrap();
    pigeond::git::set_signer(key(1));
    let t = Tracer::new(wt.parent().unwrap().join("traces"), &dev(1).0);
    if let Ok(key) = std::env::var("PIGEON_TRACE_TEST_RESTORE") {
        install::replace_for_lab(&g, "a.txt", &key, Some(&key), None, &t)
            .await
            .unwrap();
    } else {
        install::incoming(&g, &dev(1).0, &t).await.unwrap();
    }
    panic!("the configured crash point was not reached");
}

#[tokio::test]
async fn driver_traces_replay_crashes() {
    for (point, deletion, forward) in [
        ("install.after-begin", false, false),
        ("install.after-aside", false, false),
        ("install.after-proof", false, true),
        ("install.after-rename", false, true),
        ("install.before-index-observation", false, true),
        ("install.after-done", false, true),
        ("install.before-done-observation", false, true),
        ("install.after-proof", true, true),
    ] {
        let Some(l) = laptop().await else { return };
        write(&l, "a.txt", b"old\n");
        let c1 = capture::capture(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .commit
            .unwrap();
        let old = l.git.index().await.unwrap();
        let mut tree = l.git.tree(&c1).await.unwrap();
        if deletion {
            tree.remove("a.txt");
        } else {
            tree.insert("a.txt".into(), version(&l, "a.txt", b"new\n", true).await);
        }
        bens(&l, &tree, &c1, 60).await;
        let status = tokio::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "tracing::install_trace_child",
                "--nocapture",
            ])
            .env("PIGEON_TRACE_TEST_WT", &l.wt)
            .env("PIGEON_TRACE_TEST_GROUP", &l.g.id)
            .env("PIGEON_LAB_CRASH", format!("{point}:1"))
            .status()
            .await
            .unwrap();
        assert_eq!(status.signal(), Some(9), "{point}, deletion={deletion}");
        // A new tracer reads its clock from durable events, as a restarted
        // daemon does; the killed child never flushed its clock.json.
        let t = Tracer::new(l.dir.path().join("traces"), &dev(1).0);
        pigeond::evict::recover(&l.g, &t).await;
        assert!(!pigeond::evict::unsettled(&l.g));
        let index = l.git.index().await.unwrap();
        let wanted = if forward { &tree } else { &old };
        assert_eq!(
            index.get("a.txt"),
            wanted.get("a.txt"),
            "{point}: recovery must replay the pointer, not just a sparse bit"
        );
        if forward && deletion {
            assert!(!l.wt.join("a.txt").exists());
        } else {
            assert_eq!(read(&l, "a.txt"), if forward { "new\n" } else { "old\n" });
        }
        let before = l.git.index().await.unwrap();
        pigeond::evict::recover(&l.g, &t).await;
        assert_eq!(l.git.index().await.unwrap(), before, "replay is idempotent");
    }
}

#[tokio::test]
async fn driver_traces_restore_after_deletion_crash() {
    let Some(l) = laptop().await else { return };
    write(&l, "a.txt", b"old\n");
    let base = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let key = l.git.keys(&base).await.unwrap()["a.txt"].clone();
    std::fs::remove_file(l.wt.join("a.txt")).unwrap();
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert!(!l.git.index().await.unwrap().contains_key("a.txt"));
    let status = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "tracing::install_trace_child",
            "--nocapture",
        ])
        .env("PIGEON_TRACE_TEST_WT", &l.wt)
        .env("PIGEON_TRACE_TEST_GROUP", &l.g.id)
        .env("PIGEON_TRACE_TEST_RESTORE", &key)
        .env("PIGEON_LAB_CRASH", "install.after-proof:1")
        .status()
        .await
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let t = Tracer::new(l.dir.path().join("traces"), &dev(1).0);
    pigeond::evict::recover(&l.g, &t).await;
    assert!(!pigeond::evict::unsettled(&l.g));
    assert_eq!(read(&l, "a.txt"), "old\n");
    assert!(!l.git.index().await.unwrap().contains_key("a.txt"));
    capture::capture(&l.g, &dev(1).0, &t).await.unwrap();
    let main = l.git.rev("refs/heads/main").await.unwrap();
    assert_eq!(l.git.keys(&main).await.unwrap()["a.txt"], key);
}

#[tokio::test]
async fn driver_traces_release_changed_aside() {
    use std::io::{Seek, SeekFrom, Write};
    let Some(l) = laptop().await else { return };
    write(&l, "a.txt", b"old\n");
    let c1 = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&c1).await.unwrap();
    tree.insert("a.txt".into(), version(&l, "a.txt", b"new\n", true).await);
    bens(&l, &tree, &c1, 60).await;
    assert_eq!(
        install::incoming(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .written,
        1
    );
    let records = pigeond::journal_file::read(&l.g.files.install_journal());
    let entries = pigeon_core::journal::entries(&records);
    let (_, entry) = entries.iter().next().unwrap();
    let mut writer = std::fs::OpenOptions::new()
        .write(true)
        .open(&entry.begin.aside)
        .unwrap();
    writer.seek(SeekFrom::Start(0)).unwrap();
    writer.write_all(b"late save through old inode\n").unwrap();
    writer.sync_all().unwrap();
    let now = entry.kept_at.unwrap() + pigeond::asides::KEEP_SECONDS;
    pigeond::asides::release_at(&l.g, &l.t, now).await;
    assert!(
        std::path::Path::new(&entry.begin.aside).exists(),
        "a writer blocks release"
    );
    drop(writer);
    pigeond::asides::release_at(&l.g, &l.t, now).await;
    let hashes = pigeond::retention::keys(&l.g).await.unwrap();
    use sha2::Digest;
    let saved: String = sha2::Sha256::digest(b"late save through old inode\n")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert!(
        hashes
            .iter()
            .any(|k| pigeond::net::sync::key_sha256(k) == Some(saved.as_str()))
    );
    assert!(!std::path::Path::new(&entry.begin.aside).exists());
    assert_eq!(read(&l, "a.txt"), "new\n");
}

#[tokio::test]
async fn driver_traces_free_keep_and_thin() {
    let Some(l) = laptop().await else { return };
    let remote = l.dir.path().join("backup.git");
    std::fs::create_dir(&remote).unwrap();
    let backup = Git::new(&remote, GitEnv::default());
    backup.run(&["init", "--bare", "-q"]).await.unwrap();
    let uuid = l.g.roster().unwrap().roster.devices[&dev(3)]
        .annex_uuid
        .clone()
        .unwrap();
    backup.run(&["config", "annex.uuid", &uuid]).await.unwrap();
    backup.run(&["annex", "init", "backup"]).await.unwrap();
    let mut doc = l.g.roster().unwrap();
    doc.roster.k = 1;
    pigeond::change::commit_roster(&l.g, &doc, "pigeon: set k", &l.t)
        .await
        .unwrap();
    l.git
        .run(&["remote", "add", &dev(3).0, remote.to_str().unwrap()])
        .await
        .unwrap();
    write(&l, "a.txt", b"protected contents\n");
    let main = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let k = l.git.keys(&main).await.unwrap()["a.txt"].clone();
    l.git
        .run(&["annex", "copy", "--json", "--to", &dev(3).0, "--key", &k])
        .await
        .unwrap();
    l.git
        .update_ref(&format!("refs/pigeon/peers/{}/main", dev(3).0), &main, None)
        .await
        .unwrap();
    l.g.lock().heard.insert(dev(3).0, pigeond::util::now());
    assert_eq!(
        pigeond::thin::rethin_for_lab(&l.g, &dev(1).0, &l.t).await,
        1
    );
    pigeond::evict::set_target(&l.g, "", true);
    assert_eq!(
        pigeond::evict::free(&l.g, &dev(1).0, "", &l.t)
            .await
            .unwrap()
            .hidden,
        1
    );
    assert!(!l.wt.join("a.txt").exists());
    pigeond::evict::set_target(&l.g, "", false);
    assert_eq!(pigeond::evict::keep(&l.g, &l.t).await.unwrap(), 1);
    assert_eq!(read(&l, "a.txt"), "protected contents\n");
}

#[tokio::test]
async fn driver_traces_roster_forks() {
    let Some(l) = laptop().await else { return };
    let genesis = l.git.rev("refs/heads/main").await.unwrap();
    let mut ana = l.g.roster().unwrap();
    ana.roster
        .persons
        .get_mut(&PersonId("ana".into()))
        .unwrap()
        .admin = false;
    let ours = pigeond::change::commit_roster(&l.g, &ana, "pigeon: retire Ana", &l.t)
        .await
        .unwrap();
    let mut ben = roster();
    ben.persons.get_mut(&PersonId("ben".into())).unwrap().admin = false;
    let bytes = roster_file::render(&RosterDoc { roster: ben }).unwrap();
    let oid = l
        .git
        .run_in(&["hash-object", "-w", "--stdin"], bytes.as_bytes())
        .await
        .unwrap();
    let mut tree = l.git.tree(&genesis).await.unwrap();
    tree.insert(
        ".pigeon/roster.toml".into(),
        Entry {
            mode: "100644".into(),
            kind: "blob".into(),
            oid: oid.trim().into(),
        },
    );
    let theirs = bens(&l, &tree, &genesis, 60).await;
    l.git
        .update_ref("refs/heads/main", &ours, Some(&theirs))
        .await
        .unwrap();
    merge::judge_history(&l.g, &[ours.clone(), theirs.clone()])
        .await
        .unwrap();
    let merged = merge::merge(&l.g, &ours, &theirs).await.unwrap().unwrap();
    l.git
        .update_ref("refs/heads/main", &merged, Some(&ours))
        .await
        .unwrap();
    l.g.reload_roster().await;
    pigeond::trace_roster::moved(&l.t, &l.g, "Merge", Some(&ours)).await;
    assert!(
        l.g.roster()
            .unwrap()
            .roster
            .persons
            .values()
            .any(|p| p.admin)
    );
}

#[tokio::test]
async fn driver_traces_observe_index_without_refreshing_it() {
    let Some(l) = laptop().await else { return };
    for path in ["sub/a.txt", "sub/b.txt", "sub/é.txt"] {
        write(&l, path, b"same\n");
    }
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    for version in ["2", "3", "4"] {
        l.git
            .run(&["update-index", "--index-version", version])
            .await
            .unwrap();
        let before = std::fs::read(l.g.git_dir.join("index")).unwrap();
        for path in ["sub/a.txt", "sub/b.txt", "sub/é.txt"] {
            let state = pigeond::trace_state::snapshot(&l.g, path, 0).await.unwrap();
            assert_ne!(state["index"], "", "index format {version}");
        }
        assert_eq!(std::fs::read(l.g.git_dir.join("index")).unwrap(), before);
        l.git
            .run(&["update-index", "--skip-worktree", "--", "sub/a.txt"])
            .await
            .unwrap();
        let before = std::fs::read(l.g.git_dir.join("index")).unwrap();
        let state = pigeond::trace_state::snapshot(&l.g, "sub/a.txt", 0)
            .await
            .unwrap();
        assert_eq!(
            state["hidden"], true,
            "observe the stored bit even while the file is visible"
        );
        assert_eq!(
            std::fs::read(l.g.git_dir.join("index")).unwrap(),
            before,
            "observing must not clear the bit"
        );
        l.git
            .run(&["update-index", "--no-skip-worktree", "--", "sub/a.txt"])
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn driver_traces_restore() {
    let Some(l) = laptop().await else { return };
    write(&l, "a.txt", b"old\n");
    let old = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let old = l.git.keys(&old).await.unwrap()["a.txt"].clone();
    write(&l, "a.txt", b"new\n");
    let new = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let new = l.git.keys(&new).await.unwrap()["a.txt"].clone();
    assert!(
        install::replace_for_lab(&l.g, "a.txt", &old, Some(&new), None, &l.t)
            .await
            .unwrap()
    );
    assert_eq!(read(&l, "a.txt"), "old\n");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let main = l.git.rev("refs/heads/main").await.unwrap();
    assert_eq!(l.git.keys(&main).await.unwrap()["a.txt"], old);
}

#[tokio::test]
async fn driver_traces_take_group() {
    let Some(l) = laptop().await else { return };
    write(&l, "a.txt", b"old\n");
    let base = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&base).await.unwrap();
    let entry = version(&l, "a.txt", b"group\n", true).await;
    tree.insert("a.txt".into(), entry.clone());
    bens(&l, &tree, &base, 60).await;
    write(&l, "a.txt", b"personal\n");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    let div = pigeond::conflicts_file::read(&l.g).unwrap().paths["a.txt"].clone();
    assert!(
        install::replace_for_lab(
            &l.g,
            "a.txt",
            div.group.as_deref().unwrap(),
            div.mine.as_deref(),
            Some(&entry),
            &l.t
        )
        .await
        .unwrap()
    );
    assert_eq!(read(&l, "a.txt"), "group\n");
    assert!(
        pigeond::retention::keys(&l.g)
            .await
            .unwrap()
            .contains(div.mine.as_ref().unwrap())
    );
}

#[tokio::test]
async fn driver_traces_recent_foreign_save() {
    let Some(l) = laptop().await else { return };
    let mut doc = l.g.roster().unwrap();
    doc.roster
        .persons
        .get_mut(&PersonId("ana".into()))
        .unwrap()
        .admin = false;
    let base = pigeond::change::commit_roster(&l.g, &doc, "retire Ana", &l.t)
        .await
        .unwrap();
    let mut tree = l.git.tree(&base).await.unwrap();
    tree.insert(
        "a.txt".into(),
        version(&l, "a.txt", b"Ben's first\n", true).await,
    );
    let first = bens(&l, &tree, &base, 60).await;
    install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    let entry = version(&l, "a.txt", b"Ben's second\n", true).await;
    tree.insert("a.txt".into(), entry.clone());
    bens(&l, &tree, &first, 120).await;
    install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    l.g.lock().installed.get_mut("a.txt").unwrap().0 = pigeond::util::now_ns();
    write(&l, "a.txt", b"Ana's immediate save\n");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let main = l.git.rev("refs/heads/main").await.unwrap();
    let captured = l.git.tree(&main).await.unwrap();
    assert_eq!(captured.get("a.txt"), Some(&entry));
    assert!(
        captured
            .keys()
            .any(|p| pigeon_core::names::is_copy_of(p, "a.txt"))
    );
}

#[tokio::test]
async fn driver_traces_presence_races() {
    let Some(l) = laptop().await else { return };
    write(&l, "a.txt", b"old\n");
    let base = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let old = l.git.keys(&base).await.unwrap()["a.txt"].clone();
    let mut tree = l.git.tree(&base).await.unwrap();
    let entry = version(&l, "a.txt", b"incoming\n", true).await;
    tree.insert("a.txt".into(), entry.clone());
    let added = version(&l, "new.txt", b"new group file\n", true).await;
    tree.insert("new.txt".into(), added.clone());
    let main = bens(&l, &tree, &base, 60).await;
    let keys = l.git.keys(&main).await.unwrap();
    // Directly entering the driver represents a dirty scan which ran
    // before this deletion. The old code recreated the file and lost it.
    std::fs::remove_file(l.wt.join("a.txt")).unwrap();
    assert!(
        !install::replace_for_lab(
            &l.g,
            "a.txt",
            &keys["a.txt"],
            Some(&old),
            Some(&entry),
            &l.t
        )
        .await
        .unwrap()
    );
    assert!(
        !l.wt.join("a.txt").exists(),
        "capture must see the deletion"
    );
    // The dual race must not move aside an untracked creation either.
    write(&l, "new.txt", b"personal creation\n");
    assert!(
        !install::replace_for_lab(&l.g, "new.txt", &keys["new.txt"], None, Some(&added), &l.t)
            .await
            .unwrap()
    );
    assert_eq!(read(&l, "new.txt"), "personal creation\n");
    assert!(!pigeond::evict::unsettled(&l.g));
}
