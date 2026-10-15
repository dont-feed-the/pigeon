//! Held mass deletions (07 §7.3 step 3, §7.4): a laptop deletes 101 files
//! at once; the deletion is held, `main` keeps them and the headline asks
//! the person. *Bring them back* writes them again; deleted again and
//! *shared*, they leave `main`. Removing a top-level folder is held too.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use std::path::Path;
use std::process::Command;

use common::{SERIAL, dirs, eventually_within, join, pigeon, said, start, status, until};
use pigeon_proto::Response;
use pigeon_proto::api::{HeldEntry, Level, State};

fn held(d: &common::Daemon) -> Vec<HeldEntry> {
    pigeon(d, &["held"])
        .into_iter()
        .flat_map(|r| match r {
            Response::Held { held } => held,
            _ => Vec::new(),
        })
        .collect()
}

/// The paths of `main` in the folder's repository.
fn in_main(folder: &Path) -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-tree", "-r", "--name-only", "main"])
        .current_dir(folder)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.starts_with(".pigeon") && !l.starts_with(".gitattributes"))
        .map(str::to_owned)
        .collect()
}

fn name(i: usize) -> String {
    format!("f{i:03}.txt")
}

#[test]
fn a_mass_deletion_waits_for_the_person() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let s = sockets.path();
    let (a1, c1) = (start(&root, s, "a1"), start(&root, s, "c1"));
    let all = [&a1, &c1];
    let fa = root.join("ana");
    std::fs::create_dir_all(fa.join("docs")).unwrap();
    for i in 0..101 {
        std::fs::write(fa.join(name(i)), format!("file {i}\n")).unwrap();
    }
    std::fs::write(fa.join("docs/keep.txt"), "kept\n").unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&a1, &c1, &all);
    until(&a1, State::Protected, &all);
    assert_eq!(in_main(&fa).len(), 102);

    for i in 0..101 {
        std::fs::remove_file(fa.join(name(i))).unwrap();
    }
    eventually_within(
        60,
        || held(&a1).len() == 1,
        "the deletion was never held",
        &all,
    );
    let h = held(&a1).remove(0);
    assert_eq!(h.files, 101);
    assert_eq!(h.reason.id, "held-mass-deletion");
    assert_eq!(in_main(&fa).len(), 102, "main keeps the held files");
    let st = status(&a1);
    assert_eq!(st.level, Level::NeedsYou);
    assert_eq!(st.headline.id, "headline-held");

    let back = pigeon(&a1, &["held", &h.id, "--bring-back"]);
    assert!(
        said(&back, "held-brought-back", "files").is_some(),
        "{back:?}\n{}",
        common::logs(&all)
    );
    eventually_within(
        120,
        || {
            (0..101).all(|i| {
                std::fs::read_to_string(fa.join(name(i))).is_ok_and(|t| t == format!("file {i}\n"))
            })
        },
        "the held files never came back",
        &all,
    );
    assert!(held(&a1).is_empty());

    for i in 0..101 {
        std::fs::remove_file(fa.join(name(i))).unwrap();
    }
    eventually_within(
        60,
        || held(&a1).len() == 1,
        "the second deletion was never held",
        &all,
    );
    let h = held(&a1).remove(0);
    assert_ne!(h.id, "1", "an ID is never given twice");
    let shared = pigeon(&a1, &["held", &h.id, "--share"]);
    assert!(
        said(&shared, "held-shared", "files").is_some(),
        "{shared:?}"
    );
    eventually_within(
        60,
        || in_main(&fa) == ["docs/keep.txt"],
        "the shared deletion never reached main",
        &all,
    );
    assert!(held(&a1).is_empty());

    std::fs::remove_dir_all(fa.join("docs")).unwrap();
    eventually_within(
        60,
        || held(&a1).len() == 1,
        "the removed folder was never held",
        &all,
    );
    assert_eq!(held(&a1)[0].reason.id, "held-top-folder");
    assert_eq!(in_main(&fa), ["docs/keep.txt"]);
    let unknown = pigeon(&a1, &["held", "99", "--share"]);
    assert!(
        unknown
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "held-unknown")),
        "{unknown:?}"
    );
}
