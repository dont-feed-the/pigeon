//! Actual SIGKILL on both sides of the erasure main CAS. A new Group
//! replays the durable marker before capture, preserving dirty files and
//! unrelated staged entries. This checks process crashes, not power loss.
#![cfg(feature = "lab")]

use super::*;
use pigeon_proto::erase::Selection;
use std::os::unix::process::ExitStatusExt;

#[tokio::test]
async fn child() {
    let Some(folder) = std::env::var_os("PIGEON_ERASE_TEST_FOLDER") else {
        return;
    };
    let id = std::env::var("PIGEON_ERASE_TEST_GROUP").unwrap();
    pigeond::git::set_signer(key(1));
    let g = Group::open(&id, std::path::Path::new(&folder), &GitEnv::default())
        .await
        .unwrap();
    let main = g.git.rev("refs/heads/main").await.unwrap();
    merge::judge_history(&g, std::slice::from_ref(&main))
        .await
        .unwrap();
    let t = Tracer::new(g.root().join("test-traces"), &dev(1).0);
    let selection = Selection::Path(
        std::path::Path::new(&folder)
            .join("Shared")
            .to_string_lossy()
            .into(),
    );
    let plan = pigeond::erase_preview::plan(&g, &dev(1).0, &selection)
        .await
        .unwrap();
    pigeond::erase_commit::submit(&g, &dev(1).0, plan, &t)
        .await
        .unwrap();
    panic!("crash seam was not reached");
}

#[tokio::test]
async fn interrupted_erasure_recovers_without_recapturing_or_touching_private_index_entries() {
    for point in ["erase.before-main", "erase.after-main"] {
        let Some(l) = laptop().await else { return };
        let upgraded = purge_upgrade::upgrade(&l.g).await;
        let clean = b"reviewed clean bytes\n";
        let dirty = b"accepted bytes\n";
        let mut tree = l.g.git.tree(&upgraded).await.unwrap();
        tree.insert(
            "Shared/clean.txt".into(),
            version(&l, "clean.txt", clean, true).await,
        );
        tree.insert(
            "Shared/dirty.txt".into(),
            version(&l, "dirty.txt", dirty, true).await,
        );
        let original = super::purge_content::signed(&l, &tree, &[&upgraded], T0 + 100, 1).await;
        merge::judge_history(&l.g, std::slice::from_ref(&original))
            .await
            .unwrap();
        l.g.git
            .update_ref("refs/heads/main", &original, Some(&upgraded))
            .await
            .unwrap();
        install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
        let private = b"uncaptured private save\n";
        std::fs::write(l.wt.join("Shared/dirty.txt"), private).unwrap();
        pigeond::change::stage_file(
            &l.git,
            Some(&l.wt),
            "staged-private.txt",
            b"private staged bytes\n",
        )
        .await
        .unwrap();
        let staged = l.git.index().await.unwrap()["staged-private.txt"].clone();
        let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "erase_restart::child", "--nocapture"])
            .env("PIGEON_ERASE_TEST_FOLDER", &l.wt)
            .env("PIGEON_ERASE_TEST_GROUP", &l.g.id)
            .env(pigeon_sys::lab::CRASH_VAR, format!("{point}:1"))
            .kill_on_drop(true);
        let output = tokio::time::timeout(std::time::Duration::from_secs(30), command.output())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            output.status.signal(),
            Some(9),
            "{point}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(l.g.root().join("erase-install.toml").exists());
        let g = Group::open(&l.g.id, &l.wt, &GitEnv::default())
            .await
            .unwrap();
        pigeond::erase_commit::recover(&g, &dev(1).0, &l.t)
            .await
            .unwrap();
        pigeond::erase_commit::recover(&g, &dev(1).0, &l.t)
            .await
            .unwrap();
        assert!(!g.root().join("erase-install.toml").exists());
        assert_eq!(
            std::fs::read(l.wt.join("Shared/dirty.txt")).unwrap(),
            private
        );
        assert_eq!(l.git.index().await.unwrap()["staged-private.txt"], staged);
        if point == "erase.before-main" {
            assert_eq!(g.git.rev("refs/heads/main").await, Some(original));
            assert_eq!(std::fs::read(l.wt.join("Shared/clean.txt")).unwrap(), clean);
        } else {
            assert!(!l.wt.join("Shared/clean.txt").exists());
            let index = l.git.index().await.unwrap();
            assert!(!index.contains_key("Shared/clean.txt"));
            let head = g.git.rev("refs/heads/main").await.unwrap();
            assert_eq!(
                pigeond::purge_state::active(&g, &head).await.unwrap().len(),
                1
            );
        }
    }
}

#[tokio::test]
async fn unknown_erasure_markers_are_preserved_and_stop_capture() {
    let Some(l) = laptop().await else { return };
    let path = l.g.root().join("erase-install.toml");
    let main = l.g.git.rev("refs/heads/main").await;
    for text in [
        "format = [torn",
        "format = 2\nbefore = 'old'\nafter = 'new'\n",
    ] {
        std::fs::write(&path, text).unwrap();
        assert!(capture::capture(&l.g, &dev(1).0, &l.t).await.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        assert_eq!(l.g.git.rev("refs/heads/main").await, main);
    }
}
