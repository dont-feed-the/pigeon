//! The public erasure lifecycle on real peers. Explicit preparation needs
//! all current participants; readiness never erases on its own. Review,
//! confirmation, named waits and cancellation use the actual socket/CLI.
#![cfg(feature = "lab")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "real-peer erasure fixture"
)]

mod common;
use common::*;
use pigeon_proto::{Response, api::State, erase::Answer};

fn answer(responses: Vec<Response>) -> Option<Answer> {
    responses
        .into_iter()
        .filter_map(|r| match r {
            Response::Erasure { answer } => Some(*answer),
            _ => None,
        })
        .next_back()
}

fn rules(folder: &std::path::Path) -> u32 {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(folder)
        .args(["show", "HEAD:.pigeon/roster.toml"])
        .output()
        .unwrap();
    assert!(output.status.success());
    pigeond::roster_file::parse(std::str::from_utf8(&output.stdout).unwrap())
        .unwrap()
        .roster
        .rules
}

#[test]
fn preparation_confirmation_and_cancellation_preserve_current_and_retained_bytes() {
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
    let folder = root.join("family-files");
    std::fs::create_dir_all(&folder).unwrap();
    let file = folder.join("keep-until-confirmed.txt");
    let bytes = b"exact bytes retained throughout a pending erasure\0\xff\n";
    std::fs::write(&file, bytes).unwrap();
    let created = pigeon(
        &alice,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&alice, &vault, &all);
    let code = invitation(&alice);
    let bob_files = root.join("bob-files");
    let joined = pigeon(
        &bob,
        &[
            "join",
            &code,
            bob_files.to_str().unwrap(),
            "--person",
            "Bob",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    until(&alice, State::Protected, &all);
    // Per-person folders use rules 2, without preparing destructive work.
    // The later signed upgrade must still collect every participant's vote.
    let people = folder.join("People");
    std::fs::create_dir_all(&people).unwrap();
    let configured = pigeon(
        &alice,
        &[
            "folder-rule",
            people.to_str().unwrap(),
            "--per-person",
            "--yes",
        ],
    );
    assert!(
        said(&configured, "folder-rule-result", "folder").is_some(),
        "{configured:?}"
    );
    assert_eq!(rules(&folder), 2);
    let path = file.to_str().unwrap();
    let preview = answer(pigeon(&alice, &["erase", path, "--preview", "--no-input"])).unwrap();
    assert!(
        matches!(preview, Answer::Preview(p) if !p.readiness.ready && p.readiness.preparation_needed && p.current_files == 1)
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    let refused = pigeon(
        &bob,
        &[
            "erase",
            bob_files.join("keep-until-confirmed.txt").to_str().unwrap(),
            "--prepare",
            "--yes",
        ],
    );
    assert!(
        refused
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "not-allowed")),
        "{refused:?}"
    );
    let prepared = answer(pigeon(&alice, &["erase", path, "--prepare", "--yes"])).unwrap();
    assert!(matches!(prepared, Answer::Preparing(_)));
    eventually_within(
        120,
        || matches!(answer(pigeon(&alice, &["erase", path, "--preview", "--no-input"])), Some(Answer::Preview(p)) if p.readiness.ready),
        "all current peers did not finish erasure preparation",
        &all,
    );
    assert_eq!(rules(&folder), pigeon_core::purge::RULES);
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    assert!(
        matches!(answer(pigeon(&alice, &["erase"])), Some(Answer::Decisions { decisions, .. }) if decisions.is_empty())
    );
    // Without --yes no noninteractive request may submit an erasure.
    let no_input = pigeon(&alice, &["erase", path, "--no-input"]);
    assert!(
        no_input
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "needs-confirmation")),
        "{no_input:?}"
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    eventually_within(
        60,
        || {
            let result = pigeon(&alice, &["erase", path, "--yes"]);
            let submitted = result.iter().any(|r| matches!(r, Response::Erasure { answer } if matches!(answer.as_ref(), Answer::Submitted { needs_approval: false, .. })));
            if !submitted {
                assert!(
                    result
                        .iter()
                        .any(|r| matches!(r, Response::Problem(p) if p.code == "erase-stale")),
                    "unexpected confirmation failure: {result:?}"
                );
            }
            submitted
        },
        "freshly confirmed erasure did not enter main",
        &all,
    );
    eventually_within(
        60,
        || !file.exists(),
        "confirmed current deletion did not install",
        &all,
    );
    let decisions = match answer(pigeon(&alice, &["erase"])).unwrap() {
        Answer::Decisions { decisions, .. } => decisions,
        other => panic!("unexpected erasure list: {other:?}"),
    };
    assert_eq!(decisions.len(), 1);
    assert!(!decisions[0].completed);
    assert_eq!(decisions[0].waiting.len(), 1);
    assert!(!decisions[0].waiting[0].name.is_empty());
    let cancelled = answer(pigeon(
        &alice,
        &["erase", "--cancel", &decisions[0].id, "--yes"],
    ))
    .unwrap();
    assert!(
        matches!(cancelled, Answer::Cancelled { .. }),
        "{cancelled:?}"
    );
    assert!(
        !file.exists(),
        "cancellation does not silently restore a current file"
    );
    eventually_within(
        60,
        || matches!(answer(pigeon(&alice, &["erase"])), Some(Answer::Decisions { decisions, .. }) if decisions.is_empty()),
        "cancellation was not accepted",
        &all,
    );
    // No waiting month is simulated. The old content must still be here,
    // and restoring it explicitly makes a new ordinary reference.
    let restored = pigeon(&alice, &["restore", path, "--version", "1"]);
    assert!(
        said(&restored, "restore-result", "path").is_some(),
        "an already local version must restore without retrying: {restored:?}"
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
}

#[test]
fn preparing_without_a_backup_explains_the_missing_machine_without_writing_a_request() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let alice = start(&root, sockets.path(), "alice");
    let folder = root.join("family-files");
    std::fs::create_dir_all(&folder).unwrap();
    let file = folder.join("original.txt");
    let bytes = b"no erasure preparation without a backup\n";
    std::fs::write(&file, bytes).unwrap();
    let result = pigeon(
        &alice,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&result, "created", "group").is_some());
    let path = file.to_str().unwrap();
    eventually_within(
        60,
        || matches!(answer(pigeon(&alice, &["erase", path, "--preview"])), Some(Answer::Preview(p)) if p.readiness.problem.as_ref().is_some_and(|p| p.id == "erase-needs-backup")),
        "preview did not explain the missing backup",
        &[&alice],
    );
    let refused = pigeon(&alice, &["erase", path, "--prepare", "--yes"]);
    assert!(
        refused
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "erase-needs-backup")),
        "{refused:?}"
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    assert!(
        matches!(answer(pigeon(&alice, &["erase"])), Some(Answer::Decisions { decisions, .. }) if decisions.is_empty())
    );
}

#[test]
fn refounding_archives_a_pending_request_and_requires_fresh_preparation_after_rejoining() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let alice = start(&root, sockets.path(), "alice");
    let vault = start(&root, sockets.path(), "vault");
    let all = [&alice, &vault];
    let folder = root.join("files");
    std::fs::create_dir_all(&folder).unwrap();
    let file = folder.join("pending.txt");
    let keep = folder.join("keep.txt");
    let bytes = b"retained across a new authority domain\r\n\0\xff";
    std::fs::write(&file, bytes).unwrap();
    std::fs::write(&keep, b"current file stays exact\n").unwrap();
    let created = pigeon(
        &alice,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    join(&alice, &vault, &all);
    until(&alice, State::Protected, &all);
    let path = file.to_str().unwrap();
    assert!(matches!(
        answer(pigeon(&alice, &["erase", path, "--prepare", "--yes"])),
        Some(Answer::Preparing(_))
    ));
    eventually_within(
        120,
        || matches!(answer(pigeon(&alice, &["erase", path, "--preview"])), Some(Answer::Preview(p)) if p.readiness.ready),
        "original authority domain prepares",
        &all,
    );
    eventually_within(
        60,
        || {
            matches!(
                answer(pigeon(&alice, &["erase", path, "--yes"])),
                Some(Answer::Submitted {
                    needs_approval: false,
                    ..
                })
            )
        },
        "fresh confirmation submits the original request",
        &all,
    );
    eventually_within(60, || !file.exists(), "deletion installs", &all);
    let decision = match answer(pigeon(&alice, &["erase"])).unwrap() {
        Answer::Decisions { decisions, .. } => {
            assert_eq!(decisions.len(), 1);
            assert!(!decisions[0].completed);
            decisions[0].id.clone()
        }
        other => panic!("unexpected erasure list: {other:?}"),
    };
    let old = pigeond::config::load(&alice.home.join("config/config.toml"))
        .unwrap()
        .groups
        .remove(0)
        .id;
    let backup = pigeond::config::load(&vault.home.join("config/config.toml"))
        .unwrap()
        .groups
        .remove(0);
    let refounded = pigeon(&alice, &["refound", folder.to_str().unwrap(), "--yes"]);
    assert!(
        said(&refounded, "refounded", "group").is_some(),
        "{refounded:?}"
    );
    assert_eq!(std::fs::read(&keep).unwrap(), b"current file stays exact\n");
    assert!(!file.exists(), "refounding never restores implicitly");
    assert!(matches!(
        answer(pigeon(&alice, &["erase"])),
        Some(Answer::Decisions { decisions, .. }) if decisions.is_empty()
    ));
    let archive = format!("refs/pigeon/archive/{old}/main");
    let archived = std::process::Command::new("git")
        .arg("-C")
        .arg(&folder)
        .args(["ls-tree", "-r", "--name-only", &archive])
        .output()
        .unwrap();
    assert!(archived.status.success());
    assert!(String::from_utf8_lossy(&archived.stdout).contains(pigeond::purge_file::PREFIX));
    let cancelled = pigeon(&alice, &["erase", "--cancel", &decision, "--yes"]);
    assert!(
        cancelled
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "erase-unknown")),
        "an old-domain request cannot be cancelled in the new domain: {cancelled:?}"
    );
    let restored = pigeon(&alice, &["restore", path, "--version", "1"]);
    assert!(
        said(&restored, "restore-result", "path").is_some(),
        "{restored:?}"
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    let code = invitation(&alice);
    let owner = person(&alice);
    let joined = pigeon(
        &vault,
        &[
            "join",
            "--backup-machine",
            &code,
            backup.path.to_str().unwrap(),
            "--person",
            &owner,
            "--yes",
        ],
    );
    assert!(said(&joined, "joined", "group").is_some(), "{joined:?}");
    until(&alice, State::Protected, &all);
    let preview = answer(pigeon(&alice, &["erase", path, "--preview"])).unwrap();
    assert!(
        matches!(preview, Answer::Preview(p) if !p.readiness.ready && p.readiness.preparation_needed)
    );
    assert!(matches!(
        answer(pigeon(&alice, &["erase", path, "--prepare", "--yes"])),
        Some(Answer::Preparing(_))
    ));
    eventually_within(
        120,
        || matches!(answer(pigeon(&alice, &["erase", path, "--preview"])), Some(Answer::Preview(p)) if p.readiness.ready),
        "new domain independently prepares without old capability votes",
        &all,
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    assert!(
        matches!(
            answer(pigeon(&alice, &["erase"])),
            Some(Answer::Decisions { decisions, .. }) if decisions.is_empty()
        ),
        "preparing in the new domain never reissues an archived request"
    );
}
