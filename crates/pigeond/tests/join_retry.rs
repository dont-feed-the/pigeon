//! A join that fails takes back what it made, so a retry in the same
//! folder works (09 §9.6): Carol's first try cannot reach Ana's computer,
//! her second is refused, and neither leaves a repository, a group or, for
//! a folder the join made, the folder; the third, with the real invitation,
//! joins, and is told that the computer name she asked for was taken. Ana
//! created the group under the name she gave, not `$USER`'s.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use std::path::Path;

use common::{SERIAL, dirs, logs, pigeon, said, start};
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
    let answer = pigeon(&ana, &["invite", "Carol"]);
    let code = said(&answer, "invitation", "code").unwrap_or_else(|| panic!("{answer:?}"));
    let real = pigeond::invite::decode(&code).unwrap();
    let folder = root.join("carol-files");

    // Ana's computer cannot be reached: an address nobody listens on, and
    // an invitation that expires in a few seconds so the try ends.
    let mut offline = real.clone();
    offline.addresses = vec!["127.0.0.1:9".into()];
    offline.expires = pigeond::util::now() + 8;
    let first = pigeon(
        &carol,
        &[
            "join",
            &pigeond::invite::encode(&offline),
            folder.to_str().unwrap(),
        ],
    );
    assert_eq!(
        problem(&first).as_deref(),
        Some("inviter-offline"),
        "{first:?}"
    );
    assert!(!folder.exists(), "the folder the join made stays");

    // Ana's computer refuses: a secret it never gave. The folder existed
    // before, empty, and stays, empty.
    std::fs::create_dir_all(&folder).unwrap();
    let mut forged = real.clone();
    forged.secret = "ab".repeat(32);
    let second = pigeon(
        &carol,
        &[
            "join",
            &pigeond::invite::encode(&forged),
            folder.to_str().unwrap(),
        ],
    );
    assert_eq!(
        problem(&second).as_deref(),
        Some("invite-unknown"),
        "{second:?}"
    );
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
