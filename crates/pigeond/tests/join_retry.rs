//! A laptop join that fails takes back what it made, so a retry in the same
//! folder works (09 §9.6): Carol's first two tries are refused, with an
//! invitation whose secret is not the group's, and neither leaves a
//! repository, a group or, for a folder the join made, the folder; the
//! third, with the real invitation, joins, and is told that the computer
//! name she asked for was taken. Ana created the group under the name she
//! gave, not `$USER`'s.
//!
//! A failed backup join retains its unfinished repository and original UUID;
//! a later retry preserves files added while setup was interrupted.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use std::path::Path;

use common::{SERIAL, dirs, invitation, logs, pigeon, said, start};
use pigeon_proto::Response;

fn problem(rs: &[Response]) -> Option<String> {
    rs.iter().find_map(|r| match r {
        Response::Problem(p) => Some(p.code.clone()),
        _ => None,
    })
}

/// Whether `folder` holds anything.
fn has_files(folder: &Path) -> bool {
    std::fs::read_dir(folder).is_ok_and(|mut d| d.next().is_some())
}

#[test]
fn a_failed_backup_join_keeps_new_files_and_retries_with_its_original_uuid() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let ana = start(&root, sockets.path(), "ana");
    let backup = start(&root, sockets.path(), "backup");
    let folder = root.join("files");
    let created = pigeon(
        &ana,
        &[
            "create",
            "Family",
            folder.to_str().unwrap(),
            "--k",
            "1",
            "--person",
            "Ana",
        ],
    );
    assert!(said(&created, "created", "group").is_some());
    let code = invitation(&ana);
    let mut forged = pigeon_proto::join::decode(&code).unwrap();
    forged.secret = "ab".repeat(32);
    let forged = pigeon_proto::join::encode(&forged);
    let repo = root.join("backup.git");
    let first = pigeon(
        &backup,
        &[
            "join",
            "--backup-machine",
            &forged,
            repo.to_str().unwrap(),
            "--person",
            "Ana",
            "--yes",
        ],
    );
    assert!(problem(&first).is_some());
    assert!(repo.join("pigeon/joining").is_file());
    let uuid = || {
        let result = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["config", "annex.uuid"])
            .output()
            .unwrap();
        assert!(result.status.success());
        result.stdout
    };
    let before = uuid();
    let note = repo.join("personal-note.txt");
    std::fs::write(&note, b"notes added during the interrupted setup\0\n").unwrap();
    drop(backup);
    let backup = start(&root, sockets.path(), "backup");
    let second = pigeon(
        &backup,
        &[
            "join",
            "--backup-machine",
            &code,
            repo.to_str().unwrap(),
            "--person",
            "Ana",
            "--yes",
        ],
    );
    assert!(said(&second, "joined", "group").is_some(), "{second:?}");
    assert_eq!(uuid(), before);
    assert_eq!(
        std::fs::read(&note).unwrap(),
        b"notes added during the interrupted setup\0\n"
    );
    assert!(!repo.join("pigeon/joining").exists());
}

#[test]
fn a_failed_join_leaves_nothing_and_a_retry_in_the_same_folder_joins() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let ana = start(&root, sockets.path(), "ana");
    let carol = start(&root, sockets.path(), "carol");
    let all = [&ana, &carol];
    let family = root.join("ana-files");
    let created = pigeon(
        &ana,
        &[
            "create",
            "Family",
            family.to_str().unwrap(),
            "--k",
            "1",
            "--name",
            "ana-laptop",
            "--person",
            "Ana Martin",
        ],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    let people = pigeon(&ana, &["people"]);
    assert!(
        people
            .iter()
            .any(|r| matches!(r, Response::People { people }
            if people.iter().any(|p| p.name == "Ana Martin" && p.admin))),
        "{people:?}"
    );
    let code = invitation(&ana);
    let real = pigeon_proto::join::decode(&code).unwrap();
    let folder = root.join("carol-files");
    let mut forged = real.clone();
    forged.secret = "ab".repeat(32);
    let forged = pigeon_proto::join::encode(&forged);
    let refused = |rs: &[Response]| {
        assert!(problem(rs).is_some(), "{rs:?}\n{}", logs(&all));
    };

    // Refused, in a folder the join makes: the folder goes.
    let first = pigeon(
        &carol,
        &[
            "join",
            &forged,
            folder.to_str().unwrap(),
            "--person",
            "Carol",
        ],
    );
    refused(&first);
    assert!(!folder.exists(), "the folder the join made stays");

    // Refused again, in a folder that existed, empty: it stays, empty.
    std::fs::create_dir_all(&folder).unwrap();
    let second = pigeon(
        &carol,
        &[
            "join",
            &forged,
            folder.to_str().unwrap(),
            "--person",
            "Carol",
        ],
    );
    refused(&second);
    assert!(
        folder.is_dir() && !has_files(&folder),
        "the join left files"
    );
    let machines = pigeon(&carol, &["machines"]);
    assert_eq!(
        problem(&machines).as_deref(),
        Some("not-in-group"),
        "{machines:?}"
    );

    // The real invitation, in the same folder, with a name Ana's computer
    // already has.
    let third = pigeon(
        &carol,
        &[
            "join",
            &code,
            folder.to_str().unwrap(),
            "--name",
            "ana-laptop",
            "--person",
            "Carol",
        ],
    );
    let given = said(&third, "joined-renamed", "name")
        .unwrap_or_else(|| panic!("{third:?}\n{}", logs(&all)));
    assert!(given.starts_with("ana-laptop-"), "{given}");
    assert_eq!(
        said(&third, "joined-renamed", "asked").as_deref(),
        Some("ana-laptop")
    );
    assert!(folder.join(".pigeon/roster.toml").exists());
    let kept = carol.home.join("state/joins").join(&real.group);
    assert!(!kept.exists(), "the join's annex UUID is kept after it");
}
