//! `pigeon leave` (09 §9.7): Ana's laptop, in one group, may not create a
//! second; alone in the group, it leaves it at once and creates it again
//! with the same key. Once a backup machine and her second laptop joined,
//! the second laptop leaves. Its removal reaches the first laptop, which
//! drops it from the machines; the leaver then forgets the group, keeps its
//! files as plain files without `.git`, and, in no group any more, deletes
//! its device key. A group whose roster is in a format this release no
//! longer reads is left at once, as nobody could read its removal.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::{SERIAL, dirs, eventually_within, invitation, join, logs, pigeon, said, start, until};
use pigeon_proto::Response;
use pigeon_proto::api::State;
use std::path::Path;

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
            Response::Machines { machines, .. } => machines.into_iter().map(|m| m.name).collect(),
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
    let other = root.join("other");
    let second = pigeon(&a1, &["create", "Other", other.to_str().unwrap()]);
    assert_eq!(
        problem(&second).as_deref(),
        Some("in-another-group"),
        "{second:?}"
    );
    assert!(!other.join(".git").exists(), "nothing was written");

    let key = a1.home.join("config/device-key");
    let before = std::fs::read_to_string(&key).unwrap();
    let alone = pigeon(&a1, &["leave", "--yes"]);
    assert_eq!(
        said(&alone, "left", "group").as_deref(),
        Some("Family"),
        "{alone:?}"
    );
    assert!(!fa.join(".git").exists(), "the folder keeps no repository");
    assert_eq!(
        std::fs::read_to_string(fa.join("a.txt")).unwrap(),
        "first\n"
    );
    let again = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&again, "created", "group").is_some(), "{again:?}");
    assert_eq!(
        std::fs::read_to_string(&key).unwrap(),
        before,
        "the running key is written back"
    );

    join(&a1, &c1, &all);
    until(&a1, State::Protected, &all);
    let code = invitation(&a1);
    let joined = pigeon(
        &a2,
        &[
            "join",
            &code,
            "--person",
            "ana",
            "--yes",
            fa2.to_str().unwrap(),
            "--name",
            "ana-2",
        ],
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
        // The config forgets the group first; the repository moves and
        // the key goes after it.
        || {
            !std::fs::read_to_string(&config)
                .unwrap_or_default()
                .contains("[[groups]]")
                && !fa2.join(".git").exists()
                && !a2.home.join("config/device-key").exists()
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

#[test]
fn a_peer_that_never_learned_the_join_cannot_acknowledge_the_leave() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let s = sockets.path();
    let a1 = start(&root, s, "a1");
    let c1 = start(&root, s, "c1");
    let a2 = start(&root, s, "a2");
    let folder = root.join("ana");
    let created = pigeon(&a1, &["create", "Family", folder.to_str().unwrap()]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&a1, &c1, &[&a1, &c1]);
    // The backup returns with a valid roster from before the second join.
    // Its first NotInRoster after the leave is therefore no acknowledgement.
    let _ = pigeon(&c1, &["pause"]);
    drop(c1);
    let code = invitation(&a1);
    let joined = pigeon(
        &a2,
        &[
            "join",
            &code,
            "--person",
            "ana",
            "--yes",
            root.join("ana-2").to_str().unwrap(),
            "--name",
            "ana-2",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    let _ = pigeon(&a1, &["pause"]);
    let leaving = pigeon(&a2, &["leave", "--yes"]);
    assert!(said(&leaving, "leaving", "name").is_some(), "{leaving:?}");
    let c1 = start(&root, s, "c1");
    let all = [&a1, &c1, &a2];
    eventually_within(
        60,
        || {
            logs(&[&c1])
                .lines()
                .filter(|line| line.contains("stream-refused") && line.contains("NotInRoster"))
                .count()
                >= 3
                || !root.join("ana-2/.git").exists()
        },
        "the stale backup did not refuse the unknown joining device",
        &all,
    );
    // Let several one-second leave rounds observe that refusal while the
    // only peer that knows the membership still cannot accept the removal.
    for _ in 0..20 {
        assert!(
            root.join("ana-2/.git").exists(),
            "a stale roster acknowledged an unseen removal\n{}",
            logs(&all)
        );
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let again = pigeon(&a2, &["leave", "--yes"]);
    assert!(
        again
            .iter()
            .any(|r| matches!(r, Response::Said { message } if message.id == "leaving-already")),
        "a stale roster acknowledged an unseen removal: {again:?}\n{}",
        logs(&all)
    );
    let _ = pigeon(&a1, &["resume"]);
    eventually_within(
        180,
        || !machines(&a1).contains(&"ana-2".to_owned()),
        "the actual member did not accept the removal",
        &all,
    );
    eventually_within(
        180,
        || !root.join("ana-2/.git").exists(),
        "the witnessed leave did not finish",
        &all,
    );
}

fn git(path: &Path, args: &[&str]) -> String {
    // Share the daemon's bounded update-index/write-tree lock retry.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let git = pigeond::git::Git::new(path, pigeon_git::GitEnv::default());
    runtime.block_on(git.run(args)).unwrap().trim().into()
}

/// Moves `folder`'s `main` to a roster in `format = 1`, as releases before
/// 0.5.10 wrote it.
fn in_an_earlier_format(folder: &Path) {
    let roster = folder.join(".pigeon/roster.toml");
    let text = std::fs::read_to_string(&roster).unwrap();
    assert!(text.starts_with("format = 2\n"), "{text}");
    std::fs::write(&roster, text.replacen("format = 2", "format = 1", 1)).unwrap();
    let blob = git(
        folder,
        &["hash-object", "-w", "--no-filters", ".pigeon/roster.toml"],
    );
    let entry = format!("100644,{blob},.pigeon/roster.toml");
    git(folder, &["update-index", "--cacheinfo", &entry]);
    let tree = git(folder, &["write-tree"]);
    git(folder, &["config", "user.name", "old"]);
    git(folder, &["config", "user.email", "old@pigeon"]);
    let commit = git(
        folder,
        &[
            "commit-tree",
            &tree,
            "-p",
            "main",
            "-m",
            "an earlier release",
        ],
    );
    git(folder, &["update-ref", "refs/heads/main", &commit]);
}

#[test]
fn a_group_in_an_earlier_roster_format_is_left_at_once() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let s = sockets.path();
    let fa = root.join("ana");
    std::fs::create_dir_all(&fa).unwrap();
    std::fs::write(fa.join("a.txt"), "first\n").unwrap();
    let f = fa.to_str().unwrap();
    let a1 = start(&root, s, "a1");
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    drop(a1);
    in_an_earlier_format(&fa);
    let a1 = start(&root, s, "a1");
    let left = pigeon(&a1, &["leave", "--yes"]);
    assert_eq!(
        said(&left, "left-unreadable", "group").as_deref(),
        Some("Family"),
        "{left:?}\n{}",
        logs(&[&a1])
    );
    assert!(!fa.join(".git").exists(), "the folder keeps no repository");
    assert_eq!(
        std::fs::read_to_string(fa.join("a.txt")).unwrap(),
        "first\n"
    );
    let again = pigeon(&a1, &["create", "Family", f, "--k", "1"]);
    assert!(said(&again, "created", "group").is_some(), "{again:?}");
}
