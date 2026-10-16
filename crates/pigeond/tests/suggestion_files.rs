//! A suggestion decided file by file (ADR 0076, 04 §4.5, 07 §7.6), with
//! real daemons: Ana, an admin, creates the group with a backup machine and
//! invites Cid, who is not one. Cid edits three of Ana's files, one
//! suggestion. Ana sees what changes in each and accepts two of them with
//! `pigeon accept ID FILE…`, declining the third with a note. Cid's
//! accepted files end their suggestion, the declined one stays his, and
//! `pigeon suggestions` tells him which were taken, which not, and why.
//! He suggests two files again; Ana declines one with `pigeon decline ID
//! FILE…`, which accepts the other. A computer that joins later verifies
//! this history and gets the group's files.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use std::path::Path;

use common::{
    SERIAL, dirs, eventually_within, invitation, join, logs, person, pigeon, said, start, until,
};
use pigeon_proto::Response;
use pigeon_proto::api::{FileChange, Outcome, State, Suggestion};

/// How long a change may take to reach every device.
const WAIT: u64 = 120;

fn text(folder: &Path, name: &str) -> String {
    std::fs::read_to_string(folder.join(name)).unwrap_or_default()
}

fn listed(d: &common::Daemon) -> (Vec<Suggestion>, Vec<Outcome>) {
    match pigeon(d, &["suggestions"]).as_slice() {
        [
            Response::Proposals {
                proposals, decided, ..
            },
        ] => (proposals.clone(), decided.clone()),
        other => panic!("suggestions: {other:?}"),
    }
}

/// Whether Cid's divergence table holds `path` as `state`, or not at all.
fn table(folder: &Path, path: &str, state: Option<&str>) -> bool {
    let t = std::fs::read_to_string(folder.join(".git/pigeon/conflicts.json")).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&t).unwrap_or_default();
    let entry = &v["paths"][path];
    match state {
        None => entry.is_null(),
        Some(s) => entry["state"] == s,
    }
}

#[test]
fn some_files_of_a_suggestion_are_accepted_and_the_others_declined() {
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
    let (fa, fc) = (root.join("ana"), root.join("cid"));
    std::fs::create_dir_all(&fa).unwrap();
    for name in ["a", "b", "c"] {
        std::fs::write(fa.join(format!("{name}.txt")), format!("Ana's {name}\n")).unwrap();
    }
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&a1, &c1, &all);
    until(&a1, State::Protected, &all);
    let code = invitation(&a1);
    let joined = pigeon(
        &b1,
        &["join", &code, fc.to_str().unwrap(), "--person", "Cid"],
    );
    assert!(
        said(&joined, "joined", "group").is_some(),
        "{joined:?}\n{}",
        logs(&all)
    );
    eventually_within(
        WAIT,
        || {
            ["a", "b", "c"]
                .iter()
                .all(|n| text(&fc, &format!("{n}.txt")) == format!("Ana's {n}\n"))
        },
        "Cid never got Ana's files",
        &all,
    );

    // Cid edits the three files: one suggestion to Ana.
    for name in ["a", "b", "c"] {
        std::fs::write(fc.join(format!("{name}.txt")), format!("Cid's {name}\n")).unwrap();
    }
    eventually_within(
        WAIT,
        || {
            listed(&a1)
                .0
                .first()
                .is_some_and(|p| p.paths == ["a.txt", "b.txt", "c.txt"] && p.files.len() == 3)
        },
        "Ana never saw Cid's suggestion of three files",
        &all,
    );
    let (theirs, _) = listed(&a1);
    assert_eq!(theirs.len(), 1, "{theirs:?}");
    let first = theirs[0].clone();
    assert!(first.decide && !first.mine, "{first:?}");
    assert!(
        first
            .files
            .iter()
            .all(|f| f.change == FileChange::Changed && f.before == Some(8) && f.after == Some(8)),
        "{first:?}"
    );

    // A file the suggestion does not change cannot be chosen.
    let wrong = pigeon(&a1, &["accept", &first.id[..12], "nope.txt"]);
    assert!(
        wrong
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "suggestion-file-unknown")),
        "{wrong:?}"
    );
    // Cid may not decide his own.
    let refused = pigeon(&b1, &["accept", &first.id[..12], "a.txt"]);
    assert!(
        refused
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "not-allowed")),
        "{refused:?}"
    );

    // Ana accepts a.txt and c.txt (named in full), declining b.txt.
    let c_full = fa.join("c.txt");
    let decided = pigeon(
        &a1,
        &[
            "accept",
            &first.id[..12],
            "a.txt",
            c_full.to_str().unwrap(),
            "--note",
            "On garde la mienne.",
        ],
    );
    assert_eq!(
        said(&decided, "decided-some", "accepted-paths").as_deref(),
        Some("a.txt, c.txt"),
        "{decided:?}\n{}",
        logs(&all)
    );
    assert_eq!(
        said(&decided, "decided-some", "declined-paths").as_deref(),
        Some("b.txt")
    );
    eventually_within(
        WAIT,
        || text(&fa, "a.txt") == "Cid's a\n" && text(&fa, "c.txt") == "Cid's c\n",
        "Ana's folder never took the accepted files",
        &all,
    );
    assert_eq!(text(&fa, "b.txt"), "Ana's b\n");
    assert!(listed(&a1).0.is_empty());

    // On Cid's computer the accepted files end their suggestion, the
    // declined one is his to choose for, his file kept.
    eventually_within(
        WAIT,
        || {
            table(&fc, "a.txt", None)
                && table(&fc, "c.txt", None)
                && table(&fc, "b.txt", Some("conflict"))
        },
        "Cid's suggestions never ended as decided",
        &all,
    );
    assert_eq!(text(&fc, "b.txt"), "Cid's b\n");
    let (mine, told) = listed(&b1);
    assert!(mine.is_empty(), "{mine:?}");
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(!told[0].by.is_empty(), "{told:?}");
    assert_eq!(told[0].accepted, ["a.txt", "c.txt"]);
    assert_eq!(told[0].declined, ["b.txt"]);
    assert_eq!(told[0].note.as_deref(), Some("On garde la mienne."));

    // Cid suggests a.txt and c.txt again; Ana declines c.txt, which
    // accepts a.txt.
    std::fs::write(fc.join("a.txt"), "Cid's second a\n").unwrap();
    std::fs::write(fc.join("c.txt"), "Cid's second c\n").unwrap();
    eventually_within(
        WAIT,
        || {
            listed(&a1)
                .0
                .first()
                .is_some_and(|p| p.paths == ["a.txt", "c.txt"])
        },
        "Ana never saw Cid's second suggestion",
        &all,
    );
    let second = listed(&a1).0[0].id.clone();
    let declined = pigeon(
        &a1,
        &["decline", &second[..12], "c.txt", "--note", "Pas celle-ci."],
    );
    assert_eq!(
        said(&declined, "decided-some", "declined-paths").as_deref(),
        Some("c.txt"),
        "{declined:?}\n{}",
        logs(&all)
    );
    eventually_within(
        WAIT,
        || text(&fa, "a.txt") == "Cid's second a\n",
        "Ana's folder never took the file not declined",
        &all,
    );
    assert_eq!(text(&fa, "c.txt"), "Cid's c\n");
    eventually_within(
        WAIT,
        || listed(&b1).1.iter().any(|o| o.id == second),
        "Cid was never told of the second decision",
        &all,
    );
    let (_, told) = listed(&b1);
    let o = told.iter().find(|o| o.id == second).unwrap();
    assert_eq!(o.accepted, ["a.txt"]);
    assert_eq!(o.declined, ["c.txt"]);
    assert_eq!(o.note.as_deref(), Some("Pas celle-ci."));

    // A computer that joins later verifies this history and gets the files.
    let a2 = start(&root, s, "a2");
    let code = invitation(&a1);
    let owner = person(&a1);
    let later = root.join("ana-2");
    let joined = pigeon(
        &a2,
        &[
            "join",
            &code,
            later.to_str().unwrap(),
            "--person",
            &owner,
            "--yes",
        ],
    );
    let everyone = [&a1, &b1, &c1, &a2];
    assert!(
        said(&joined, "joined", "group").is_some(),
        "{joined:?}\n{}",
        logs(&everyone)
    );
    eventually_within(
        WAIT,
        || {
            text(&later, "a.txt") == "Cid's second a\n"
                && text(&later, "b.txt") == "Ana's b\n"
                && text(&later, "c.txt") == "Cid's c\n"
        },
        "the later computer never got the decided files",
        &everyone,
    );
    assert!(listed(&a2).0.is_empty());
}
