//! Real signed folder setup (04 §4.3): an admin configures ownership,
//! ordinary members suggest instead of changing rules, stewards can edit,
//! and a metadata change cannot smuggle personal index entries into main.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "integration fixtures")]
mod common;
use common::*;
use pigeon_proto::{Response, api::State};
use std::path::Path;

fn report(d: &Daemon, path: &Path) -> Box<pigeon_proto::policy::Report> {
    pigeon(d, &["folder-rule", path.to_str().unwrap()])
        .into_iter()
        .find_map(|r| match r {
            Response::FolderRules(report) => Some(report),
            _ => None,
        })
        .unwrap()
}

#[test]
fn signed_folder_rules_enforce_authority_and_preserve_personal_index_work() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let bob = start(&root, sockets.path(), "bob");
    let folder = root.join("Family");
    let photos = folder.join("Photos");
    std::fs::create_dir_all(&photos).unwrap();
    std::fs::write(photos.join("first.txt"), b"Alice original\n").unwrap();
    std::fs::write(photos.join("second.txt"), b"Alice second\n").unwrap();
    let created = pigeon(
        &lap,
        &[
            "create",
            "Family",
            folder.to_str().unwrap(),
            "--k",
            "1",
            "--name",
            "alice-laptop",
        ],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    let code = invitation(&lap);
    let other = root.join("bob-files");
    let joined = pigeon(
        &bob,
        &[
            "join",
            &code,
            "--person",
            "Bob",
            other.to_str().unwrap(),
            "--name",
            "bob-laptop",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    until(&lap, State::Published, &[&lap, &bob]);
    eventually_within(
        60,
        || std::fs::read(other.join("Photos/first.txt")).is_ok_and(|b| b == b"Alice original\n"),
        "initial data not installed",
        &[&lap, &bob],
    );

    let configured = pigeon(
        &lap,
        &["folder-rule", photos.to_str().unwrap(), "--owner", "me"],
    );
    assert!(
        said(&configured, "folder-rule-result", "folder").is_some(),
        "{configured:?}"
    );
    let own = report(&lap, &photos);
    assert!(own.can_edit && own.rules[0].direct && own.rules[0].owner.is_some());
    eventually_within(
        60,
        || report(&bob, &other.join("Photos")).rules[0].owner.is_some(),
        "owner rule not shared",
        &[&lap, &bob],
    );
    let refused = pigeon(
        &bob,
        &[
            "folder-rule",
            other.join("Photos").to_str().unwrap(),
            "--owner",
            "me",
        ],
    );
    assert!(
        refused
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "not-allowed")),
        "{refused:?}"
    );
    std::fs::write(other.join("Photos/first.txt"), b"Bob personal suggestion\0").unwrap();
    eventually_within(
        60,
        || {
            pigeon(&bob, &["suggestions"]).iter().any(
                |r| matches!(r, Response::Proposals { proposals, .. } if !proposals.is_empty()),
            )
        },
        "ordinary change not suggested",
        &[&lap, &bob],
    );
    assert_eq!(
        std::fs::read(photos.join("first.txt")).unwrap(),
        b"Alice original\n"
    );

    let stewards = pigeon(
        &lap,
        &[
            "folder-rule",
            photos.to_str().unwrap(),
            "--add-only",
            "--steward",
            "Bob",
        ],
    );
    assert!(
        said(&stewards, "folder-rule-result", "folder").is_some(),
        "{stewards:?}"
    );
    eventually_within(
        60,
        || {
            let r = report(&bob, &other.join("Photos"));
            r.rules[0].owner.is_none() && !r.rules[0].stewards.is_empty()
        },
        "steward rule not shared",
        &[&lap, &bob],
    );
    std::fs::write(other.join("Photos/second.txt"), b"Bob steward edit\n").unwrap();
    eventually_within(
        90,
        || std::fs::read(photos.join("second.txt")).is_ok_and(|b| b == b"Bob steward edit\n"),
        "steward edit not accepted",
        &[&lap, &bob],
    );

    // Put real unpublished working bytes and an unrelated raw entry in
    // the personal index. A rule update must commit only roster metadata.
    let draft = folder.join("private-draft.txt");
    std::fs::write(&draft, b"editor bytes not accepted\0").unwrap();
    let writer = std::fs::OpenOptions::new()
        .write(true)
        .open(&draft)
        .unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let git = pigeond::git::Git::new(&folder, pigeon_git::GitEnv::default());
    let blob = runtime
        .block_on(git.run_in(&["hash-object", "-w", "--stdin"], b"private index entry"))
        .unwrap();
    // This artificial external index edit politely waits for real Git
    // writers; lock contention is not a failed product operation.
    eventually_within(
        15,
        || match runtime.block_on(git.run(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{},private-draft.txt", blob.trim()),
        ])) {
            Ok(_) => true,
            Err(error) if error.to_string().contains("index.lock") => false,
            Err(error) => panic!("fixture index edit: {error}"),
        },
        "fixture could not stage private index entry",
        &[&lap, &bob],
    );
    let reset = pigeon(
        &lap,
        &["folder-rule", photos.to_str().unwrap(), "--default"],
    );
    assert!(
        said(&reset, "folder-rule-result", "folder").is_some(),
        "{reset:?}"
    );
    let main = runtime.block_on(git.tree("refs/heads/main")).unwrap();
    assert!(!main.contains_key("private-draft.txt"));
    let index = runtime.block_on(git.index()).unwrap();
    assert_eq!(index["private-draft.txt"].oid, blob.trim());
    assert_eq!(
        std::fs::read(&draft).unwrap(),
        b"editor bytes not accepted\0"
    );
    let inherited = report(&lap, &photos);
    assert!(!inherited.rules[0].direct && inherited.rules[0].owner.is_none());
    // Collector changes and invitation welcomes use the same roster writer.
    // Keep the editor open throughout, so capture cannot accept these bytes.
    let selected = pigeon(&lap, &["settings", "telemetry-device", "alice-laptop"]);
    assert!(
        said(&selected, "setting-changed", "value").is_some(),
        "{selected:?}"
    );
    let cara = start(&root, sockets.path(), "cara");
    let code = invitation(&lap);
    let joined = pigeon(
        &cara,
        &[
            "join",
            &code,
            "--person",
            "Cara",
            root.join("cara-files").to_str().unwrap(),
            "--name",
            "cara-laptop",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    let main = runtime.block_on(git.tree("refs/heads/main")).unwrap();
    assert!(!main.contains_key("private-draft.txt"));
    let index = runtime.block_on(git.index()).unwrap();
    assert_eq!(index["private-draft.txt"].oid, blob.trim());
    assert_eq!(
        std::fs::read(&draft).unwrap(),
        b"editor bytes not accepted\0"
    );
    let peer_git = pigeond::git::Git::new(root.join("cara-files"), pigeon_git::GitEnv::default());
    let peer_main = runtime.block_on(peer_git.tree("refs/heads/main")).unwrap();
    assert!(!peer_main.contains_key("private-draft.txt"));
    assert!(!root.join("cara-files/private-draft.txt").exists());
    drop(writer);
}
