//! A slow content stream must leave capture and history available
//! (ADR 0037, 15 §15.2–15.3). Real peers and the production CLI configure
//! the shared limiter; Git's accepted main proves the later save is captured.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, reason = "real bounded resource-progress fixture")]

mod common;

use common::{SERIAL, dirs, eventually_within, join, pigeon, said, start, until};
use pigeon_proto::api::State;

fn captured(folder: &std::path::Path, name: &str) -> bool {
    let output = std::process::Command::new("git")
        .current_dir(folder)
        .args(["ls-tree", "--name-only", "main", "--", name])
        .output()
        .unwrap();
    output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == name
}

#[test]
fn a_throttled_transfer_does_not_hold_later_saves_or_metadata() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_temp, root, sockets)) = dirs() else {
        return;
    };
    let alice = start(&root, sockets.path(), "alice");
    let vault = start(&root, sockets.path(), "vault");
    let all = [&alice, &vault];
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("first.txt"), b"already protected\n").unwrap();
    let created = pigeon(
        &alice,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&alice, &vault, &all);
    until(&alice, State::Protected, &all);
    let configured = pigeon(&alice, &["settings", "resources.upload", "1 B/s"]);
    assert!(
        configured
            .iter()
            .all(|r| !matches!(r, pigeon_proto::Response::Problem(_))),
        "{configured:?}"
    );
    // The production controller reloads config every two seconds. This
    // explicit boundary ensures the initial burst cannot finish the file.
    std::thread::sleep(std::time::Duration::from_millis(2_100));
    std::fs::write(folder.join("large.bin"), vec![0xa5; 262_144]).unwrap();
    eventually_within(
        15,
        || captured(&folder, "large.bin"),
        "large file was not captured",
        &all,
    );
    // At one byte/second, this content cannot finish within the observation.
    // Allow the first round to reach its transfer before the next save.
    std::thread::sleep(std::time::Duration::from_secs(2));
    std::fs::write(folder.join("later.txt"), b"capture keeps progressing\n").unwrap();
    eventually_within(
        15,
        || captured(&folder, "later.txt"),
        "a content wait blocked capture of a later save",
        &all,
    );
    let history = pigeon(
        &alice,
        &["history", folder.join("later.txt").to_str().unwrap()],
    );
    assert!(
        history.iter().any(
            |r| matches!(r, pigeon_proto::Response::History { versions } if versions.len() == 1)
        ),
        "{history:?}"
    );
    assert_eq!(
        std::fs::read(folder.join("later.txt")).unwrap(),
        b"capture keeps progressing\n"
    );
    let paused = pigeon(&alice, &["pause"]);
    assert!(
        paused
            .iter()
            .all(|r| !matches!(r, pigeon_proto::Response::Problem(_))),
        "{paused:?}"
    );
    std::fs::write(
        folder.join("paused-save.txt"),
        b"capture ignores a person's pause\n",
    )
    .unwrap();
    eventually_within(
        15,
        || captured(&folder, "paused-save.txt"),
        "a person's pause blocked capture",
        &all,
    );
    let resumed = pigeon(&alice, &["resume"]);
    assert!(
        resumed
            .iter()
            .all(|r| !matches!(r, pigeon_proto::Response::Problem(_))),
        "{resumed:?}"
    );
}
