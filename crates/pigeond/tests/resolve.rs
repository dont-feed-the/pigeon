//! Resolving divergences (07 §7.6, ux.md §8.8): Ana, an admin, creates the
//! group with a backup machine and invites Cid, who is not one. Cid's edit
//! of Ana's file is a waiting suggestion, which `pigeon conflicts` lists;
//! *Take the group's version* gives him Ana's file back and withdraws his
//! proposal, so Ana has nothing to decide. His next edit, declined, is a
//! conflict; *Keep mine as a copy* puts his version at a new name, which
//! reaches Ana, and the file is the group's again. The next, declined
//! too, he suggests again, and Ana sees it waiting. A clean file is not
//! diverging.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use std::path::Path;

use common::{Daemon, SERIAL, dirs, eventually_within, join, logs, pigeon, said, start, until};
use pigeon_proto::Response;
use pigeon_proto::api::{Divergence, State, Suggestion};

/// How long a change may take to reach every device.
const WAIT: u64 = 120;

fn text(folder: &Path, name: &str) -> String {
    std::fs::read_to_string(folder.join(name)).unwrap_or_default()
}

fn suggestions(d: &Daemon) -> Vec<Suggestion> {
    match pigeon(d, &["suggestions"]).as_slice() {
        [Response::Proposals { proposals, .. }] => proposals.clone(),
        other => panic!("suggestions: {other:?}"),
    }
}

fn conflicts(d: &Daemon) -> Vec<Divergence> {
    match pigeon(d, &["conflicts"]).as_slice() {
        [Response::Conflicts { conflicts }] => conflicts.clone(),
        other => panic!("conflicts: {other:?}"),
    }
}

fn state_of(d: &Daemon, path: &str) -> Option<String> {
    conflicts(d)
        .into_iter()
        .find(|c| c.path == path)
        .map(|c| c.state)
}

fn problem(rs: &[Response]) -> Option<String> {
    rs.iter().find_map(|r| match r {
        Response::Problem(p) => Some(p.code.clone()),
        _ => None,
    })
}

/// Cid saves `body` at a.txt; Ana declines the suggestion; Cid is told.
fn declined(ana: &Daemon, cid: &Daemon, fc: &Path, body: &str, all: &[&Daemon]) {
    std::fs::write(fc.join("a.txt"), body).unwrap();
    eventually_within(
        WAIT,
        || suggestions(ana).len() == 1,
        "Ana never saw it",
        all,
    );
    let id = suggestions(ana)[0].id.clone();
    let no = pigeon(ana, &["decline", &id[..12]]);
    assert!(
        said(&no, "declined-suggestion", "author").is_some(),
        "{no:?}"
    );
    eventually_within(
        WAIT,
        || state_of(cid, "a.txt").as_deref() == Some("conflict"),
        "Cid never learned of the decline",
        all,
    );
}

#[test]
fn a_divergence_is_resolved_three_ways() {
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
    std::fs::write(fa.join("a.txt"), "first\n").unwrap();
    std::fs::write(fa.join("b.txt"), "bee\n").unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&a1, &c1, &all);
    until(&a1, State::Protected, &all);
    let answer = pigeon(&a1, &["invite", "Cid"]);
    let code = said(&answer, "invitation", "code").unwrap_or_else(|| panic!("{answer:?}"));
    let joined = pigeon(
        &b1,
        &["join", &code, fc.to_str().unwrap(), "--name", "cid-1"],
    );
    assert!(
        said(&joined, "joined", "group").is_some(),
        "{joined:?}\n{}",
        logs(&all)
    );
    eventually_within(
        WAIT,
        || text(&fc, "a.txt") == "first\n",
        "Cid never got a.txt",
        &all,
    );
    let a = fc.join("a.txt");
    let a = a.to_str().unwrap();

    // A clean file is not diverging.
    let clean = pigeon(
        &b1,
        &[
            "resolve",
            fc.join("b.txt").to_str().unwrap(),
            "--take-group",
        ],
    );
    assert_eq!(
        problem(&clean).as_deref(),
        Some("not-diverging"),
        "{clean:?}"
    );

    // Waiting, then taken back: his proposal is withdrawn.
    std::fs::write(fc.join("a.txt"), "cid's edit\n").unwrap();
    eventually_within(
        WAIT,
        || state_of(&b1, "a.txt").as_deref() == Some("waiting"),
        "Cid's edit never became a waiting suggestion",
        &all,
    );
    eventually_within(
        WAIT,
        || suggestions(&a1).len() == 1,
        "Ana never saw it",
        &all,
    );
    let took = pigeon(&b1, &["resolve", a, "--take-group"]);
    assert!(
        said(&took, "resolved-take-group", "path").is_some(),
        "{took:?}\n{}",
        logs(&all)
    );
    assert_eq!(text(&fc, "a.txt"), "first\n");
    assert!(conflicts(&b1).is_empty());
    eventually_within(
        WAIT,
        || suggestions(&a1).is_empty(),
        "the withdrawal never reached Ana",
        &all,
    );
    std::thread::sleep(std::time::Duration::from_secs(3));
    assert_eq!(text(&fc, "a.txt"), "first\n", "capture took the file again");
    assert!(suggestions(&b1).is_empty());

    // Declined, then kept as a copy, which reaches Ana.
    declined(&a1, &b1, &fc, "cid's second edit\n", &all);
    let copy = fc.join("a (Cid's).txt");
    let kept = pigeon(&b1, &["resolve", a, "--keep-copy", copy.to_str().unwrap()]);
    assert!(
        said(&kept, "resolved-keep-copy", "dest").is_some(),
        "{kept:?}\n{}",
        logs(&all)
    );
    assert_eq!(text(&fc, "a.txt"), "first\n");
    eventually_within(
        WAIT,
        || text(&fa, "a (Cid's).txt") == "cid's second edit\n",
        "Cid's copy never reached Ana",
        &all,
    );
    assert!(conflicts(&b1).is_empty());

    // Declined, then suggested again.
    declined(&a1, &b1, &fc, "cid's third edit\n", &all);
    let again = pigeon(&b1, &["resolve", a, "--suggest-again"]);
    assert!(
        said(&again, "resolved-suggest-again", "path").is_some(),
        "{again:?}\n{}",
        logs(&all)
    );
    assert_eq!(state_of(&b1, "a.txt").as_deref(), Some("waiting"));
    eventually_within(
        WAIT,
        || suggestions(&a1).len() == 1,
        "Ana never saw the suggestion again",
        &all,
    );
    assert_eq!(text(&fa, "a.txt"), "first\n");
    assert_eq!(text(&fc, "a.txt"), "cid's third edit\n");
}
