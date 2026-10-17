//! Real Home health without a telemetry stack (12 §12.2). Current and
//! retained counts share status evidence; a signed checking report cannot
//! count as protection, and unreadable refs cannot become empty safe data.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;
use common::*;
use pigeon_git::GitEnv;
use pigeon_proto::api::{Level, State};
use pigeond::{
    git::Git,
    report_file::{self, Read},
};

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}
fn field<'a>(page: &'a str, id: &str) -> &'a str {
    page.split(&format!("id=\"{id}\""))
        .nth(1)
        .unwrap()
        .split_once('>')
        .unwrap()
        .1
        .split('<')
        .next()
        .unwrap()
}

#[test]
fn checking_and_unreadable_evidence_never_becomes_protected_health() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let vault = start(&root, sockets.path(), "vault");
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    let answer = pigeon(
        &lap,
        &[
            "create",
            "Family",
            folder.to_str().unwrap(),
            "--k",
            "1",
            "--name",
            "laptop",
        ],
    );
    assert!(said(&answer, "created", "group").is_some(), "{answer:?}");
    assert_eq!(status(&lap).headline.id, "headline-empty");
    std::fs::write(folder.join("note.txt"), b"first bytes\n").unwrap();
    join(&lap, &vault, &[&lap, &vault]);
    until(&lap, State::Protected, &[&lap, &vault]);
    std::fs::write(folder.join("note.txt"), b"second bytes\n").unwrap();
    until(&lap, State::Protected, &[&lap, &vault]);
    let session = web::login(&lap);
    let page = plain(&web::page(&session, "/"));
    assert!(field(&page, "health-current").contains("Safe: 1"), "{page}");
    assert!(
        field(&page, "health-retained").contains("Safe: 1"),
        "{page}"
    );
    let backup_session = web::login(&vault);
    eventually_within(
        60,
        || {
            let page = plain(&web::page(&backup_session, "/"));
            field(&page, "health-current").contains("Safe: 1")
                && field(&page, "health-retained").contains("Safe: 1")
        },
        "backup ignored its own observed stored copies",
        &[&lap, &vault],
    );

    let secret: [u8; 32] = pigeond::util::unhex(
        std::fs::read_to_string(vault.home.join("config/device-key"))
            .unwrap()
            .trim(),
    )
    .unwrap()
    .try_into()
    .unwrap();
    let device = pigeon_core::sig::device_id(&secret).0;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let git = Git::new(&folder, GitEnv::default());
    let reference = format!("refs/pigeon/status/{device}");
    eventually_within(
        60,
        || runtime.block_on(git.rev(&reference)).is_some(),
        "no signed backup report",
        &[&lap, &vault],
    );
    drop(vault);
    // Stop the receiver too: an in-flight valid report must not race the
    // deliberate checking-report fixture's compare-and-swap.
    drop(lap);
    runtime.block_on(async {
        let old = git.rev(&reference).await.unwrap();
        let bytes = git.blob(&format!("{reference}:status.toml")).await.unwrap();
        let Read::Known(mut report) = report_file::parse(&String::from_utf8_lossy(&bytes)) else {
            panic!("unreadable fixture report")
        };
        report.checking = true;
        report.at = report_file::datetime(pigeond::util::now() + 1);
        let text = report_file::render(&report).unwrap();
        let blob = git
            .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
            .await
            .unwrap();
        let tree = git
            .run_in(
                &["mktree"],
                format!("100644 blob {}\tstatus.toml\n", blob.trim()).as_bytes(),
            )
            .await
            .unwrap();
        let unsigned = git
            .run(&["commit-tree", tree.trim(), "-m", "checking fixture"])
            .await
            .unwrap();
        let signed = git.sign(unsigned.trim(), &secret).await.unwrap();
        git.update_ref(&reference, &signed, Some(&old))
            .await
            .unwrap();
    });
    let lap = start(&root, sockets.path(), "lap");
    let session = web::login(&lap);
    eventually_within(
        60,
        || plain(&web::page(&session, "/")).contains("Checking stored files"),
        "checking report was not shown",
        &[&lap],
    );
    let page = plain(&web::page(&session, "/"));
    assert!(field(&page, "health-current").contains("Safe: 0"), "{page}");
    assert!(
        field(&page, "health-retained").contains("Safe: 0"),
        "{page}"
    );
    assert_ne!(status(&lap).level, Level::Safe);

    // A broken main ref is a read failure, never an empty safe group.
    let main = folder.join(".git/refs/heads/main");
    let original = std::fs::read(&main).unwrap();
    std::fs::write(&main, format!("{}\n", "f".repeat(40))).unwrap();
    let page = plain(&web::page(&session, "/"));
    assert!(
        page.contains("Pigeon cannot check every saved version right now"),
        "{page}"
    );
    assert!(
        field(&page, "health-current").contains("Not checked yet"),
        "{page}"
    );
    assert!(
        field(&page, "health-retained").contains("Not checked yet"),
        "{page}"
    );
    assert_eq!(status(&lap).level, Level::NeedsYou);
    std::fs::write(&main, original).unwrap();
    assert_eq!(
        std::fs::read(folder.join("note.txt")).unwrap(),
        b"second bytes\n"
    );
}

#[test]
fn another_laptop_proves_retained_publication_after_backup_removal() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let bob = start(&root, sockets.path(), "bob");
    let vault = start(&root, sockets.path(), "vault");
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    let answer = pigeon(
        &lap,
        &[
            "create",
            "Family",
            folder.to_str().unwrap(),
            "--k",
            "1",
            "--name",
            "laptop",
        ],
    );
    assert!(said(&answer, "created", "group").is_some(), "{answer:?}");
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
    join(&lap, &vault, &[&lap, &bob, &vault]);
    std::fs::write(folder.join("note.txt"), b"first bytes\n").unwrap();
    until(&lap, State::Protected, &[&lap, &bob, &vault]);
    eventually_within(
        60,
        || std::fs::read(other.join("note.txt")).is_ok_and(|b| b == b"first bytes\n"),
        "first version never reached Bob",
        &[&lap, &bob, &vault],
    );
    std::fs::write(folder.join("note.txt"), b"second bytes\n").unwrap();
    until(&lap, State::Protected, &[&lap, &bob, &vault]);
    eventually_within(
        60,
        || std::fs::read(other.join("note.txt")).is_ok_and(|b| b == b"second bytes\n"),
        "second version never reached Bob",
        &[&lap, &bob, &vault],
    );
    let removed = pigeon(&lap, &["remove-computer", "vault", "--yes"]);
    assert!(
        said(&removed, "removed-computer", "name").is_some(),
        "{removed:?}"
    );
    drop(vault);
    for daemon in [&lap, &bob] {
        let session = web::login(daemon);
        eventually_within(
            90,
            || {
                let page = plain(&web::page(&session, "/"));
                let retained = field(&page, "health-retained");
                retained.contains("Shared, not safe yet: 1")
                    && retained.contains("Only on this computer: 0")
                    && field(&page, "health-current").contains("Safe: 0")
            },
            "retained publication ignored another laptop",
            &[&lap, &bob],
        );
    }
    assert_eq!(
        std::fs::read(folder.join("note.txt")).unwrap(),
        b"second bytes\n"
    );
    assert_eq!(
        std::fs::read(other.join("note.txt")).unwrap(),
        b"second bytes\n"
    );
}
