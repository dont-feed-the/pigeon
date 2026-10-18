//! A real disposable ZFS volume admits a Linux backup, refuses laptops
//! before writes, retains exact objects and reopens them after restart.
#![cfg(feature = "lab")]
#![cfg(target_os = "linux")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "explicit native ZFS fixture"
)]

mod common;
use std::path::Path;
use std::process::Command;

use common::{dirs, eventually_within, invitation, person, pigeon, said, start, until};
use pigeon_proto::Response;
use sha2::{Digest, Sha256};

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg(format!("--git-dir={}", repo.display()))
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "native fixture git failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim_end().to_owned()
}

fn exact(repo: &Path, path: &str, bytes: &[u8]) {
    let blob = git(repo, &["show", &format!("main:{path}")]);
    let key = pigeond::git::pointer_key(blob.as_bytes()).unwrap();
    let wanted: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(pigeond::net::sync::key_sha256(&key), Some(wanted.as_str()));
    let location = git(repo, &["annex", "contentlocation", &key]);
    let object = repo.join(location);
    assert!(std::fs::symlink_metadata(&object).unwrap().is_file());
    assert_eq!(std::fs::read(&object).unwrap(), bytes);
    assert_eq!(pigeon_sys::volume::inspect(&object).unwrap().name, "zfs");
}

#[test]
#[ignore = "requires scripts/check-zfs and its disposable native ZFS pool"]
fn a_native_zfs_backup_keeps_exact_objects_and_reopens_without_rejoining() {
    let volume =
        std::env::var_os("PIGEON_TEST_ZFS_ROOT").expect("explicit native ZFS root is required");
    let volume = Path::new(&volume);
    assert_eq!(pigeon_sys::volume::inspect(volume).unwrap().name, "zfs");
    let Some((_tmp, root, sockets)) = dirs() else {
        panic!("pinned annex is required")
    };
    let a = start(&root, sockets.path(), "zfs-owner");
    let c = start(&root, sockets.path(), "zfs-backup");
    let refused_folder = volume.join("laptop-must-not-exist");
    let refused = pigeon(
        &a,
        &[
            "create",
            "Refused",
            refused_folder.to_str().unwrap(),
            "--k",
            "1",
        ],
    );
    assert!(
        refused
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code=="file-system-unsupported"))
    );
    assert!(!refused_folder.exists());
    let folder = root.join("files");
    std::fs::create_dir_all(folder.join("nested")).unwrap();
    let alpha = b"native ZFS alpha\0\n";
    let beta = vec![47; 16_777];
    std::fs::write(folder.join("alpha.txt"), alpha).unwrap();
    std::fs::write(folder.join("nested/beta.bin"), &beta).unwrap();
    let created = pigeon(
        &a,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    let code = invitation(&a);
    let who = person(&a);
    let repo = volume.join("backup.git");
    let joined = pigeon(
        &c,
        &[
            "join",
            "--backup-machine",
            &code,
            repo.to_str().unwrap(),
            "--name",
            "native-zfs",
            "--person",
            &who,
            "--yes",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    // The general report omits already-protected entries. Ask each real
    // path, preserving the original protection observation's 120s budget.
    eventually_within(
        120,
        || {
            ["alpha.txt", "nested/beta.bin"].iter().all(|path| {
                pigeon(&a, &["status", folder.join(path).to_str().unwrap()])
                    .iter()
                    .any(|r| {
                        matches!(r, Response::Status(s) if s.versions.iter().any(|v|
                    v.path == *path && v.state == pigeon_proto::api::State::Protected))
                    })
            })
        },
        "both named ZFS-backed versions protected",
        &[&a, &c],
    );
    assert_eq!(pigeon_sys::volume::inspect(&repo).unwrap().name, "zfs");
    exact(&repo, "alpha.txt", alpha);
    exact(&repo, "nested/beta.bin", &beta);
    let uuid = git(&repo, &["config", "annex.uuid"]);
    let head = git(&repo, &["rev-parse", "main"]);
    drop(c);
    let c = start(&root, sockets.path(), "zfs-backup");
    until(&a, pigeon_proto::api::State::Protected, &[&a, &c]);
    assert_eq!(git(&repo, &["config", "annex.uuid"]), uuid);
    assert_eq!(git(&repo, &["rev-parse", "main"]), head);
    exact(&repo, "alpha.txt", alpha);
    exact(&repo, "nested/beta.bin", &beta);
}
