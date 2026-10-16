//! Repair only the old synthetic divergence of a hidden accepted file. Real
//! pending files, missing suggestions, unknown stats and differing modes stay.
#![allow(clippy::unwrap_used, reason = "tests")]

use super::*;
use crate::conflicts_file::{Divergence, Kind, State, Table};
use pigeon_core::authority::Entry;

struct Fixture {
    _root: tempfile::TempDir,
    g: Group,
    table: Table,
}

async fn fixture(copy_mode: &str, copy_name: &str) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let wt = root.path().join("files");
    std::fs::create_dir(&wt).unwrap();
    let git = crate::git::Git::new(&wt, pigeon_git::GitEnv::default());
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "fixture"]).await.unwrap();
    git.run(&["config", "user.email", "fixture@pigeon"])
        .await
        .unwrap();
    let store = root.path().join("pool");
    let group = "a".repeat(40);
    let source = root.path().join("source");
    std::fs::write(&source, b"mine").unwrap();
    let key = format!(
        "SHA256E-s4--{}.txt",
        crate::net::sync::sha256_file(&source).unwrap()
    );
    pool::retain(&store, &group, &key, &source, false).unwrap();
    let oid = git
        .run_in(
            &["hash-object", "-w", "--stdin"],
            format!("/annex/objects/{key}\n").as_bytes(),
        )
        .await
        .unwrap();
    let entry = Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: oid.trim().into(),
    };
    let other_key = format!("SHA256E-s4--{}.txt", "b".repeat(64));
    let oid = git
        .run_in(
            &["hash-object", "-w", "--stdin"],
            format!("/annex/objects/{other_key}\n").as_bytes(),
        )
        .await
        .unwrap();
    let other = Entry {
        oid: oid.trim().into(),
        ..entry.clone()
    };
    let copy = Entry {
        mode: copy_mode.into(),
        ..entry.clone()
    };
    let tree = [("a.txt".into(), other), (copy_name.into(), copy)].into();
    let tree = crate::merge::write_tree(&git, &tree).await.unwrap();
    let commit = git
        .run_in(&["commit-tree", &tree], b"accepted fixture\n")
        .await
        .unwrap();
    git.update_ref("refs/heads/main", commit.trim(), None)
        .await
        .unwrap();
    let mut g = Group::open(&group, &wt, &pigeon_git::GitEnv::default())
        .await
        .unwrap();
    g.shared = Some(crate::config::BridgeStorage {
        store,
        source_socket: root.path().join("offline.sock"),
    });
    crate::install::index_entry(&g, "a.txt", Some(&entry))
        .await
        .unwrap();
    git.run(&["update-index", "--skip-worktree", "--", "a.txt"])
        .await
        .unwrap();
    let table = Table {
        format: crate::conflicts_file::FORMAT,
        paths: [(
            "a.txt".into(),
            Divergence {
                kind: Kind::Concurrent,
                since: "2026-10-16T00:00:00Z".into(),
                state: State::Conflict,
                mine: Some(key),
                mine_stat: None,
                proposal: None,
                group: Some(other_key),
                installed: None,
                told: false,
            },
        )]
        .into(),
    };
    Fixture {
        _root: root,
        g,
        table,
    }
}

#[tokio::test]
async fn hidden_accepted_conflict_repair_unblocks_sparse_compaction() {
    let fixture = fixture("100644", "a (mine, 2026-10-16 00h00).txt").await;
    crate::conflicts_file::write(&fixture.g, &fixture.table).unwrap();
    compact(&fixture.g).await.unwrap();
    assert!(
        crate::conflicts_file::read(&fixture.g)
            .unwrap()
            .paths
            .is_empty()
    );
    assert!(!fixture.g.worktree.as_ref().unwrap().join("a.txt").exists());
    let key = fixture.table.paths["a.txt"].mine.as_ref().unwrap();
    let store = &fixture.g.shared.as_ref().unwrap().store;
    pool::verify(&pool::path(store, &fixture.g.id, key).unwrap(), key).unwrap();
    // Replaying recovery makes no extra version or overlay.
    compact(&fixture.g).await.unwrap();
}

#[tokio::test]
async fn missing_or_changed_real_overlays_are_never_pruned_as_sparse_conflicts() {
    for case in [
        "suggestion",
        "stat",
        "visible",
        "symlink",
        "mode",
        "name",
        "not-hidden",
    ] {
        let mode = if case == "mode" { "100755" } else { "100644" };
        let name = if case == "name" {
            "unrelated.txt"
        } else {
            "a (mine, 2026-10-16 00h00).txt"
        };
        let mut fixture = fixture(mode, name).await;
        let d = fixture.table.paths.get_mut("a.txt").unwrap();
        match case {
            "suggestion" => d.kind = Kind::Suggestion,
            "stat" => d.mine_stat = Some([1, 2, 4, 5]),
            "visible" => std::fs::write(
                fixture.g.worktree.as_ref().unwrap().join("a.txt"),
                b"new uncaptured save",
            )
            .unwrap(),
            "symlink" => std::os::unix::fs::symlink(
                "missing-target",
                fixture.g.worktree.as_ref().unwrap().join("a.txt"),
            )
            .unwrap(),
            "not-hidden" => {
                fixture
                    .g
                    .git
                    .run(&["update-index", "--no-skip-worktree", "--", "a.txt"])
                    .await
                    .unwrap();
            }
            _ => {}
        }
        crate::conflicts_file::write(&fixture.g, &fixture.table).unwrap();
        assert!(compact(&fixture.g).await.is_err(), "{case}");
        assert_eq!(
            crate::conflicts_file::read(&fixture.g).unwrap(),
            fixture.table,
            "{case}"
        );
    }
}
