//! Real capacity accounting (08 §8.9): duplicate keys, retained deletion,
//! declined ordinary-member content, signed disk reports and both languages.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "integration fixtures")]
mod common;
use common::*;
use pigeon_proto::{Response, api::State};

fn report(d: &Daemon) -> Box<pigeon_proto::capacity::Report> {
    pigeon(d, &["status", "--capacity"])
        .into_iter()
        .find_map(|r| {
            if let Response::Capacity(report) = r {
                Some(report)
            } else {
                None
            }
        })
        .unwrap()
}

#[test]
fn shared_keys_and_declined_suggestions_remain_visible_and_count_once() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let alice = start(&root, sockets.path(), "alice");
    let bob = start(&root, sockets.path(), "bob");
    let vault = start(&root, sockets.path(), "vault");
    let all = [&alice, &bob, &vault];
    let folder = root.join("Family");
    std::fs::create_dir_all(folder.join("Media")).unwrap();
    let old = b"original one\0";
    let new = b"replacement\0";
    let proposed = b"ordinary rejected\0";
    std::fs::write(folder.join("Media/a.bin"), old).unwrap();
    std::fs::write(folder.join("Media/b.bin"), old).unwrap();
    let result = pigeon(
        &alice,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&result, "created", "group").is_some());
    join(&alice, &vault, &all);
    // An empty accepted catalogue also has aggregate state Protected.
    // Require both actual captured paths before asking about their bytes.
    // Keep the original protection wait's 120-second observation budget.
    eventually_within(
        120,
        || {
            pigeon(&alice, &["status", folder.join("Media").to_str().unwrap()])
                .into_iter()
                .any(|r| {
                    matches!(r, Response::Status(s) if
                    ["Media/a.bin", "Media/b.bin"].iter().all(|path|
                        s.versions.iter().any(|v| v.path == *path && v.state == State::Protected)))
                })
        },
        "initial captured paths are not protected",
        &all,
    );
    let first = report(&alice);
    assert_eq!(first.current, old.len() as u64);
    assert_eq!(first.history, 0);
    let rule = pigeon(
        &alice,
        &[
            "folder-rule",
            folder.join("Media").to_str().unwrap(),
            "--owner",
            "me",
        ],
    );
    assert!(said(&rule, "folder-rule-result", "folder").is_some());
    let code = invitation(&alice);
    let other = root.join("bob-files");
    let result = pigeon(
        &bob,
        &["join", &code, "--person", "Bob", other.to_str().unwrap()],
    );
    assert!(said(&result, "joined", "group").is_some());
    eventually(
        || std::fs::read(other.join("Media/a.bin")).ok().as_deref() == Some(old.as_slice()),
        "initial version not installed",
        &all,
    );
    std::fs::write(folder.join("Media/a.bin"), new).unwrap();
    eventually(
        || std::fs::read(other.join("Media/a.bin")).ok().as_deref() == Some(new.as_slice()),
        "replacement not installed",
        &all,
    );
    // An ordinary proposal must be a deliberate later edit, outside ADR
    // 0071's ten-second stale-writer conflict interval.
    std::thread::sleep(std::time::Duration::from_secs(12));
    std::fs::write(other.join("Media/a.bin"), proposed).unwrap();
    let proposal = std::sync::Mutex::new(None);
    eventually(
        || {
            let id = pigeon(&alice, &["suggestions"])
                .into_iter()
                .find_map(|r| match r {
                    Response::Proposals { proposals, .. } => {
                        proposals.first().map(|p| p.id.clone())
                    }
                    _ => None,
                });
            let found = id.is_some();
            *proposal.lock().unwrap() = id;
            found
        },
        "ordinary suggestion not relayed",
        &all,
    );
    let id = proposal.into_inner().unwrap().unwrap();
    let declined = pigeon(&alice, &["decline", &id]);
    assert!(!declined.iter().any(|r| matches!(r, Response::Problem(_))));
    std::fs::remove_file(folder.join("Media/b.bin")).unwrap();
    eventually(
        || {
            pigeon(&alice, &["deleted"]).into_iter().any(|r| matches!(r, Response::Deleted { files, .. } if files.iter().any(|f| f.path == "Media/b.bin")))
        },
        "deletion not accepted",
        &all,
    );
    until(&alice, State::Protected, &all);
    let capacity = report(&alice);
    assert_eq!(capacity.current, new.len() as u64);
    assert_eq!(capacity.history, old.len() as u64);
    assert_eq!(capacity.suggestions, proposed.len() as u64);
    assert_eq!(capacity.overflow, Some(0));
    assert!(capacity.no_spare && capacity.limited.is_empty());
    assert!(capacity.proposals.iter().any(|p| p.id == id
        && p.state == "rejected"
        && p.additional_bytes == proposed.len() as u64));
    assert!(capacity.machines.iter().any(|m| m.machine.custodian
        && m.reported_at.is_some()
        && m.disk.is_some()
        && m.report_fresh));
    let session = web::login(&alice);
    let page = web::page(&session, "/space");
    assert!(
        page.contains("Backup storage")
            && page.contains("A suggestion saved")
            && page.contains("declined")
    );
    assert!(page.contains("no spare backup machine") && page.contains("Configured capacity"));
    assert!(!page.contains("smaller than one copy"));
    // Sizes are said in kB, MB or GB from 1,000 bytes on: never a raw count.
    let raw = page.match_indices(" bytes").any(|(at, _)| {
        let digits: String = page[..at]
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect();
        digits.len() > 3
    });
    assert!(!raw, "{page}");
    let settings = web::page(&session, "/settings");
    assert!(settings.contains("href=\"/space\""));
    pigeon(&alice, &["settings", "language", "fr"]);
    let french = web::page(&session, "/space");
    assert!(
        french.contains("Espace de sauvegarde")
            && french.contains("refusée")
            && french.contains("pas de machine de sauvegarde en réserve")
    );
    assert_eq!(std::fs::read(folder.join("Media/a.bin")).unwrap(), new);
    assert!(!folder.join("Media/b.bin").exists());
    let json = serde_json::to_vec(&Response::Capacity(capacity.clone())).unwrap();
    assert_eq!(
        serde_json::from_slice::<Response>(&json).unwrap(),
        Response::Capacity(capacity)
    );
}
