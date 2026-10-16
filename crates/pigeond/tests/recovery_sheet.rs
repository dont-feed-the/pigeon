//! Disaster recovery (09 §9.9, ADR 0047): all laptops are lost and the
//! sole backup belongs to a non-admin. The sheet, rather than that backup,
//! signs the recovered admin's new device. Revoking the used sheet stops
//! a second recovery; no secret key is written into the new home.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::*;
use pigeon_proto::Response;
use pigeon_proto::api::State;
use std::path::Path;

fn git(path: &Path, args: &[&str]) -> Vec<u8> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

fn secret_on_disk(path: &Path, secret: &[u8]) -> bool {
    for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
        let kind = entry.file_type().unwrap();
        if kind.is_dir() && secret_on_disk(&entry.path(), secret) {
            return true;
        }
        if kind.is_file()
            && std::fs::read(entry.path())
                .is_ok_and(|b| b.windows(secret.len()).any(|s| s == secret))
        {
            return true;
        }
    }
    false
}

#[test]
fn a_sheet_recovers_an_admin_from_someone_elses_backup() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let alice = start(&root, sockets.path(), "alice");
    let bob = start(&root, sockets.path(), "bob");
    let vault = start(&root, sockets.path(), "vault");
    let (fa, fb, recovered) = (
        root.join("alice-files"),
        root.join("bob-files"),
        root.join("recovered"),
    );
    std::fs::create_dir_all(&fa).unwrap();
    std::fs::write(fa.join("recipe.txt"), b"two apples\n").unwrap();
    let created = pigeon(
        &alice,
        &[
            "create",
            "Family",
            fa.to_str().unwrap(),
            "--k",
            "1",
            "--name",
            "alice-laptop",
        ],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    assert!(pigeon(&alice, &["people"]).iter().any(|a| matches!(a,
        Response::People { people } if people.iter().any(|p| p.this_person && p.recovery_recommended))));
    let before_backup = pigeon(&alice, &["recovery-sheet"]);
    assert!(
        before_backup
            .iter()
            .any(|a| matches!(a, Response::Problem(p) if p.code == "recovery-needs-backup"))
    );
    assert!(pigeon(&alice, &["people"]).iter().any(|a| matches!(a,
        Response::People { people } if people.iter().any(|p| p.this_person && p.recovery_recommended))));
    let ticket = invitation(&alice);
    let joined = pigeon(
        &bob,
        &[
            "join",
            &ticket,
            "--person",
            "Bob",
            fb.to_str().unwrap(),
            "--name",
            "bob-laptop",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    join(&bob, &vault, &[&alice, &bob, &vault]);
    until(&alice, State::Protected, &[&alice, &bob, &vault]);
    std::fs::write(fa.join("recipe.txt"), b"three apples\n").unwrap();
    until(&alice, State::Protected, &[&alice, &bob, &vault]);
    eventually_within(
        120,
        || std::fs::read(fb.join("recipe.txt")).is_ok_and(|b| b == b"three apples\n"),
        "Bob receives latest recipe",
        &[&alice, &bob, &vault],
    );
    assert!(pigeon(&bob, &["people"]).iter().any(|r| matches!(r,
        Response::People { people } if people.iter().any(|p| p.this_person && !p.admin))));
    until(&bob, State::Protected, &[&alice, &bob, &vault]);
    eventually_within(
        120,
        || {
            let journal = fb.join(".git/pigeon/install.ndjson");
            std::fs::metadata(&journal).is_ok_and(|m| m.len() == 0)
                && std::process::Command::new("git")
                    .arg("-C")
                    .arg(&fb)
                    .env("GIT_OPTIONAL_LOCKS", "0")
                    .args(["status", "--porcelain", "--", "recipe.txt"])
                    .output()
                    .is_ok_and(|o| o.status.success() && o.stdout.is_empty())
        },
        "incoming installation fully settles before participant edit",
        &[&alice, &bob, &vault],
    );
    // ADR 0071 deliberately treats an edit within ten seconds of an
    // incoming install as a conflict copy. This fixture needs a suggestion.
    std::thread::sleep(std::time::Duration::from_secs(12));
    std::fs::write(fb.join("recipe.txt"), b"Bob's declined recipe\n").unwrap();
    let proposals = || {
        pigeon(&alice, &["suggestions"])
            .into_iter()
            .find_map(|r| match r {
                Response::Proposals { proposals, .. } => Some(proposals),
                _ => None,
            })
            .unwrap_or_default()
    };
    eventually_within(
        120,
        || !proposals().is_empty(),
        "real recipe suggestion",
        &[&alice, &bob, &vault],
    );
    let proposal = proposals()[0].id.clone();
    let declined = pigeon(
        &alice,
        &["decline", &proposal, "--note", "Keep the three apples"],
    );
    assert!(said(&declined, "declined-suggestion", "author").is_some());
    let vault_repo = pigeond::config::load(&vault.home.join("config/config.toml"))
        .unwrap()
        .groups[0]
        .path
        .clone();
    eventually_within(
        120,
        || {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&vault_repo)
                .args(["ls-tree", "-r", "--name-only", "refs/heads/main"])
                .output()
                .is_ok_and(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .contains(&format!(".pigeon/decisions/{proposal}.toml"))
                })
        },
        "backup receives decline before losing laptops",
        &[&alice, &bob, &vault],
    );
    let printed = pigeon(&alice, &["recovery-sheet"]);
    let sheet_text = said(&printed, "recovery-sheet-text", "sheet").unwrap();
    assert!(pigeon(&alice, &["people"]).iter().any(|a| matches!(a,
        Response::People { people } if people.iter().any(|p| p.this_person
            && !p.recovery_recommended && !p.recovery_refresh_recommended))));
    let sheet = pigeond::recovery::decode(&sheet_text).unwrap();
    let sheet_key: [u8; 32] = pigeond::util::unhex(&sheet.key)
        .unwrap()
        .try_into()
        .unwrap();
    let sheet_device = pigeon_core::sig::device_id(&sheet_key);
    let private = root.join("sheet.txt");
    pigeond::util::write_private(&private, sheet_text.as_bytes()).unwrap();
    // A key absent from the roster cannot fetch even the metadata.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let outsider = pigeond::net::Node::bind(&pigeond::net::NetConfig {
            secret_key: [99; 32],
            bind: "127.0.0.1:0".parse().unwrap(),
            mdns: false,
            reach: pigeond::net::Reach::Local,
        })
        .await
        .unwrap();
        let custodian = &sheet.custodians[0];
        let addresses = custodian
            .addresses
            .iter()
            .filter_map(|a| a.parse().ok())
            .collect();
        assert!(
            pigeond::net::recovery::fetch(
                &outsider,
                &custodian.device,
                addresses,
                &sheet.group,
                &root
            )
            .await
            .is_err(),
            "an unknown key read recovery metadata"
        );
        outsider.close().await;
    });
    drop(runtime);
    drop(alice);
    drop(bob);
    let fresh = start(&root, sockets.path(), "fresh");
    let answer = pigeon(
        &fresh,
        &[
            "recover",
            private.to_str().unwrap(),
            recovered.to_str().unwrap(),
        ],
    );
    assert!(
        said(&answer, "recovered", "group").is_some(),
        "{answer:?}\n{}",
        logs(&[&vault, &fresh])
    );
    eventually_within(
        120,
        || std::fs::read(recovered.join("recipe.txt")).is_ok_and(|s| s == b"three apples\n"),
        "recovery did not retrieve the current bytes",
        &[&vault, &fresh],
    );
    let commit = git(
        &recovered,
        &[
            "log",
            "--format=%H",
            "--fixed-strings",
            "--grep=pigeon: recover with a sheet",
            "main",
        ],
    );
    let commit = String::from_utf8(commit).unwrap();
    let raw = git(&recovered, &["cat-file", "commit", commit.trim()]);
    assert_eq!(pigeon_core::sig::verify(&raw).unwrap(), sheet_device);
    let history = pigeon(
        &fresh,
        &["history", recovered.join("recipe.txt").to_str().unwrap()],
    );
    assert!(
        history
            .iter()
            .any(|r| matches!(r, Response::History { versions } if versions.len() == 2)),
        "{history:?}"
    );
    assert!(
        !secret_on_disk(&fresh.home, sheet.key.as_bytes()),
        "sheet secret persisted in daemon home"
    );
    assert!(
        !secret_on_disk(&recovered, sheet.key.as_bytes()),
        "sheet secret persisted in repository"
    );
    let machines = pigeon(&fresh, &["machines"]);
    let name = machines
        .iter()
        .find_map(|r| match r {
            Response::Machines { machines, .. } => machines
                .iter()
                .find(|m| m.device == sheet_device.0)
                .map(|m| m.name.clone()),
            _ => None,
        })
        .unwrap();
    // The paper names itself as the People page lists it.
    assert_eq!(sheet.label, name);
    assert!(
        sheet_text.contains(&format!("This sheet: {name}")),
        "{sheet_text}"
    );
    let revoked = pigeon(&fresh, &["remove-computer", &name, "--yes"]);
    assert!(
        said(&revoked, "revoked-sheet", "name").is_some(),
        "{revoked:?}"
    );
    until(&fresh, State::Protected, &[&vault, &fresh]);
    let backup = vault
        .home
        .join("config")
        .join(format!("{}.git", sheet.group));
    eventually_within(
        120,
        || {
            let raw = git(&backup, &["show", "main:.pigeon/roster.toml"]);
            pigeond::roster_file::parse(&String::from_utf8_lossy(&raw))
                .is_ok_and(|d| !d.roster.devices.contains_key(&sheet_device))
        },
        "the backup never learned that the sheet was revoked",
        &[&vault, &fresh],
    );
    let second = start(&root, sockets.path(), "second");
    let denied = pigeon(
        &second,
        &[
            "recover",
            private.to_str().unwrap(),
            root.join("denied").to_str().unwrap(),
        ],
    );
    assert!(
        denied
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "recovery-not-allowed")),
        "revoked sheet recovered again"
    );
    assert!(said(&denied, "recovered", "group").is_none());
}
