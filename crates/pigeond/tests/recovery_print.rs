//! An unreachable backup must not leave an unprinted paper key in the
//! roster, suppressing the advice that protects a sole admin (09 §9.9).
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, reason = "tests")]

mod common;
use common::*;
use pigeon_proto::{Response, api::State};

#[test]
fn an_unprinted_sheet_is_revoked_when_no_backup_receives_it() {
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
            .any(|a| matches!(a, Response::Problem(p) if p.code == "peer-unreachable")),
        "{failed:?}"
    );
    assert!(pigeon(&alice, &["machines"]).iter().all(|a| match a {
        Response::Machines { machines } => machines.iter().all(|m| !m.recovery),
        _ => true,
    }));
    assert!(pigeon(&alice, &["people"]).iter().any(|a| matches!(a,
        Response::People { people } if people.iter().any(|p| p.this_person && p.recovery_recommended))));
}
