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

use common::{SERIAL, dirs, eventually_within, invitation, join, logs, pigeon, said, start, until};
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
    let code = invitation(&a1);
    let joined = pigeon(
        &b1,
        &[
            "join",
            &code,
            "--person",
            "Bob",
            fb.to_str().unwrap(),
            "--name",
            "bob-1",
        ],
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
    // Archive history includes an earlier version and a real declined
    // proposal. Neither may rely on a live proposal ref after refounding.
    std::fs::write(fa.join("a.txt"), "second\n").unwrap();
    eventually_within(
        120,
        || std::fs::read(fb.join("a.txt")).is_ok_and(|b| b == b"second\n"),
        "second version reaches Bob",
        &all,
    );
    eventually_within(
        120,
        || {
            let journal = fb.join(".git/pigeon/install.ndjson");
            std::fs::metadata(&journal).is_ok_and(|m| m.len() == 0)
                && std::process::Command::new("git")
                    .arg("-C")
                    .arg(&fb)
                    .env("GIT_OPTIONAL_LOCKS", "0")
                    .args(["status", "--porcelain", "--", "a.txt"])
                    .output()
                    .is_ok_and(|o| o.status.success() && o.stdout.is_empty())
        },
        "incoming installation fully settles before participant edit",
        &all,
    );
    // ADR 0071 deliberately treats an edit within ten seconds of an
    // incoming install as a conflict copy. This fixture needs a suggestion.
    std::thread::sleep(std::time::Duration::from_secs(12));
    std::fs::write(fb.join("a.txt"), "Bob's proposal\n").unwrap();
    let pending = || {
        pigeon(&a1, &["suggestions"])
            .into_iter()
            .find_map(|r| match r {
                Response::Proposals { proposals } => Some(proposals),
                _ => None,
            })
            .unwrap_or_default()
    };
    eventually_within(120, || !pending().is_empty(), "Bob's real suggestion", &all);
    let proposal = pending()[0].id.clone();
    let declined = pigeon(&a1, &["decline", &proposal]);
    assert!(said(&declined, "declined-suggestion", "author").is_some());
    eventually_within(
        120,
        || {
            std::fs::read_to_string(fb.join(".git/pigeon/conflicts.json"))
                .is_ok_and(|s| s.contains("\"conflict\""))
        },
        "Bob learns the decline",
        &all,
    );
    let resolved = pigeon(
        &b1,
        &[
            "resolve",
            fb.join("a.txt").to_str().unwrap(),
            "--take-group",
        ],
    );
    assert!(problem(&resolved).is_none(), "{resolved:?}");
    assert_eq!(std::fs::read(fb.join("a.txt")).unwrap(), b"second\n");
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
        "second\n"
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
    let code = invitation(&b1);
    let unasked = pigeon(&a1, &["join", &code, "--person", "Ana", f]);
    assert_eq!(
        problem(&unasked).as_deref(),
        Some("needs-confirmation"),
        "{unasked:?}"
    );
    let joined = pigeon(
        &a1,
        &[
            "join", &code, "--person", "Ana", f, "--name", "ana-1", "--yes",
        ],
    );
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
        "second\n"
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
    // A fresh computer receives complete archive metadata, including the
    // declined proposal, and can read the older version. Repeat through a
    // second refounding to test both signed links and proposal ref relay.
    let cara = start(&root, s, "cara");
    let fc = root.join("cara-files");
    let code = invitation(&b1);
    let joined = pigeon(
        &cara,
        &[
            "join",
            &code,
            "--person",
            "Cara",
            fc.to_str().unwrap(),
            "--yes",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    eventually_within(
        180,
        || std::fs::read(fc.join("a.txt")).is_ok_and(|b| b == b"second\n"),
        "Cara receives current bytes",
        &[&a1, &b1, &c1, &cara],
    );
    let older = || {
        said(
            &pigeon(
                &cara,
                &["view", fc.join("a.txt").to_str().unwrap(), "--version", "1"],
            ),
            "view-ready",
            "path",
        )
    };
    eventually_within(
        120,
        || older().is_some_and(|p| std::fs::read(p).is_ok_and(|b| b == b"first\n")),
        "declined archive stays verifiable on a fresh laptop",
        &[&a1, &b1, &c1, &cara],
    );
    let refounded = pigeon(&cara, &["refound", fc.to_str().unwrap(), "--yes"]);
    assert!(
        said(&refounded, "refounded", "group").is_some(),
        "{refounded:?}"
    );
    let daria = start(&root, s, "daria");
    let fd = root.join("daria-files");
    let code = invitation(&cara);
    let joined = pigeon(
        &daria,
        &[
            "join",
            &code,
            "--person",
            "Daria",
            fd.to_str().unwrap(),
            "--yes",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    eventually_within(
        180,
        || {
            let copy = said(
                &pigeon(
                    &daria,
                    &["view", fd.join("a.txt").to_str().unwrap(), "--version", "1"],
                ),
                "view-ready",
                "path",
            );
            copy.is_some_and(|p| std::fs::read(p).is_ok_and(|b| b == b"first\n"))
        },
        "both archive links and proposal proofs reach another fresh laptop",
        &[&a1, &b1, &c1, &cara, &daria],
    );
    eventually_within(
        180,
        || std::fs::read(fd.join("a.txt")).is_ok_and(|b| b == b"second\n"),
        "the fresh laptop installs the current version after reading archived history",
        &[&a1, &b1, &c1, &cara, &daria],
    );
}
