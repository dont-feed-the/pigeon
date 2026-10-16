//! A rejoined bare backup exposes verified old versions through read-only
//! history/view commands (09 §9.9), with no virtual folder on disk.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;
use common::*;
use pigeon_proto::{Response, api::State};

fn count(d: &Daemon, path: &str) -> usize {
    pigeon(d, &["history", path])
        .into_iter()
        .filter_map(|r| match r {
            Response::History { versions } => Some(versions.len()),
            _ => None,
        })
        .sum()
}

#[test]
fn a_bare_backup_reads_old_versions_after_rejoining_in_place() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let vault = start(&root, sockets.path(), "vault");
    let folder = root.join("files");
    std::fs::create_dir_all(folder.join("nested")).unwrap();
    let file = folder.join("nested/note.txt");
    std::fs::write(&file, b"one\r\n\0").unwrap();
    let created = pigeon(
        &lap,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&lap, &vault, &[&lap, &vault]);
    until(&lap, State::Protected, &[&lap, &vault]);
    std::fs::write(&file, b"two\r\n\0").unwrap();
    until(&lap, State::Protected, &[&lap, &vault]);
    let entry = pigeond::config::load(&vault.home.join("config/config.toml"))
        .unwrap()
        .groups
        .remove(0);
    let virtual_file = entry.path.join("nested/note.txt");
    let path = virtual_file.to_str().unwrap();
    assert!(!virtual_file.exists());
    eventually_within(
        60,
        || count(&vault, path) == 2,
        "backup did not expose both versions",
        &[&lap, &vault],
    );
    let answer = pigeon(&lap, &["refound", folder.to_str().unwrap(), "--yes"]);
    assert!(said(&answer, "refounded", "group").is_some(), "{answer:?}");
    let code = invitation(&lap);
    let answer = pigeon(
        &vault,
        &[
            "join",
            "--backup-machine",
            &code,
            entry.path.to_str().unwrap(),
            "--name",
            "vault",
            "--person",
            "ana",
            "--yes",
        ],
    );
    assert!(said(&answer, "joined", "group").is_some(), "{answer:?}");
    assert_eq!(count(&vault, path), 2);
    let answer = pigeon(&vault, &["view", path, "--version", "1"]);
    let copy = said(&answer, "view-ready", "path").unwrap_or_else(|| panic!("{answer:?}"));
    assert_eq!(std::fs::read(&copy).unwrap(), b"one\r\n\0");
    assert!(std::fs::metadata(copy).unwrap().permissions().readonly());
    assert!(!virtual_file.exists());
}
