//! A real restore records its signed new reference before absent content
//! can arrive (08 §8.6). Restart retains the request and exact visible bytes;
//! supplying a copy later lets an explicit retry finish without a tombstone.
#![cfg(feature = "lab")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "real-peer restore fixture"
)]

mod common;
use common::*;
use pigeon_proto::Response;
use sha2::Digest;
use std::{path::Path, process::Command};

fn git(d: &Daemon, root: &Path, args: &[&str]) -> String {
    let output = git_env(Command::new("git").arg("-C").arg(root), &d.home)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn count(d: &Daemon, file: &Path) -> usize {
    pigeon(d, &["history", file.to_str().unwrap()])
        .iter()
        .filter_map(|r| match r {
            Response::History { versions } => Some(versions.len()),
            _ => None,
        })
        .sum()
}

#[test]
fn absent_bytes_leave_a_durable_new_version_and_retry_preserves_private_files() {
    absent_admin(false);
}

#[test]
fn an_absent_folder_snapshot_retains_its_request_before_receiving() {
    absent_admin(true);
}

fn absent_admin(folder_restore: bool) {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let alice = start(&root, sockets.path(), "alice");
    let folder = root.join("family");
    let docs = folder.join("Docs");
    std::fs::create_dir_all(&docs).unwrap();
    let file = docs.join("notes.txt");
    let old = b"original absent binary contents\0\xff\r\n";
    let new = b"current visible contents\0\xfe\r\n";
    std::fs::write(&file, old).unwrap();
    let created = pigeon(
        &alice,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    eventually_within(
        60,
        || count(&alice, &file) == 1,
        "first version missing",
        &[&alice],
    );
    let history = git(
        &alice,
        &folder,
        &["rev-list", "--reverse", "HEAD", "--", "Docs/notes.txt"],
    );
    let original = history.lines().next().unwrap();
    let pointer = git(
        &alice,
        &folder,
        &["show", &format!("{original}:Docs/notes.txt")],
    );
    let first_at = pigeon(&alice, &["history", file.to_str().unwrap()])
        .into_iter()
        .find_map(|r| match r {
            Response::History { versions } => versions.first().map(|v| v.saved_at.clone()),
            _ => None,
        })
        .unwrap();
    // Folder snapshots select by seconds, so the two snapshots must have
    // distinguishable dates. This is unrelated to receiver retention waits.
    std::thread::sleep(std::time::Duration::from_secs(2));
    let key = pointer
        .lines()
        .next()
        .unwrap()
        .strip_prefix("/annex/objects/")
        .unwrap();
    std::fs::write(&file, new).unwrap();
    let private = folder.join("unrelated.bin");
    let private_bytes = b"unrelated exact bytes must not be replaced\0\xf2\n";
    std::fs::write(&private, private_bytes).unwrap();
    eventually_within(
        60,
        || count(&alice, &file) == 2 && count(&alice, &private) == 1,
        "second version missing",
        &[&alice],
    );
    let index = git(&alice, &folder, &["write-tree"]);
    // Fixture-only forced removal supplies an absent saved version without
    // claiming any month elapsed or weakening the production purge gate.
    git(&alice, &folder, &["annex", "drop", "--force", "--key", key]);
    let args = if folder_restore {
        vec!["restore", docs.to_str().unwrap(), "--at", &first_at]
    } else {
        vec!["restore", file.to_str().unwrap(), "--version", "1"]
    };
    let result = pigeon(&alice, &args);
    assert!(
        result
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code=="restore-waiting")),
        "{result:?}"
    );
    let requested = git(&alice, &folder, &["rev-parse", "HEAD"]);
    assert_eq!(
        git(&alice, &folder, &["show", "HEAD:Docs/notes.txt"]),
        pointer
    );
    assert_eq!(
        git(&alice, &folder, &["write-tree"]),
        index,
        "receiving must not rewrite the index"
    );
    assert_eq!(std::fs::read(&file).unwrap(), new);
    assert_eq!(std::fs::read(&private).unwrap(), private_bytes);
    assert_eq!(
        count(&alice, &file),
        3,
        "the requested restore has its own new reference"
    );
    drop(alice);
    let alice = start(&root, sockets.path(), "alice");
    assert_eq!(git(&alice, &folder, &["rev-parse", "HEAD"]), requested);
    assert_eq!(std::fs::read(&file).unwrap(), new);
    assert_eq!(std::fs::read(&private).unwrap(), private_bytes);
    let supply = root.join("available-copy.bin");
    std::fs::write(&supply, old).unwrap();
    git(
        &alice,
        &folder,
        &["annex", "setkey", key, supply.to_str().unwrap()],
    );
    let result = pigeon(&alice, &args);
    let message = if folder_restore {
        "restore-folder-result"
    } else {
        "restore-result"
    };
    assert!(said(&result, message, "path").is_some(), "{result:?}");
    assert_eq!(std::fs::read(&file).unwrap(), old);
    assert_eq!(std::fs::read(&private).unwrap(), private_bytes);
}

#[test]
fn a_member_requests_absent_bytes_as_a_signed_suggestion_without_changing_main() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let alice = start(&root, sockets.path(), "alice");
    let bob = start(&root, sockets.path(), "bob");
    let folder = root.join("family");
    std::fs::create_dir_all(&folder).unwrap();
    let file = folder.join("notes.txt");
    let old = b"member's requested original\0\xff\r\n";
    let new = b"accepted latest version\0\xfe\r\n";
    std::fs::write(&file, old).unwrap();
    let created = pigeon(
        &alice,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    eventually_within(
        60,
        || count(&alice, &file) == 1,
        "first version missing",
        &[&alice, &bob],
    );
    let code = invitation(&alice);
    let bob_folder = root.join("bob-files");
    let joined = pigeon(
        &bob,
        &[
            "join",
            &code,
            bob_folder.to_str().unwrap(),
            "--person",
            "Bob",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    let bob_file = bob_folder.join("notes.txt");
    eventually_within(
        60,
        || std::fs::read(&bob_file).is_ok_and(|b| b == old),
        "Bob never received original",
        &[&alice, &bob],
    );
    std::fs::write(&file, new).unwrap();
    eventually_within(
        60,
        || std::fs::read(&bob_file).is_ok_and(|b| b == new) && count(&bob, &bob_file) == 2,
        "Bob never received latest",
        &[&alice, &bob],
    );
    let history = git(
        &bob,
        &bob_folder,
        &["rev-list", "--reverse", "HEAD", "--", "notes.txt"],
    );
    let original = history.lines().next().unwrap();
    let pointer = git(
        &bob,
        &bob_folder,
        &["show", &format!("{original}:notes.txt")],
    );
    let key = pointer
        .lines()
        .next()
        .unwrap()
        .strip_prefix("/annex/objects/")
        .unwrap();
    drop(alice);
    // Isolate the unavailable-copy branch instead of spending the P2P
    // dial budget contacting a deliberately stopped fixture peer.
    for remote in git(&bob, &bob_folder, &["remote"]).lines() {
        git(
            &bob,
            &bob_folder,
            &["config", &format!("remote.{remote}.annex-ignore"), "true"],
        );
    }
    git(
        &bob,
        &bob_folder,
        &["annex", "drop", "--force", "--key", key],
    );
    let main = git(&bob, &bob_folder, &["rev-parse", "HEAD"]);
    let index = git(&bob, &bob_folder, &["write-tree"]);
    let result = pigeon(
        &bob,
        &["restore", bob_file.to_str().unwrap(), "--version", "1"],
    );
    assert!(
        result
            .iter()
            .any(|r| matches!(r,Response::Problem(p) if p.code=="restore-waiting")),
        "{result:?}"
    );
    assert_eq!(git(&bob, &bob_folder, &["rev-parse", "HEAD"]), main);
    assert_eq!(git(&bob, &bob_folder, &["write-tree"]), index);
    assert_eq!(std::fs::read(&bob_file).unwrap(), new);
    let suggestions = pigeon(&bob, &["suggestions"]);
    assert!(
        suggestions
            .iter()
            .any(|r| matches!(r, Response::Proposals {proposals}
        if proposals.iter().any(|p| p.mine && p.paths==["notes.txt"]))),
        "{suggestions:?}"
    );
    drop(bob);
    let bob = start(&root, sockets.path(), "bob");
    let suggestions = pigeon(&bob, &["suggestions"]);
    assert!(
        suggestions
            .iter()
            .any(|r| matches!(r, Response::Proposals {proposals}
        if proposals.iter().any(|p| p.mine && p.paths==["notes.txt"]))),
        "{suggestions:?}"
    );
    assert_eq!(std::fs::read(&bob_file).unwrap(), new);
    assert_eq!(git(&bob, &bob_folder, &["rev-parse", "HEAD"]), main);
    let supply = root.join("available-copy.bin");
    std::fs::write(&supply, old).unwrap();
    git(
        &bob,
        &bob_folder,
        &["annex", "setkey", key, supply.to_str().unwrap()],
    );
    let private = b"member's own newer save before retry\0\xfb\n";
    std::fs::write(&bob_file, private).unwrap();
    let restored = pigeon(
        &bob,
        &["restore", bob_file.to_str().unwrap(), "--version", "1"],
    );
    assert!(
        said(&restored, "restore-result", "path").is_some(),
        "{restored:?}"
    );
    assert_eq!(std::fs::read(&bob_file).unwrap(), old);
    assert_eq!(git(&bob, &bob_folder, &["rev-parse", "HEAD"]), main);
    let private_key = format!(
        "SHA256E-s{}--{}.txt",
        private.len(),
        pigeond::util::hex(&sha2::Sha256::digest(private))
    );
    let retained = git(
        &bob,
        &bob_folder,
        &["annex", "contentlocation", &private_key],
    );
    assert_eq!(std::fs::read(bob_folder.join(retained)).unwrap(), private);
}
