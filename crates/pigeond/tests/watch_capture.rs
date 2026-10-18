//! Native background change detection (ADR 0013): a saved file is captured
//! without polling the status API or waiting for the ten-minute fallback.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, reason = "isolated real-daemon watcher fixture")]

mod common;

use std::process::Command;
use std::time::{Duration, Instant};

use common::{dirs, pigeon, said, start};
use sha2::{Digest, Sha256};

fn empty_scan_completed(home: &std::path::Path) -> bool {
    let logs = home.join("state/logs");
    std::fs::read_dir(logs)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            std::fs::read_to_string(entry.path())
                .unwrap_or_default()
                .lines()
                .any(|line| {
                    serde_json::from_str::<serde_json::Value>(line).is_ok_and(|row| {
                        row["message"] == "candidate-scan-complete" && row["pending"] == 0
                    })
                })
        })
}

#[test]
fn a_save_and_deletion_wake_background_capture_without_status_polling() {
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let home = root.join("watched/config");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("config.toml"),
        "[network]\nbind=\"127.0.0.1:0\"\nmdns=false\nrelays=\"off\"\n[loops]\ninterval_seconds=600\n").unwrap();
    let daemon = start(&root, sockets.path(), "watched");
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    let created = pigeon(
        &daemon,
        &["create", "Watched", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some());
    let deadline = Instant::now() + Duration::from_secs(30);
    while !empty_scan_completed(&daemon.home) {
        assert!(
            Instant::now() < deadline,
            "initial empty scan did not finish"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    // Let notifications from setup drain before the save under observation.
    std::thread::sleep(Duration::from_secs(1));
    let bytes = b"a real background save\n";
    let expected = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    std::fs::write(folder.join("watched.txt"), bytes).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let pointer = loop {
        let result = common::git_env(
            Command::new("git")
                .current_dir(&folder)
                .args(["show", "main:watched.txt"]),
            &daemon.home,
        )
        .output()
        .unwrap();
        if result.status.success() && String::from_utf8_lossy(&result.stdout).contains(&expected) {
            break String::from_utf8(result.stdout).unwrap();
        }
        assert!(
            Instant::now() < deadline,
            "background capture did not observe the save without status polling\n{}",
            common::logs(&[&daemon])
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    let key = pointer.trim().rsplit('/').next().unwrap();
    let location = common::git_env(
        Command::new("git")
            .current_dir(&folder)
            .args(["annex", "contentlocation", key]),
        &daemon.home,
    )
    .output()
    .unwrap();
    assert!(location.status.success());
    let path = folder.join(String::from_utf8(location.stdout).unwrap().trim());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    // Drain the save's notifications before testing a new deletion hint.
    std::thread::sleep(Duration::from_secs(1));
    std::fs::remove_file(folder.join("watched.txt")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let result = common::git_env(
            Command::new("git")
                .current_dir(&folder)
                .args(["ls-tree", "main", "--", "watched.txt"]),
            &daemon.home,
        )
        .output()
        .unwrap();
        if result.status.success() && result.stdout.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background capture did not observe the deletion without status polling\n{}",
            common::logs(&[&daemon])
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn native_registrations(home: &std::path::Path) -> usize {
    std::fs::read_dir(home.join("state/logs"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| {
            std::fs::read_to_string(entry.path())
                .unwrap_or_default()
                .lines()
                .filter(|line| {
                    serde_json::from_str::<serde_json::Value>(line).is_ok_and(|row| {
                        row["message"] == "capture-watch-registered" && row["native"] == true
                    })
                })
                .count()
        })
        .sum()
}

#[test]
fn a_folder_created_after_startup_observes_later_file_saves() {
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let config = root.join("newdir/config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"),
        "[network]\nbind=\"127.0.0.1:0\"\nmdns=false\nrelays=\"off\"\n[loops]\ninterval_seconds=600\n").unwrap();
    let daemon = start(&root, sockets.path(), "newdir");
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    let created = pigeon(
        &daemon,
        &["create", "New folder", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some());
    let deadline = Instant::now() + Duration::from_secs(30);
    while native_registrations(&daemon.home) == 0 || !empty_scan_completed(&daemon.home) {
        assert!(
            Instant::now() < deadline,
            "initial native registration did not finish"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let before = native_registrations(&daemon.home);
    let child = folder.join("new-folder");
    std::fs::create_dir(&child).unwrap();
    #[cfg(target_os = "linux")]
    {
        let deadline = Instant::now() + Duration::from_secs(60);
        while native_registrations(&daemon.home) <= before {
            assert!(
                Instant::now() < deadline,
                "new directory registration did not refresh"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = before;
    let bytes = b"saved after directory registration\n";
    let expected = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    std::fs::write(child.join("later.txt"), bytes).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let pointer = loop {
        let result = common::git_env(
            Command::new("git")
                .current_dir(&folder)
                .args(["show", "main:new-folder/later.txt"]),
            &daemon.home,
        )
        .output()
        .unwrap();
        if result.status.success() && String::from_utf8_lossy(&result.stdout).contains(&expected) {
            break String::from_utf8(result.stdout).unwrap();
        }
        assert!(
            Instant::now() < deadline,
            "new directory's save was not captured"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    let key = pointer.trim().rsplit('/').next().unwrap();
    let location = common::git_env(
        Command::new("git")
            .current_dir(&folder)
            .args(["annex", "contentlocation", key]),
        &daemon.home,
    )
    .output()
    .unwrap();
    assert!(location.status.success());
    assert_eq!(
        std::fs::read(folder.join(String::from_utf8(location.stdout).unwrap().trim())).unwrap(),
        bytes
    );
}

fn main_has_file(daemon: &common::Daemon, folder: &std::path::Path) -> bool {
    let out = common::git_env(
        Command::new("git")
            .current_dir(folder)
            .args(["ls-tree", "main", "--", "observed.txt"]),
        &daemon.home,
    )
    .output()
    .unwrap();
    out.status.success() && !out.stdout.is_empty()
}

fn observed_deletion_admissions(home: &std::path::Path) -> usize {
    std::fs::read_dir(home.join("state/logs"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| {
            std::fs::read_to_string(entry.path())
                .unwrap_or_default()
                .lines()
                .filter(|line| {
                    serde_json::from_str::<serde_json::Value>(line).is_ok_and(|row| {
                        row["message"] == "capture-observation-admission"
                            && row["native"] == true
                            && row["watch_revision"] == 0
                            && row["invalidated"] == true
                            && row["due"] == true
                    })
                })
                .count()
        })
        .sum()
}

#[test]
fn a_pending_observation_wakes_capture_even_when_native_hints_are_lost() {
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let config = root.join("lost/config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"),
        "[network]\nbind=\"127.0.0.1:0\"\nmdns=false\nrelays=\"off\"\n[loops]\ninterval_seconds=600\n").unwrap();
    let daemon = common::start_with_env(
        &root,
        sockets.path(),
        "lost",
        &[("PIGEON_LAB_SUPPRESS_CHANGE_HINTS", "1")],
    );
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("observed.txt"), b"a retained native version\n").unwrap();
    let created = pigeon(
        &daemon,
        &["create", "Lost hints", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some());
    let deadline = Instant::now() + Duration::from_secs(30);
    while !main_has_file(&daemon, &folder) {
        assert!(Instant::now() < deadline, "initial capture never completed");
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_secs(1));
    let before = observed_deletion_admissions(&daemon.home);
    std::fs::remove_file(folder.join("observed.txt")).unwrap();
    std::thread::sleep(Duration::from_secs(3));
    // A real public observation discovers the pending deletion. It must wake
    // capture without waiting for either the 600-second loop or fallback.
    let history = pigeon(
        &daemon,
        &["history", folder.join("observed.txt").to_str().unwrap()],
    );
    assert!(
        history.iter().any(|reply| matches!(reply, pigeon_proto::Response::History { versions } if !versions.is_empty()))
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while main_has_file(&daemon, &folder) {
        assert!(
            Instant::now() < deadline,
            "an observed pending deletion did not wake capture\n{}",
            common::logs(&[&daemon])
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        observed_deletion_admissions(&daemon.home) > before,
        "no pending observation admitted capture independently of native hints"
    );
}
