//! Held mass deletions (07 §7.3 step 3, §7.4): a laptop deletes 101 files
//! at once; the deletion is held, `main` keeps them and the headline asks
//! the person. *Bring them back* writes them again; deleted again and
//! *shared*, they leave `main`. Removing a top-level folder is held too.
//! A new file with a non-portable name, or respelling a name of `main`, is
//! held back with a suggested name (05 §5.4); *Rename* or the person's own
//! rename lets capture add it.
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

fn delete_files(folder: &Path, all: &[&common::Daemon]) {
    for i in 0..101 {
        let path = folder.join(name(i));
        // Protected copies can be re-thinned while the person deletes.
        // The original name is briefly absent between aside and rename-in.
        // Require a successful unlink: ignoring NotFound would leave a file
        // undeleted when thinning puts its replacement back.
        eventually_within(
            5,
            || match std::fs::remove_file(&path) {
                Ok(()) => true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => panic!("deleting {}: {e}\n{}", path.display(), common::logs(all)),
            },
            &format!("{} stayed missing during deletion", path.display()),
            all,
        );
    }
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

    delete_files(&fa, &all);
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

    delete_files(&fa, &all);
    eventually_within(
        60,
        || held(&a1).len() == 1,
        "the second deletion was never held",
        &all,
    );
    let h = held(&a1).remove(0);
    assert_ne!(h.id, "1", "an ID is never given twice");
    let shared = pigeon(&a1, &["held", &h.id, "--share", "--yes"]);
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
    let unknown = pigeon(&a1, &["held", "99", "--share", "--yes"]);
    assert!(
        unknown
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "held-unknown")),
        "{unknown:?}"
    );
}

fn problem(r: &[Response], code: &str) -> bool {
    r.iter()
        .any(|r| matches!(r, Response::Problem(p) if p.code == code))
}

#[test]
fn a_non_portable_name_waits_for_a_rename() {
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
    std::fs::create_dir_all(&fa).unwrap();
    std::fs::write(fa.join("ok.txt"), "ok\n").unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&a1, &c1, &all);
    until(&a1, State::Protected, &all);

    // Default APFS aliases these paths: writing Ok.TXT there edits ok.txt
    // rather than creating the second, colliding entry this fixture needs.
    // Portable case collisions are also checked by the core name laws.
    let distinct_case_name = !fa.join("Ok.TXT").exists();
    std::fs::write(fa.join("a:b.txt"), "colon\n").unwrap();
    if distinct_case_name {
        std::fs::write(fa.join("Ok.TXT"), "respelt\n").unwrap();
    }
    let expected_held = if distinct_case_name { 2 } else { 1 };
    eventually_within(
        60,
        || held(&a1).len() == expected_held,
        "the names were never held back",
        &all,
    );
    let names = held(&a1);
    let suggested: Vec<Option<&str>> = names.iter().map(|h| h.suggested.as_deref()).collect();
    assert!(suggested.contains(&Some("a-b.txt")), "{names:?}");
    if distinct_case_name {
        assert!(suggested.contains(&Some("Ok 2.TXT")), "{names:?}");
    }
    assert!(
        names.iter().all(|h| h.reason.id == "held-name"),
        "{names:?}"
    );
    assert_eq!(in_main(&fa), ["ok.txt"]);
    let st = status(&a1);
    assert_eq!(st.level, Level::NeedsYou);
    assert_eq!(st.headline.id, "headline-held-names");

    let colon = names.iter().find(|h| h.paths == ["a:b.txt"]).unwrap();
    let refused = pigeon(&a1, &["held", &colon.id, "--share", "--yes"]);
    assert!(problem(&refused, "held-choice"), "{refused:?}");
    let renamed = pigeon(&a1, &["held", &colon.id, "--rename"]);
    assert!(
        said(&renamed, "held-renamed", "to").is_some(),
        "{renamed:?}"
    );
    if distinct_case_name {
        std::fs::rename(fa.join("Ok.TXT"), fa.join("other.txt")).unwrap();
    } else {
        std::fs::write(fa.join("other.txt"), "respelt\n").unwrap();
    }
    eventually_within(
        60,
        || in_main(&fa) == ["a-b.txt", "ok.txt", "other.txt"],
        "the renamed files never reached main",
        &all,
    );
    eventually_within(60, || held(&a1).is_empty(), "the names stayed held", &all);
    assert_eq!(std::fs::read(fa.join("ok.txt")).unwrap(), b"ok\n");
    assert_eq!(std::fs::read(fa.join("other.txt")).unwrap(), b"respelt\n");
    assert_eq!(
        std::fs::read_to_string(fa.join("a-b.txt")).unwrap(),
        "colon\n"
    );
}
