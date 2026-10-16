//! An unreachable backup must not leave an unprinted recovery sheet in the
//! roster, suppressing the advice that protects a sole admin: nothing is
//! signed until a backup machine answers (09 §9.9).
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, reason = "tests")]

mod common;
use common::*;
use pigeon_proto::{Response, api::State};

#[test]
fn no_sheet_is_signed_while_no_backup_machine_answers() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let alice = start(&root, sockets.path(), "alice");
    let bob = start(&root, sockets.path(), "bob");
    let backup = start(&root, sockets.path(), "backup");
    let folder = root.join("files");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("note.txt"), b"saved").unwrap();
    pigeon(
        &alice,
        &[
            "create",
            "Family",
            folder.to_str().unwrap(),
            "--k",
            "1",
            "--name",
            "Alice",
        ],
    );
    let ticket = invitation(&alice);
    pigeon(
        &bob,
        &[
            "join",
            &ticket,
            "--person",
            "Bob",
            root.join("bob-files").to_str().unwrap(),
            "--name",
            "Bob",
        ],
    );
    join(&bob, &backup, &[&alice, &bob, &backup]);
    until(&alice, State::Protected, &[&alice, &bob, &backup]);
    drop(backup);
    let failed = pigeon(&alice, &["recovery-sheet"]);
    assert!(
        failed
            .iter()
            .any(|a| matches!(a, Response::Problem(p) if p.code == "recovery-backup-offline")),
        "{failed:?}"
    );
    // Checked before anything is signed: the history has no sheet at all.
    let log = std::process::Command::new("git")
        .args(["log", "--format=%s", "main"])
        .current_dir(&folder)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&log.stdout);
    assert!(!log.contains("recovery sheet"), "{log}");
    assert!(pigeon(&alice, &["machines"]).iter().all(|a| match a {
        Response::Machines { machines, .. } => machines.iter().all(|m| !m.recovery),
        _ => true,
    }));
    assert!(pigeon(&alice, &["people"]).iter().any(|a| matches!(a,
        Response::People { people } if people.iter().any(|p| p.this_person && p.recovery_recommended))));
}
