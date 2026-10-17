//! Native create/join preflight refuses a real unsupported Linux volume
//! before making a folder, index or annex repository (14 §14.2).
#![cfg(feature = "lab")]
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, reason = "native CLI filesystem fixtures")]

mod common;

use common::{dirs, invitation, pigeon, said, start};
use pigeon_proto::Response;

fn refused(answer: &[Response]) {
    assert!(
        answer
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "file-system-unsupported")),
        "{answer:?}"
    );
}

#[test]
fn create_and_both_join_roles_refuse_a_native_unsupported_volume_before_writes() {
    let Some((_temp, root, sockets)) = dirs() else {
        return;
    };
    if let Ok(expected) = std::env::var("PIGEON_TEST_FILESYSTEM") {
        assert_eq!(pigeon_sys::volume::inspect(&root).unwrap().name, expected);
    }
    let a = start(&root, sockets.path(), "fs-owner");
    let b = start(&root, sockets.path(), "fs-joiner");
    let target = format!(
        "/proc/pigeon-filesystem-{}-never-create/nested",
        std::process::id()
    );
    refused(&pigeon(
        &a,
        &["create", "Family", &target, "--k", "1", "--name", "owner"],
    ));
    assert!(!std::path::Path::new(&target).exists());
    let unsupported = tempfile::tempdir_in("/dev/shm").unwrap();
    let existing = unsupported.path().join("important.bin");
    let saved = b"existing private file\0\xff\n";
    std::fs::write(&existing, saved).unwrap();
    let unsupported_path = unsupported.path().to_str().unwrap();
    refused(&pigeon(
        &a,
        &[
            "create",
            "Family",
            unsupported_path,
            "--k",
            "1",
            "--name",
            "owner",
        ],
    ));
    assert_eq!(std::fs::read(&existing).unwrap(), saved);
    let folder = root.join("supported-fixture");
    let made = pigeon(
        &a,
        &[
            "create",
            "Family",
            folder.to_str().unwrap(),
            "--k",
            "1",
            "--name",
            "owner",
        ],
    );
    assert!(said(&made, "created", "group").is_some(), "{made:?}");
    let code = invitation(&a);
    for backup in [false, true] {
        let mut args = vec![
            "join", &code, &target, "--name", "joiner", "--person", "ana", "--yes",
        ];
        if backup {
            args.push("--backup-machine");
        }
        refused(&pigeon(&b, &args));
        assert!(!std::path::Path::new(&target).exists());
        let mut existing_args = vec![
            "join",
            &code,
            unsupported_path,
            "--name",
            "joiner",
            "--person",
            "ana",
            "--yes",
        ];
        if backup {
            existing_args.push("--backup-machine");
        }
        refused(&pigeon(&b, &existing_args));
        assert_eq!(std::fs::read(&existing).unwrap(), saved);
        assert_eq!(std::fs::read_dir(unsupported.path()).unwrap().count(), 1);
    }
    let configuration = std::fs::read_to_string(b.home.join("config/config.toml")).unwrap();
    assert!(
        !configuration.contains("[[groups]]"),
        "a refused join must not record a group"
    );
}

#[test]
fn doctor_reports_an_existing_repository_on_a_native_unsupported_volume() {
    let Some((_temp, root, sockets)) = dirs() else {
        return;
    };
    let volume = tempfile::tempdir_in("/dev/shm").unwrap();
    assert_eq!(
        pigeon_sys::volume::inspect(volume.path()).unwrap().name,
        "tmpfs"
    );
    let folder = volume.path().join("legacy");
    std::fs::create_dir(&folder).unwrap();
    // Simulate an existing pre-admission repository; the public create
    // command must refuse this volume, so it cannot build this fixture.
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec![
            "-c",
            "user.name=Legacy fixture",
            "-c",
            "user.email=fixture@pigeon",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "legacy",
        ],
    ] {
        let output = common::git_env(&mut std::process::Command::new("git"), &root)
            .arg("-C")
            .arg(&folder)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(&folder)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let id = String::from_utf8(output.stdout).unwrap().trim().to_owned();
    let d = start(&root, sockets.path(), "fs-doctor");
    let home = d.home.clone();
    drop(d);
    pigeond::config::add_group(
        &home.join("config/config.toml"),
        &pigeond::config::GroupEntry {
            id,
            path: folder,
            priority: Default::default(),
        },
    )
    .unwrap();
    let d = start(&root, sockets.path(), "fs-doctor");
    refused(&pigeon(&d, &["doctor"]));
}
