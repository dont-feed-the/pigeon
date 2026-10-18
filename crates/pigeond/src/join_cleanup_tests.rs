//! Laptop rollback must not delete another application's files created
//! after the initial empty-folder check (09 §9.6).
#![allow(clippy::unwrap_used, reason = "isolated join cleanup regressions")]

use super::*;
async fn preserves_later_project(existing: bool) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("files");
    if existing {
        std::fs::create_dir(&path).unwrap();
    }
    let attempt = Attempt::begin(&path).unwrap();
    assert!(attempt.empty().unwrap());
    // Another program creates a project while the join awaits its peer.
    // These paths' names do not establish that the attempt owns them.
    let files = [
        (
            ".git/objects/personal-note.txt",
            b"personal project history".as_slice(),
        ),
        (
            ".pigeon/personal-note.txt",
            b"personal project metadata".as_slice(),
        ),
        (".gitattributes", b"personal project attributes".as_slice()),
        ("notes.txt", b"personal project notes".as_slice()),
    ];
    for (name, bytes) in files {
        let file = path.join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, bytes).unwrap();
    }
    let result = attempt
        .run(async { Err::<(), _>("invite-proof".to_owned()) })
        .await;
    assert_eq!(result, Err("invite-proof".to_owned()));
    for (name, bytes) in files {
        assert_eq!(std::fs::read(path.join(name)).unwrap(), bytes, "{name}");
    }
}

#[tokio::test]
async fn a_failed_new_laptop_join_preserves_a_concurrent_project() {
    preserves_later_project(false).await;
}

#[tokio::test]
async fn a_failed_existing_laptop_join_preserves_a_concurrent_project() {
    preserves_later_project(true).await;
}
