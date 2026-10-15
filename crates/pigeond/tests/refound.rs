//! `pigeon refound` (09 §9.9, ADR 0047): Ana's group has a backup machine
//! and Bob's laptop, which is no admin's. Bob refounds it: his repository
//! then serves a new group whose genesis is a root commit naming the old
//! group and `main`, with Bob as its only admin and his laptop as its only
//! machine. The old `main` stays archived and the folder keeps its files.
//! Ana's folder then joins the new group in place by Bob's invitation,
//! keeping its annex UUID and archiving its own `main`, and gets Bob's
//! next file.
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

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn people(d: &common::Daemon) -> Vec<(String, bool, Vec<String>)> {
    pigeon(d, &["people"])
        .into_iter()
        .flat_map(|r| match r {
            Response::People { people } => people
                .into_iter()
                .map(|p| (p.name, p.admin, p.machines))
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn a_member_refounds_the_group_in_place() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let s = sockets.path();
    let (a1, c1, b1) = (
        start(&root, s, "a1"),
        start(&root, s, "c1"),
        start(&root, s, "b1"),
    );
    let all = [&a1, &c1, &b1];
    let (fa, fb) = (root.join("ana"), root.join("bob"));
    std::fs::create_dir_all(&fa).unwrap();
    std::fs::write(fa.join("a.txt"), "first\n").unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&a1, &c1, &all);
    until(&a1, State::Protected, &all);
    let answer = pigeon(&a1, &["invite", "Bob"]);
    let code = said(&answer, "invitation", "code").unwrap_or_else(|| panic!("{answer:?}"));
    let joined = pigeon(
        &b1,
        &["join", &code, fb.to_str().unwrap(), "--name", "bob-1"],
    );
    assert!(
        said(&joined, "joined", "group").is_some(),
        "{joined:?}\n{}",
        logs(&all)
    );
    eventually_within(
        180,
        || std::fs::read_to_string(fb.join("a.txt")).is_ok_and(|t| t == "first\n"),
        "Bob's laptop never got a.txt",
        &all,
    );
    let old = git(&fb, &["rev-list", "--max-parents=0", "main"]);
    let old_main = git(&fb, &["rev-parse", "main"]);
    let uuid = git(&fb, &["config", "annex.uuid"]);

    let unasked = pigeon(&b1, &["refound"]);
    assert_eq!(
        problem(&unasked).as_deref(),
        Some("needs-confirmation"),
        "{unasked:?}"
    );
    let done = pigeon(&b1, &["refound", fb.to_str().unwrap(), "--yes"]);
    assert!(
        said(&done, "refounded", "group").is_some(),
        "{done:?}\n{}",
        logs(&all)
    );

    let new = git(&fb, &["rev-parse", "main"]);
    assert_ne!(new, old_main);
    assert_eq!(git(&fb, &["rev-list", "--count", "main"]), "1");
    let body = git(&fb, &["log", "-1", "--format=%B", "main"]);
    assert!(
        body.contains(&format!("Pigeon-Refound: {old} {old_main}")),
        "{body}"
    );
    assert_eq!(
        git(
            &fb,
            &["rev-parse", &format!("refs/pigeon/archive/{old}/main")]
        ),
        old_main
    );
    assert_eq!(git(&fb, &["config", "annex.uuid"]), uuid);
    let config = std::fs::read_to_string(b1.home.join("config/config.toml")).unwrap();
    assert!(config.contains(&new) && !config.contains(&old), "{config}");
    assert_eq!(
        people(&b1),
        vec![("Bob".to_owned(), true, vec!["bob-1".to_owned()])]
    );
    assert_eq!(
        std::fs::read_to_string(fb.join("a.txt")).unwrap(),
        "first\n"
    );
    // The old group goes on without Bob's new history.
    eventually_within(
        60,
        || {
            let status = pigeon(&b1, &["status"]);
            problem(&status).is_none()
        },
        "the new group never answered status",
        &all,
    );
    assert_eq!(git(&fa, &["rev-list", "--max-parents=0", "main"]), old);

    // Ana's folder joins the new group in place, once she accepts Bob.
    let ana_main = git(&fa, &["rev-parse", "main"]);
    let ana_uuid = git(&fa, &["config", "annex.uuid"]);
    let answer = pigeon(&b1, &["invite", "Ana"]);
    let code = said(&answer, "invitation", "code").unwrap_or_else(|| panic!("{answer:?}"));
    let unasked = pigeon(&a1, &["join", &code, f]);
    assert_eq!(
        problem(&unasked).as_deref(),
        Some("needs-confirmation"),
        "{unasked:?}"
    );
    let joined = pigeon(&a1, &["join", &code, f, "--name", "ana-1", "--yes"]);
    assert!(
        said(&joined, "joined", "group").is_some(),
        "{joined:?}\n{}",
        logs(&all)
    );
    assert_eq!(git(&fa, &["rev-list", "--max-parents=0", "main"]), new);
    assert_eq!(
        git(
            &fa,
            &["rev-parse", &format!("refs/pigeon/archive/{old}/main")]
        ),
        ana_main
    );
    assert_eq!(git(&fa, &["config", "annex.uuid"]), ana_uuid);
    let config = std::fs::read_to_string(a1.home.join("config/config.toml")).unwrap();
    assert!(config.contains(&new) && !config.contains(&old), "{config}");
    assert_eq!(
        std::fs::read_to_string(fa.join("a.txt")).unwrap(),
        "first\n"
    );
    std::fs::write(fb.join("b.txt"), "from bob\n").unwrap();
    eventually_within(
        180,
        || std::fs::read_to_string(fa.join("b.txt")).is_ok_and(|t| t == "from bob\n"),
        "Ana's folder never got b.txt from the new group",
        &all,
    );
    let names: Vec<String> = people(&b1).into_iter().map(|p| p.0).collect();
    assert_eq!(names, ["Ana", "Bob"]);
}
