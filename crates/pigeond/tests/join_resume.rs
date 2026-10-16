//! A join cut short resumes (09 §9.6): a computer is in the group only
//! once its welcome is checked. Killed after the admitter signed it in, or
//! just before `config.toml` records it, Bob's laptop is in no group, and
//! the same invitation finishes the join in the same repository, with the
//! annex UUID the admitter's roster holds. A group an earlier release
//! recorded without `main`, which it could neither join nor leave, is such
//! a join again once the daemon starts.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::*;
use pigeon_proto::Response;
use std::path::Path;

const POINTS: [&str; 2] = ["join.after-welcome", "join.before-config"];

fn git(path: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}

fn configured(home: &Path) -> Vec<pigeond::config::GroupEntry> {
    pigeond::config::load(&home.join("config/config.toml"))
        .unwrap()
        .groups
}

/// Ana's group, made on `lap` in `folder`: its ID.
fn created(lap: &Daemon, folder: &Path) -> String {
    std::fs::create_dir_all(folder).unwrap();
    std::fs::write(folder.join("a.txt"), b"first\n").unwrap();
    let answer = pigeon(
        lap,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&answer, "created", "group").is_some(), "{answer:?}");
    git(folder, &["rev-list", "--max-parents=0", "main"])
}

fn joins(d: &Daemon, code: &str, folder: &Path) -> Vec<Response> {
    pigeon(
        d,
        &["join", code, folder.to_str().unwrap(), "--person", "bob"],
    )
}

/// Bob's join into `folder`, killed at `point`: the annex UUID it left.
fn cut_short(root: &Path, sockets: &Path, code: &str, folder: &Path, point: &str) -> String {
    let mut bob = start_with_crash(root, sockets, "bob", Some(point));
    let _ = joins(&bob, code, folder);
    assert_crashed(&mut bob);
    drop(bob);
    git(folder, &["config", "annex.uuid"])
}

fn resumed(lap: &Daemon, bob: &Daemon, code: &str, folder: &Path, uuid: &str, what: &str) {
    let answer = joins(bob, code, folder);
    assert!(
        said(&answer, "joined", "group").is_some(),
        "{what}: {answer:?}\n{}",
        logs(&[lap, bob])
    );
    assert_eq!(git(folder, &["config", "annex.uuid"]), uuid, "{what}");
    assert!(!folder.join(".git/pigeon/joining").exists(), "{what}");
    let entries = configured(&bob.home);
    assert_eq!(entries.len(), 1, "{what}: {entries:?}");
    assert_eq!(entries[0].path, folder, "{what}");
}

#[test]
fn a_join_cut_short_resumes_in_its_repository() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for point in POINTS {
        let Some((_tmp, root, sockets)) = dirs() else {
            return;
        };
        let lap = start(&root, sockets.path(), "lap");
        created(&lap, &root.join("ana"));
        let code = invitation(&lap);
        let folder = root.join("files");
        let uuid = cut_short(&root, sockets.path(), &code, &folder, point);
        assert!(
            configured(&root.join("bob")).is_empty(),
            "{point}: recorded before its welcome was checked"
        );
        let bob = start(&root, sockets.path(), "bob");
        resumed(&lap, &bob, &code, &folder, &uuid, point);
    }
}

#[test]
fn a_group_recorded_without_main_is_joined_again() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let group = created(&lap, &root.join("ana"));
    let code = invitation(&lap);
    let folder = root.join("files");
    let uuid = cut_short(&root, sockets.path(), &code, &folder, POINTS[0]);
    // What an earlier release left: the group recorded, without a mark.
    let marker = folder.join(".git/pigeon/joining");
    std::fs::remove_file(&marker).unwrap();
    let entry = pigeond::config::GroupEntry {
        id: group.clone(),
        path: folder.clone(),
    };
    pigeond::config::add_group(&root.join("bob/config/config.toml"), &entry).unwrap();
    let bob = start(&root, sockets.path(), "bob");
    assert!(configured(&bob.home).is_empty(), "{}", logs(&[&bob]));
    assert_eq!(std::fs::read_to_string(&marker).unwrap().trim(), group);
    resumed(&lap, &bob, &code, &folder, &uuid, "recorded without main");
}
