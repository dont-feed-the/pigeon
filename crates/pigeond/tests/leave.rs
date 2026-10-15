//! `pigeon leave` (09 §9.7): Ana's only laptop may not leave, being the
//! group's last admin computer; once a backup machine and her second
//! laptop joined, the second laptop leaves. Its removal reaches the first
//! laptop, which drops it from the machines; the leaver then forgets the
//! group, keeps its files as plain files without `.git`, and, in no group
//! any more, deletes its device key.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::{SERIAL, dirs, eventually_within, join, logs, pigeon, said, start, until};
use pigeon_proto::Response;
use pigeon_proto::api::State;

fn problem(rs: &[Response]) -> Option<String> {
    rs.iter().find_map(|r| match r {
        Response::Problem(p) => Some(p.code.clone()),
        _ => None,
    })
}

fn machines(d: &common::Daemon) -> Vec<String> {
    pigeon(d, &["machines"])
        .into_iter()
        .flat_map(|r| match r {
            Response::Machines { machines } => machines.into_iter().map(|m| m.name).collect(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn a_laptop_leaves_once_a_peer_has_its_removal() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let s = sockets.path();
    let (a1, c1, a2) = (
        start(&root, s, "a1"),
        start(&root, s, "c1"),
        start(&root, s, "a2"),
    );
    let all = [&a1, &c1, &a2];
    let (fa, fa2) = (root.join("ana"), root.join("ana-2"));
    std::fs::create_dir_all(&fa).unwrap();
    std::fs::write(fa.join("a.txt"), "first\n").unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    let alone = pigeon(&a1, &["leave", "--yes"]);
    assert_eq!(
        problem(&alone).as_deref(),
        Some("leave-last-admin"),
        "{alone:?}"
    );

    join(&a1, &c1, &all);
    until(&a1, State::Protected, &all);
    let answer = pigeon(&a1, &["invite", "ana"]);
    let code = said(&answer, "invitation", "code").unwrap_or_else(|| panic!("{answer:?}"));
    let joined = pigeon(
        &a2,
        &["join", &code, fa2.to_str().unwrap(), "--name", "ana-2"],
    );
    assert!(
        said(&joined, "joined", "group").is_some(),
        "{joined:?}\n{}",
        logs(&all)
    );
    eventually_within(
        180,
        || std::fs::read_to_string(fa2.join("a.txt")).is_ok_and(|t| t == "first\n"),
        "the second laptop never got a.txt",
        &all,
    );
    assert!(machines(&a1).contains(&"ana-2".to_owned()));

    let unasked = pigeon(&a2, &["leave"]);
    assert_eq!(
        problem(&unasked).as_deref(),
        Some("needs-confirmation"),
        "{unasked:?}"
    );
    let leaving = pigeon(&a2, &["leave", "--yes"]);
    assert!(said(&leaving, "leaving", "name").is_some(), "{leaving:?}");
    let again = pigeon(&a2, &["leave", "--yes"]);
    assert!(
        again
            .iter()
            .any(|r| matches!(r, Response::Said { message } if message.id == "leaving-already")),
        "{again:?}"
    );
    eventually_within(
        180,
        || !machines(&a1).contains(&"ana-2".to_owned()),
        "the first laptop never accepted the removal",
        &all,
    );
    let config = a2.home.join("config/config.toml");
    eventually_within(
        180,
        || {
            !std::fs::read_to_string(&config)
                .unwrap_or_default()
                .contains("[[groups]]")
        },
        "the leaver never forgot the group",
        &all,
    );
    // Config removal starts cleanup; it does not acknowledge its end.
    eventually_within(
        180,
        || !fa2.join(".git").exists() && !a2.home.join("config/device-key").exists(),
        "the leaver never finished repository and key cleanup",
        &all,
    );
    assert_eq!(
        std::fs::read_to_string(fa2.join("a.txt")).unwrap(),
        "first\n"
    );
    assert!(!fa2.join(".git").exists(), "the folder keeps no repository");
    assert!(
        !a2.home.join("config/device-key").exists(),
        "the key is deleted"
    );
    let status = pigeon(&a2, &["status"]);
    assert_eq!(
        problem(&status).as_deref(),
        Some("not-in-group"),
        "{status:?}"
    );
}
