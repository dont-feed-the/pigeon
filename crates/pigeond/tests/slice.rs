//! The safety slice on one host (13 §13.12): three `pigeond`, a laptop and
//! two backup machines, each with its own home. Driven through the `pigeon`
//! CLI with `--json`: a file is local after `create`, published once one
//! backup machine joined, protected once the second did; `free` hides it,
//! and `keep` gives back the same bytes. A second test stops a backup
//! machine while a version is saved and freed, and starts it again (S4, S2).
//!
//! Needs git-annex on PATH, and `pigeon` and `git-annex-p2p-iroh` built next
//! to `pigeond` (`cargo build -p pigeon -p git-annex-p2p-iroh`). Skipped
//! without git-annex unless `PIGEON_REQUIRE_GIT_ANNEX` is set, as in CI.
#![cfg(feature = "lab")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
#![allow(clippy::print_stderr, reason = "tests")]

mod common;

use std::time::{Duration, Instant};

use common::{SERIAL, dirs, eventually, join, logs, pigeon, said, start, status, until};
use pigeon_proto::Response;
use pigeon_proto::api::State;

#[test]
fn local_published_protected_then_free_and_keep() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let s = sockets.path();
    let (lap, c1, c2) = (
        start(&root, s, "lap"),
        start(&root, s, "c1"),
        start(&root, s, "c2"),
    );
    let all = [&lap, &c1, &c2];

    let folder = root.join("folder");
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    let big: Vec<u8> = (0u32..300_000)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    std::fs::write(folder.join("sub/big.bin"), &big).unwrap();
    std::fs::write(folder.join("a.txt"), b"hello\n").unwrap();
    let f = folder.to_str().unwrap();
    let created = pigeon(&lap, &["create", "Family", f, "--k", "2", "--name", "lap"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");

    let s = until(&lap, State::Local, &all);
    assert!(s.versions.iter().any(|v| v.path == "sub/big.bin"), "{s:?}");

    join(&lap, &c1, &all);
    until(&lap, State::Published, &all);
    join(&lap, &c2, &all);
    let s = until(&lap, State::Protected, &all);
    assert_eq!(s.headline.id, "headline-safe", "{s:?}");

    let machines = pigeon(&lap, &["machines"]);
    match machines.as_slice() {
        [Response::Machines { machines }] => {
            let mut names: Vec<&str> = machines.iter().map(|m| m.name.as_str()).collect();
            names.sort_unstable();
            assert_eq!(names, ["c1", "c2", "lap"]);
            assert_eq!(machines.iter().filter(|m| m.custodian).count(), 2);
        }
        other => panic!("machines: {other:?}"),
    }

    let sub = folder.join("sub");
    let freed = pigeon(&lap, &["free", sub.to_str().unwrap()]);
    assert_eq!(
        said(&freed, "free-result", "safe").as_deref(),
        Some("1"),
        "{freed:?}"
    );
    // The request records the target; the loops hide the file.
    eventually(
        || !sub.join("big.bin").exists(),
        "big.bin is still here",
        &all,
    );
    // The web's Files page says so, and offers to keep it (10 §10.3).
    let web = common::web::login(&lap);
    let files = common::web::page(&web, "/files");
    assert!(
        files.contains("sub: Safe, 1 file; its space is freed") && files.contains("/files/keep"),
        "{files}"
    );
    // The folder's page lists its file; the file's page its version.
    let listed = common::web::page(&web, &format!("/folder?path={}", sub.display()));
    assert!(listed.contains("sub/big.bin: Safe"), "{listed}");
    let file = common::web::page(
        &web,
        &format!("/file?path={}", sub.join("big.bin").display()),
    );
    assert!(file.contains("Versions") && file.contains("Safe"), "{file}");
    // `pigeon settings` writes config.toml, and the web speaks French at
    // once (11 §11.2).
    let set = pigeon(&lap, &["settings", "language", "fr"]);
    assert!(said(&set, "setting-changed", "value").is_some(), "{set:?}");
    let settings = common::web::page(&web, "/settings");
    assert!(
        settings.contains("Réglages") && settings.contains("value=\"fr\" selected"),
        "{settings}"
    );
    pigeon(&lap, &["settings", "language", "auto"]);
    assert_eq!(status(&lap).state, State::Protected);

    let kept = pigeon(&lap, &["keep", sub.to_str().unwrap()]);
    assert!(
        said(&kept, "keep-result", "folder").is_some(),
        "{kept:?}\n{}",
        logs(&all)
    );
    eventually(
        || sub.join("big.bin").exists(),
        "big.bin never came back",
        &all,
    );
    assert_eq!(std::fs::read(sub.join("big.bin")).unwrap(), big);

    let history = pigeon(&lap, &["history", folder.join("a.txt").to_str().unwrap()]);
    match history.as_slice() {
        [Response::History { versions }] => {
            assert!(
                versions.iter().any(|v| v.state == State::Protected),
                "{versions:?}"
            );
        }
        other => panic!("history: {other:?}"),
    }
    // A deleted file is listed under Deleted files, and restore brings
    // its last version back (ux.md §6.2).
    std::fs::remove_file(folder.join("a.txt")).unwrap();
    eventually(
        || common::web::page(&web, "/deleted").contains("a.txt, deleted"),
        "a.txt is not under Deleted files",
        &all,
    );
    let back = pigeon(&lap, &["restore", folder.join("a.txt").to_str().unwrap()]);
    assert!(
        std::fs::read(folder.join("a.txt")).is_ok_and(|b| b == b"hello\n"),
        "{back:?}"
    );
    let paused = pigeon(&lap, &["pause", "2h"]);
    assert!(
        said(&paused, "paused-until", "until").is_some(),
        "{paused:?}"
    );
    match pigeon(&lap, &["status"]).as_slice() {
        [Response::Status(r)] => assert_eq!(r.headline.id, "headline-paused", "{r:?}"),
        other => panic!("status while paused: {other:?}"),
    }
    let resumed = pigeon(&lap, &["resume"]);
    assert!(said(&resumed, "resumed", "-").is_some(), "{resumed:?}");
    match pigeon(&lap, &["erase", "a.txt"]).as_slice() {
        [Response::Problem(p)] => assert_eq!(p.code, "not-in-this-build"),
        other => panic!("erase: {other:?}"),
    }
}

/// S4 and S2 on one host: a backup machine stopped while a version is saved
/// gets it once it starts again, and a folder freed meanwhile is freed then.
#[test]
fn a_backup_machine_back_from_a_stop_gets_the_new_version() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let s = sockets.path();
    let (lap, c1, c2) = (
        start(&root, s, "lap"),
        start(&root, s, "c1"),
        start(&root, s, "c2"),
    );
    let folder = root.join("folder");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), b"first\n").unwrap();
    let f = folder.to_str().unwrap();
    let created = pigeon(&lap, &["create", "Family", f, "--k", "2", "--name", "lap"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&lap, &c1, &[&lap, &c1, &c2]);
    join(&lap, &c2, &[&lap, &c1, &c2]);
    until(&lap, State::Protected, &[&lap, &c1, &c2]);

    // S2: `free` leaves a version that is not protected in place, and the
    // loops free it once it is.
    drop(c2);
    let later = folder.join("later");
    std::fs::create_dir_all(&later).unwrap();
    std::fs::write(later.join("b.txt"), b"second\n").unwrap();
    let s1 = until(&lap, State::Published, &[&lap, &c1]);
    assert!(
        s1.versions.iter().any(|v| v.path == "later/b.txt"),
        "{s1:?}"
    );
    let freed = pigeon(&lap, &["free", later.to_str().unwrap()]);
    assert_eq!(
        said(&freed, "free-result", "safe").as_deref(),
        Some("0"),
        "{freed:?}"
    );
    assert!(
        later.join("b.txt").exists(),
        "freed before it was protected"
    );
    let c2 = start(&root, s, "c2");
    let all = [&lap, &c1, &c2];
    until(&lap, State::Protected, &all);
    let deadline = Instant::now() + Duration::from_secs(60);
    while later.join("b.txt").exists() {
        assert!(Instant::now() < deadline, "never freed\n{}", logs(&all));
        std::thread::sleep(Duration::from_millis(250));
    }
    assert_eq!(status(&lap).state, State::Protected);
}
