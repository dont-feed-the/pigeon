//! A suggestion (04 §4.5, 07 §7.3 step 7, 07 §7.6): Ana, an admin, creates
//! the group with a backup machine and invites Cid, who is not one. Cid
//! adds a file, which reaches Ana, and edits it, which he may since he alone
//! wrote it (ADR 0070); Cid then edits Ana's file in a folder
//! with the default rule (add-only, the admins as stewards), which he may
//! not change. His edit becomes a proposal, kept as
//! `refs/pigeon/proposals/<device>/<commit>` and relayed to every device;
//! `main` keeps Ana's version everywhere, Cid keeps his file as a waiting
//! `suggestion`, and the backup machine holds its content. A later save
//! stacks a new proposal on the first. Ana sees it in `pigeon suggestions`
//! and accepts it, which Cid may not; the adoption reaches every device,
//! and Cid's suggestion ends without his file being touched. Ana declines
//! his next one, which becomes a conflict on his laptop, his file kept.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{
    Daemon, SERIAL, dirs, eventually_within, git_env, invitation, join, logs, pigeon, said, start,
    until,
};
use pigeon_proto::Response;
use pigeon_proto::api::State;

/// How long a change may take to reach every device.
const WAIT: u64 = 120;

fn text(folder: &Path, name: &str) -> String {
    std::fs::read_to_string(folder.join(name)).unwrap_or_default()
}

/// The repository of `d`'s one group.
fn repo(d: &Daemon) -> PathBuf {
    let c = pigeond::config::load(&d.home.join("config/config.toml")).unwrap();
    c.groups.first().unwrap().path.clone()
}

/// `git` in `dir` as `d`: its standard output.
fn git(d: &Daemon, dir: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    let out = git_env(&mut cmd, &d.home).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The proposal refs `d` keeps.
fn proposals(d: &Daemon) -> Vec<String> {
    let format = "--format=%(refname)";
    git(
        d,
        &repo(d),
        &["for-each-ref", format, "refs/pigeon/proposals/"],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// Whether some annex object of `dir` (a bare repository) holds `content`.
fn holds(dir: &Path, content: &str) -> bool {
    let mut stack = vec![dir.join("annex/objects")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if std::fs::read_to_string(&p).is_ok_and(|t| t == content) {
                return true;
            }
        }
    }
    false
}

#[test]
fn a_change_its_person_may_not_make_becomes_a_relayed_suggestion() {
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
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&a1, &c1, &all);
    until(&a1, State::Protected, &all);
    let code = invitation(&a1);
    let joined = pigeon(
        &b1,
        &["join", &code, "--person", "Cid", fc.to_str().unwrap()],
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

    // An addition is anyone's to make.
    std::fs::write(fc.join("c.txt"), "from cid\n").unwrap();
    eventually_within(
        WAIT,
        || text(&fa, "c.txt") == "from cid\n",
        "Cid's new file never reached Ana",
        &all,
    );
    // So is a change of a file nobody else wrote (ADR 0070).
    std::fs::write(fc.join("c.txt"), "cid's own edit\n").unwrap();
    eventually_within(
        WAIT,
        || text(&fa, "c.txt") == "cid's own edit\n",
        "Cid's edit of his own file never reached Ana",
        &all,
    );
    assert!(
        proposals(&b1).is_empty(),
        "his own file became a suggestion"
    );

    // A change of Ana's file is a suggestion to the admins.
    std::fs::write(fc.join("a.txt"), "cid's edit\n").unwrap();
    let table = fc.join(".git/pigeon/conflicts.json");
    eventually_within(
        WAIT,
        || {
            let t = std::fs::read_to_string(&table).unwrap_or_default();
            t.contains("\"a.txt\"") && t.contains("\"suggestion\"") && t.contains("\"waiting\"")
        },
        "Cid's edit never became a suggestion",
        &all,
    );
    let first = proposals(&b1);
    assert_eq!(first.len(), 1, "{first:?}");
    eventually_within(
        WAIT,
        || proposals(&c1) == first && proposals(&a1) == first,
        "the proposal was not relayed to every device",
        &all,
    );
    eventually_within(
        WAIT,
        || holds(&repo(&c1), "cid's edit\n"),
        "the backup machine never got the suggestion's content",
        &all,
    );
    assert_eq!(
        text(&fa, "a.txt"),
        "first\n",
        "main took a change Cid may not make"
    );
    assert_eq!(
        text(&fc, "a.txt"),
        "cid's edit\n",
        "Cid's file was overwritten"
    );
    // The suggestion waits: capture leaves the unchanged file alone.
    std::thread::sleep(std::time::Duration::from_secs(5));
    assert_eq!(proposals(&b1), first);

    // A later save stacks a new proposal on the first.
    std::fs::write(fc.join("a.txt"), "cid's second edit\n").unwrap();
    eventually_within(
        WAIT,
        || proposals(&b1).len() == 2,
        "the second save never stacked a proposal",
        &all,
    );
    let commit = |r: &str| r.rsplit('/').next().unwrap().to_owned();
    let old = commit(&first[0]);
    let new = proposals(&b1)
        .iter()
        .map(|r| commit(r))
        .find(|c| *c != old)
        .unwrap();
    let parent = git(&b1, &fc, &["rev-parse", &format!("{new}^")]);
    assert_eq!(
        parent.trim(),
        old,
        "the second proposal is not on the first"
    );
    let main = git(&a1, &fa, &["rev-parse", "main:a.txt"]);
    let mains = git(&b1, &fc, &["rev-parse", "main:a.txt"]);
    assert_eq!(main, mains, "Cid's main differs from Ana's at a.txt");

    // Ana decides; Cid may not.
    let listed = |d: &Daemon| match pigeon(d, &["suggestions"]).as_slice() {
        [Response::Proposals { proposals, .. }] => proposals.clone(),
        other => panic!("suggestions: {other:?}"),
    };
    eventually_within(
        WAIT,
        || listed(&a1).iter().any(|p| p.id == new),
        "Ana never saw the stacked suggestion",
        &all,
    );
    let theirs = listed(&a1);
    assert_eq!(theirs.len(), 1, "{theirs:?}");
    assert!(
        theirs[0].decide && !theirs[0].mine && !theirs[0].force,
        "{theirs:?}"
    );
    assert_eq!(theirs[0].paths, ["a.txt"]);
    let own = listed(&b1);
    assert!(own.len() == 1 && own[0].mine && !own[0].decide, "{own:?}");
    // Ana is told that a suggestion waits for her, once; Cid is not, as
    // it is his own (10 §10.4).
    let told = |d: &Daemon| {
        std::fs::read_to_string(d.home.join("notified.log"))
            .unwrap_or_default()
            .matches("notify-waiting")
            .count()
    };
    eventually_within(WAIT, || told(&a1) == 1, "Ana was not told", &all);
    assert_eq!(told(&b1), 0, "Cid was told of his own suggestion");
    // The steward can inspect exact proposed bytes without accepting it.
    let preview = pigeon(
        &a1,
        &[
            "view",
            fa.join("a.txt").to_str().unwrap(),
            "--suggestion",
            &new[..12],
        ],
    );
    let copy = said(&preview, "view-ready", "path").unwrap_or_else(|| panic!("{preview:?}"));
    assert_eq!(std::fs::read(&copy).unwrap(), b"cid's second edit\n");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&copy).unwrap().permissions().mode() & 0o222,
        0
    );
    assert!(!Path::new(&copy).starts_with(&fa));
    assert_eq!(text(&fa, "a.txt"), "first\n");
    let unmodified = pigeon(
        &a1,
        &[
            "view",
            fa.join("c.txt").to_str().unwrap(),
            "--suggestion",
            &new[..12],
        ],
    );
    assert!(
        unmodified
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "not-allowed"))
    );
    let refused = pigeon(&b1, &["accept", &new[..12]]);
    assert!(
        refused
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "not-allowed")),
        "{refused:?}\n{}",
        logs(&all)
    );
    let accepted = pigeon(&a1, &["accept", &new[..12]]);
    assert!(
        said(&accepted, "accepted-suggestion", "author").is_some(),
        "{accepted:?}\n{}",
        logs(&all)
    );

    // The adoption reaches everyone; Cid's suggestion ends, his file untouched.
    eventually_within(
        WAIT,
        || text(&fa, "a.txt") == "cid's second edit\n",
        "Ana's folder never took the accepted suggestion",
        &all,
    );
    eventually_within(
        WAIT,
        || {
            !std::fs::read_to_string(&table)
                .unwrap_or_default()
                .contains("\"a.txt\"")
        },
        "Cid's suggestion never ended",
        &all,
    );
    assert_eq!(text(&fc, "a.txt"), "cid's second edit\n");
    assert!(listed(&a1).is_empty() && listed(&b1).is_empty());
    let status = git(&b1, &fc, &["status", "--porcelain"]);
    assert!(!status.contains("a.txt"), "{status}");

    // Ana declines the next one: Cid keeps his file, now a conflict.
    std::fs::write(fc.join("a.txt"), "cid's third edit\n").unwrap();
    eventually_within(
        WAIT,
        || listed(&a1).len() == 1,
        "Ana never saw the third suggestion",
        &all,
    );
    let third = listed(&a1)[0].id.clone();
    let declined = pigeon(
        &a1,
        &["decline", &third[..12], "--note", "On garde celle-ci."],
    );
    assert!(
        said(&declined, "declined-suggestion", "author").is_some(),
        "{declined:?}\n{}",
        logs(&all)
    );
    eventually_within(
        WAIT,
        || {
            let t = std::fs::read_to_string(&table).unwrap_or_default();
            t.contains("\"a.txt\"") && t.contains("\"conflict\"")
        },
        "Cid never learned that his suggestion was declined",
        &all,
    );
    assert!(listed(&a1).is_empty() && listed(&b1).is_empty());
    assert_eq!(text(&fa, "a.txt"), "cid's second edit\n");
    assert_eq!(text(&fc, "a.txt"), "cid's third edit\n");
    // A fresh laptop must verify the separately retained declined proposal,
    // not reject the group's valid decision history as unverified.
    let a2 = start(&root, s, "a2");
    let code = invitation(&a1);
    let second_folder = root.join("ana-2");
    let joined = pigeon(
        &a2,
        &[
            "join",
            &code,
            "--person",
            "ana",
            "--yes",
            second_folder.to_str().unwrap(),
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    eventually_within(
        WAIT,
        || text(&second_folder, "a.txt") == "cid's second edit\n",
        "fresh join after decline",
        &[&a1, &b1, &c1, &a2],
    );
    assert_eq!(text(&second_folder, "c.txt"), "cid's own edit\n");
    assert!(listed(&a2).is_empty());
}
