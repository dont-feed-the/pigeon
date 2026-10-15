//! Two people and three laptops on one host (09 §9.6, 07 §7.5–7.6): Ana
//! creates the group with two backup machines, invites Bea, a new person,
//! and her own second laptop; each laptop gets the others' files. Bea then
//! edits a file while her `pigeond` is stopped, Ana edits it too, and once
//! Bea's daemon is back both versions stay: the catalogue merge gives one
//! the name and the other its conflict name, the laptop whose version lost
//! the name keeps its file as a `concurrent` divergence, and nothing is
//! overwritten.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::{Daemon, SERIAL, dirs, eventually_within, join, logs, pigeon, said, start, until};
use pigeon_proto::api::State;

/// Invites `joiner` from `inviter` with `invite` (the CLI's words) and
/// joins it in `folder`.
fn join_laptop(inviter: &Daemon, invite: &[&str], joiner: &Daemon, folder: &Path, all: &[&Daemon]) {
    let answer = pigeon(inviter, invite);
    let code = said(&answer, "invitation", "code").unwrap_or_else(|| panic!("{answer:?}"));
    let joined = pigeon(joiner, &["join", &code, folder.to_str().unwrap()]);
    assert!(
        said(&joined, "joined", "group").is_some(),
        "{joined:?}\n{}",
        logs(all)
    );
}

/// The files of `folder`'s root named `a….txt`: `a.txt` and its conflict
/// copies (05 §5.3), with their text.
fn copies(folder: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ours = name.starts_with('a') && name.ends_with(".txt");
            let text = std::fs::read_to_string(e.path()).ok()?;
            ours.then_some((name, text))
        })
        .collect()
}

/// How long a change may take to reach every laptop: five daemons on one
/// busy host, each fetching from four peers.
const WAIT: u64 = 180;

fn text(folder: &Path, name: &str) -> String {
    std::fs::read_to_string(folder.join(name)).unwrap_or_default()
}

#[test]
fn two_people_and_three_laptops_keep_both_concurrent_versions() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let s = sockets.path();
    let (a1, c1, c2, a2, b1) = (
        start(&root, s, "a1"),
        start(&root, s, "c1"),
        start(&root, s, "c2"),
        start(&root, s, "a2"),
        start(&root, s, "b1"),
    );
    let (fa, fa2, fb) = (root.join("ana"), root.join("ana-2"), root.join("bea"));
    std::fs::create_dir_all(&fa).unwrap();
    std::fs::write(fa.join("a.txt"), "first\n").unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "2", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    {
        let all = [&a1, &c1, &c2, &a2, &b1];
        join(&a1, &c1, &all);
        join(&a1, &c2, &all);
        until(&a1, State::Protected, &all);

        // A new person, an admin, and a second laptop of Ana's.
        join_laptop(&a1, &["invite", "--admin", "Bea"], &b1, &fb, &all);
        join_laptop(&a1, &["invite", "ana"], &a2, &fa2, &all);
        eventually_within(
            WAIT,
            || text(&fb, "a.txt") == "first\n" && text(&fa2, "a.txt") == "first\n",
            "the new laptops never got a.txt",
            &all,
        );
        std::fs::write(fb.join("b.txt"), "from bea\n").unwrap();
        eventually_within(
            WAIT,
            || text(&fa, "b.txt") == "from bea\n" && text(&fa2, "b.txt") == "from bea\n",
            "Bea's file never reached Ana's laptops",
            &all,
        );
    }

    // Bea edits a.txt while her pigeond is stopped; Ana edits it meanwhile.
    drop(b1);
    std::fs::write(fa.join("a.txt"), "ana's edit\n").unwrap();
    eventually_within(
        WAIT,
        || text(&fa2, "a.txt") == "ana's edit\n",
        "Ana's edit never reached her second laptop",
        &[&a1, &c1, &c2, &a2],
    );
    std::fs::write(fb.join("a.txt"), "bea's edit\n").unwrap();
    let b1 = start(&root, s, "b1");
    let all = [&a1, &c1, &c2, &a2, &b1];
    let both = |m: &BTreeMap<String, String>| {
        let mut texts: Vec<&str> = m.values().map(String::as_str).collect();
        texts.sort_unstable();
        texts == ["ana's edit\n", "bea's edit\n"]
    };
    // The laptop that saved neither shows both: one under a.txt, the other
    // under its conflict name.
    eventually_within(
        WAIT,
        || copies(&fa2).len() == 2 && both(&copies(&fa2)),
        "Ana's second laptop never showed both versions",
        &all,
    );
    let winner = text(&fa2, "a.txt");
    let (lost, lost_text) = if winner == "ana's edit\n" {
        (&fb, "bea's edit\n")
    } else {
        (&fa, "ana's edit\n")
    };
    // The laptop whose version lost the name keeps its file, and says so in
    // conflicts.json; the other shows both.
    let won = if lost == &fb { &fa } else { &fb };
    eventually_within(
        WAIT,
        || copies(won).len() == 2 && both(&copies(won)),
        "the winner's laptop never showed both versions",
        &all,
    );
    let table = lost.join(".git/pigeon/conflicts.json");
    eventually_within(
        WAIT,
        || {
            let c = copies(lost);
            c.len() == 2
                && c.values().all(|t| t == lost_text)
                && std::fs::read_to_string(&table).is_ok_and(|t| t.contains("\"a.txt\""))
        },
        "the losing laptop did not keep its file with its conflict copy",
        &all,
    );
    assert_eq!(text(&fa, "b.txt"), "from bea\n");
}
