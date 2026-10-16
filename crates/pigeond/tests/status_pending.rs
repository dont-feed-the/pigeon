//! Status never calls an uncaptured save safe, including a writer that
//! keeps its descriptor open while asking about the new path (spec 06 §6.2).
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use std::io::Write;

use common::{SERIAL, dirs, pigeon, said, start};
use pigeon_proto::Response;
use pigeon_proto::api::{Level, State};

#[test]
fn an_open_uncaptured_file_is_local_instead_of_safe() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let daemon = start(&root, sockets.path(), "a1");
    let folder = root.join("files");
    std::fs::create_dir_all(&folder).unwrap();
    let created = pigeon(
        &daemon,
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
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    let path = folder.join("fresh.txt");
    let mut writer = std::fs::File::create(&path).unwrap();
    writer.write_all(b"new bytes not captured yet\n").unwrap();
    writer.flush().unwrap();
    for args in [vec!["status"], vec!["status", path.to_str().unwrap()]] {
        let answers = pigeon(&daemon, &args);
        let report = answers
            .iter()
            .find_map(|a| {
                if let Response::Status(s) = a {
                    Some(s)
                } else {
                    None
                }
            })
            .unwrap();
        assert_ne!(report.level, Level::Safe, "{report:?}");
        assert_eq!(report.state, State::Local, "{report:?}");
        assert!(
            report.versions.iter().any(|v| v.path == "fresh.txt"
                && v.state == State::Local
                && v.reason.as_ref().is_some_and(|r| r.id == "why-capturing")),
            "{report:?}"
        );
    }
    drop(writer);
}
