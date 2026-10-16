//! Status tells the truth (06 §6.4, §6.7, 09 §9.7): a version's age counts
//! from when this computer first saw it, so another capture does not make
//! an unsafe version young again; and a computer removed from the group
//! says so, everywhere, instead of "Everything is safe".
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::{
    Daemon, SERIAL, dirs, eventually_within, join, logs, pigeon, said, start, status, until, web,
};
use pigeon_proto::Response;
use pigeon_proto::api::{Level, State, VersionStatus};

/// `path`'s line in `d`'s status, once captured.
fn captured(d: &Daemon, path: &str, all: &[&Daemon]) -> VersionStatus {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let s = status(d);
        if let Some(v) = s
            .versions
            .iter()
            .find(|v| v.path == path && v.reason.as_ref().is_some_and(|r| r.id != "why-capturing"))
        {
            return v.clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{path} never captured: {s:?}\n{}",
            logs(all)
        );
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

#[test]
fn another_capture_does_not_make_an_unsafe_version_young_again() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let vault = start(&root, sockets.path(), "vault");
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    let f = folder.to_str().unwrap();
    let created = pigeon(
        &lap,
        &["create", "Family", f, "--k", "1", "--name", "laptop"],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&lap, &vault, &[&lap, &vault]);
    std::fs::write(folder.join("a.txt"), b"kept safe\n").unwrap();
    until(&lap, State::Protected, &[&lap, &vault]);
    // The only backup machine stops: new versions stay not safe.
    drop(vault);
    std::fs::write(folder.join("x.txt"), b"waits for the backup\n").unwrap();
    let x = captured(&lap, "x.txt", &[&lap]);
    assert!(matches!(x.state, State::Local | State::Published), "{x:?}");
    std::thread::sleep(std::time::Duration::from_secs(2));
    std::fs::write(folder.join("y.txt"), b"a later capture\n").unwrap();
    let y = captured(&lap, "y.txt", &[&lap]);
    let again = status(&lap)
        .versions
        .into_iter()
        .find(|v| v.path == "x.txt")
        .unwrap();
    // The later commit on main leaves x's age alone; y is younger.
    assert_eq!(
        again.saved_at,
        x.saved_at,
        "{again:?} {x:?}\n{}",
        logs(&[&lap])
    );
    assert!(y.saved_at > x.saved_at, "{y:?} {x:?}");
}

#[test]
fn a_removed_computer_says_it_was_removed() {
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
    join(&a1, &c1, &all);
    until(&a1, State::Protected, &all);
    let answer = pigeon(&a1, &["invite", "ana"]);
    let code = said(&answer, "invitation", "code").unwrap_or_else(|| panic!("{answer:?}"));
    let folder = fa2.to_str().unwrap();
    let joined = pigeon(&a2, &["join", &code, folder, "--name", "ana-2"]);
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
    eventually_within(
        120,
        || status(&a2).level == Level::Safe,
        "the second laptop never said its files were safe",
        &all,
    );

    let removed = pigeon(&a1, &["remove-computer", "ana-2", "--yes"]);
    assert!(
        said(&removed, "removed-computer", "name").is_some(),
        "{removed:?}"
    );
    eventually_within(
        180,
        || status(&a2).headline.id == "headline-removed",
        "the removed laptop never learned it was removed",
        &all,
    );
    let report = status(&a2);
    assert_eq!(report.level, Level::NeedsYou, "{report:?}");
    assert!(report.versions.is_empty(), "{report:?}");
    // Nothing counts as safe on it any more, and a file says why.
    let file = fa2.join("a.txt");
    let asked = pigeon(&a2, &["status", file.to_str().unwrap()]);
    let Some(Response::Status(one)) = asked.first() else {
        panic!("{asked:?}");
    };
    assert!(
        one.versions.iter().any(|v| v.path == "a.txt"
            && v.state != State::Protected
            && v.reason.as_ref().is_some_and(|r| r.id == "why-removed")),
        "{one:?}"
    );
    let machines = pigeon(&a2, &["machines"]);
    assert!(
        machines.iter().any(|r| matches!(r,
            Response::Progress { message, .. } if message.id == "removed-from-group")),
        "{machines:?}"
    );
    // The web says the same on Home and on People & machines.
    let session = web::login(&a2);
    let home = web::page(&session, "/");
    assert!(
        home.contains("This computer was removed from the group"),
        "{home}"
    );
    assert!(!home.contains("Everything is safe"), "{home}");
    let people = web::page(&session, "/machines");
    assert!(
        people.contains("This computer is no longer in the group"),
        "{people}"
    );
    // The group itself carries on.
    assert_ne!(status(&a1).headline.id, "headline-removed");
}
