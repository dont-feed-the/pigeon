//! In-place recovery across real SIGKILLs (09 §9.9, ADR 0047): a verified
//! welcome is sufficient for local replay, without another invitation.
//! Both a refounder's laptop and a returning bare backup keep their
//! annex identity, bytes and existing object inodes at every journal seam.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::*;
use pigeon_proto::{Response, api::State};
use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const POINTS: [&str; 4] = [
    "join-in-place.before-main",
    "join-in-place.after-main",
    "join-in-place.after-index",
    "join-in-place.after-config",
];

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

fn objects(root: &Path) -> BTreeMap<PathBuf, (u64, Vec<u8>)> {
    fn walk(base: &Path, at: &Path, out: &mut BTreeMap<PathBuf, (u64, Vec<u8>)>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            if metadata.is_dir() {
                walk(base, &entry.path(), out);
            } else if metadata.is_file() {
                out.insert(
                    entry.path().strip_prefix(base).unwrap().into(),
                    (metadata.ino(), std::fs::read(entry.path()).unwrap()),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn created(lap: &Daemon, folder: &Path) -> String {
    std::fs::create_dir_all(folder).unwrap();
    std::fs::write(
        folder.join("kept.txt"),
        b"recovery must keep these exact bytes\n",
    )
    .unwrap();
    let answer = pigeon(
        lap,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    said(&answer, "created", "group").unwrap_or_else(|| panic!("{answer:?}"))
}

fn configured(d: &Daemon) -> Vec<pigeond::config::GroupEntry> {
    pigeond::config::load(&d.home.join("config/config.toml"))
        .unwrap()
        .groups
}

fn verify(
    d: &Daemon,
    path: &Path,
    git_dir: &Path,
    old: &str,
    old_main: &str,
    uuid: &str,
) -> String {
    let root = git(path, &["rev-list", "--max-parents=0", "main"]);
    assert_ne!(root, old);
    assert_eq!(git(path, &["config", "annex.uuid"]), uuid);
    assert_eq!(
        git(
            path,
            &["rev-parse", &format!("refs/pigeon/archive/{old}/main")]
        ),
        old_main
    );
    assert_eq!(
        git(
            path,
            &[
                "rev-parse",
                &format!("refs/pigeon/archive/{old}/local/{old_main}")
            ]
        ),
        old_main
    );
    assert!(!git_dir.join("pigeon/in-place.json").exists());
    assert_eq!(
        configured(d),
        vec![pigeond::config::GroupEntry {
            id: root.clone(),
            path: path.into(),
            priority: Default::default(),
        }]
    );
    root
}

#[test]
fn a_refounder_restarts_at_each_durable_transition() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for point in POINTS {
        let Some((_tmp, root, sockets)) = dirs() else {
            return;
        };
        let lap = start(&root, sockets.path(), "lap");
        let folder = root.join("files");
        created(&lap, &folder);
        eventually_within(
            60,
            || git(&folder, &["ls-tree", "--name-only", "main"]).contains("kept.txt"),
            "file not captured",
            &[&lap],
        );
        drop(lap);
        let mut lap = start_with_crash(&root, sockets.path(), "lap", Some(point));
        let old = git(&folder, &["rev-list", "--max-parents=0", "main"]);
        let old_main = git(&folder, &["rev-parse", "main"]);
        let uuid = git(&folder, &["config", "annex.uuid"]);
        let inode = std::fs::metadata(folder.join("kept.txt")).unwrap().ino();
        let _ = pigeon(&lap, &["refound", folder.to_str().unwrap(), "--yes"]);
        assert_crashed(&mut lap);
        let intent = folder.join(".git/pigeon/in-place.json");
        assert!(intent.exists(), "{point}: no durable intent");
        drop(lap);
        let lap = start(&root, sockets.path(), "lap");
        verify(&lap, &folder, &folder.join(".git"), &old, &old_main, &uuid);
        assert_eq!(
            std::fs::metadata(folder.join("kept.txt")).unwrap().ino(),
            inode,
            "{point}"
        );
        assert_eq!(
            std::fs::read(folder.join("kept.txt")).unwrap(),
            b"recovery must keep these exact bytes\n"
        );
    }
}

#[test]
fn a_backup_rejoins_and_restarts_without_recopying_its_objects() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for point in POINTS {
        let Some((_tmp, root, sockets)) = dirs() else {
            return;
        };
        let lap = start(&root, sockets.path(), "lap");
        let vault = start(&root, sockets.path(), "vault");
        let folder = root.join("files");
        created(&lap, &folder);
        join(&lap, &vault, &[&lap, &vault]);
        // An empty accepted tree can already be Protected. The preservation
        // snapshot needs this actual captured file on its backup first.
        // Retain the original protection wait's 120-second budget.
        eventually_within(
            120,
            || {
                pigeon(&lap, &["status", folder.join("kept.txt").to_str().unwrap()])
                    .into_iter()
                    .any(|r| matches!(r, Response::Status(s) if
                        s.versions.iter().any(|v| v.path == "kept.txt" && v.state == State::Protected)))
            },
            "backup-rejoin fixture file is not protected",
            &[&lap, &vault],
        );
        let entry = configured(&vault).remove(0);
        let path = entry.path;
        let uuid = git(&path, &["config", "annex.uuid"]);
        let old_main = git(&folder, &["rev-parse", "main"]);
        eventually_within(
            60,
            || git(&path, &["rev-parse", "main"]) == old_main,
            "backup main did not converge",
            &[&lap, &vault],
        );
        let before = objects(&path.join("annex/objects"));
        assert!(!before.is_empty());
        drop(vault);
        // A stopped fetch may already have left this lock. Retain that
        // obstruction unchanged; otherwise create one for the fixture.
        // The importer must use its private ref without replacing either
        // lock's inode or bytes. This never rewrites a pre-existing lock.
        let refs = git(
            &path,
            &["for-each-ref", "--format=%(refname)", "refs/remotes/"],
        );
        let peer_ref = refs
            .lines()
            .find(|name| name.ends_with("/git-annex"))
            .unwrap();
        let lock = path.join(format!("{peer_ref}.lock"));
        let held = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock);
        use std::io::Write;
        let held = match held {
            Ok(mut file) => {
                file.write_all(b"owned by the metadata-fetch fixture\n")
                    .unwrap();
                Some(file)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
            Err(error) => panic!("cannot prepare metadata lock: {error}"),
        };
        let lock_before = (
            std::fs::metadata(&lock).unwrap().ino(),
            std::fs::read(&lock).unwrap(),
        );
        let mut vault = start_with_crash(&root, sockets.path(), "vault", Some(point));
        let answer = pigeon(&lap, &["refound", folder.to_str().unwrap(), "--yes"]);
        assert!(said(&answer, "refounded", "group").is_some(), "{answer:?}");
        let new = git(&folder, &["rev-list", "--max-parents=0", "main"]);
        let code = invitation(&lap);
        let _ = pigeon(
            &vault,
            &[
                "join",
                "--backup-machine",
                &code,
                path.to_str().unwrap(),
                "--name",
                "vault",
                "--person",
                "ana",
                "--yes",
            ],
        );
        assert_crashed(&mut vault);
        assert_eq!(
            (
                std::fs::metadata(&lock).unwrap().ino(),
                std::fs::read(&lock).unwrap()
            ),
            lock_before
        );
        drop(held);
        std::fs::remove_file(&lock).unwrap();
        assert!(path.join("pigeon/in-place.json").exists(), "{point}");
        drop(vault);
        // No admitter is available during replay; the invitation is
        // neither needed nor sent again by the restarted daemon.
        drop(lap);
        let vault = start(&root, sockets.path(), "vault");
        assert_eq!(
            verify(&vault, &path, &path, &entry.id, &old_main, &uuid),
            new
        );
        let after = objects(&path.join("annex/objects"));
        for (name, value) in before {
            assert_eq!(after.get(&name), Some(&value), "{point}: {name:?}");
        }
        let staging = std::fs::read_dir(vault.home.join("state"))
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("join-stage-"));
        assert!(!staging, "{point}: retained staging repository");
    }
}
