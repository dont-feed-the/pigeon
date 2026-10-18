//! Positive-answer barriers reuse only native lookup processes, never results.
#![allow(clippy::unwrap_used, reason = "isolated object durability fixtures")]

use super::{AnnexSyncer, Repository, Syncer};
use std::os::unix::fs::PermissionsExt;

fn fixture() -> (tempfile::TempDir, Repository, AnnexSyncer) {
    let root = tempfile::Builder::new()
        .prefix("p-sync-")
        .tempdir_in("/tmp")
        .unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let script = bin.join("git");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import os, pathlib, socket, sys, time
live = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
live.bind('live-'+str(os.getpid()))
with open('starts','a') as f: f.write('start\n')
def answer(key):
    if key == 'blocked':
        pathlib.Path('waiting').touch()
        while True: time.sleep(.1)
    print('.git/annex/objects/a/b/'+key+'/'+key, flush=True)
if '--batch' in sys.argv:
    for key in sys.stdin: answer(key.rstrip('\n'))
else: answer(sys.argv[-1])
"#,
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let repository = Repository {
        group: "synthetic".into(),
        git_dir: root.path().join(".git"),
        worktree: Some(root.path().into()),
        role: pigeon_core::filter::Role::Laptop,
        auth_token: None,
        checking: false,
    };
    let syncer = AnnexSyncer::new(
        Default::default(),
        pigeon_git::GitEnv {
            path_first: vec![bin],
            extra: Vec::new(),
        },
    );
    (root, repository, syncer)
}

fn object(repository: &Repository, n: usize) -> (String, std::path::PathBuf) {
    let data = format!("independent barrier bytes {n}\n").into_bytes();
    let key = format!(
        "SHA256E-s{}--{}",
        data.len(),
        crate::util::hex(&<sha2::Sha256 as sha2::Digest>::digest(&data))
    );
    let object = repository
        .git_dir
        .join("annex/objects/a/b")
        .join(&key)
        .join(&key);
    std::fs::create_dir_all(object.parent().unwrap()).unwrap();
    std::fs::write(&object, data).unwrap();
    (key, object)
}

fn starts(root: &std::path::Path) -> usize {
    std::fs::read_to_string(root.join("starts"))
        .unwrap()
        .lines()
        .count()
}

fn live_sockets(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("live-")
        })
        .collect()
}

async fn closed(sockets: &[std::path::PathBuf]) {
    let end = tokio::time::Instant::now() + std::time::Duration::from_secs(8);
    while sockets.iter().any(|path| {
        std::os::unix::net::UnixDatagram::unbound()
            .unwrap()
            .connect(path)
            .is_ok()
    }) {
        assert!(
            tokio::time::Instant::now() < end,
            "lookup child kept its private socket open"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn repeated_barriers_bound_native_starts_and_recheck_actual_bytes() {
    let (root, repository, syncer) = fixture();
    let mut objects = Vec::new();
    let mut previous_start = std::time::Instant::now();
    let mut possible_idle_expiries = 0;
    for n in 0..257 {
        let (key, object) = object(&repository, n);
        let started = std::time::Instant::now();
        assert!(syncer.sync(&repository, &key, true).await.unwrap());
        // An overloaded runner can legitimately expire the idle helper
        // between barriers. Account for that policy rather than assuming
        // the entire sequence always gets CPU within two seconds.
        if previous_start.elapsed() >= std::time::Duration::from_secs(2) {
            possible_idle_expiries += 1;
        }
        previous_start = started;
        objects.push((key, object));
    }
    assert!(
        (2..=2 + possible_idle_expiries).contains(&starts(root.path())),
        "one native lookup per key remains at the positive-answer barrier"
    );
    let (key, object) = &objects[0];
    std::fs::write(object, b"damaged bytes").unwrap();
    assert!(!syncer.sync(&repository, key, true).await.unwrap());
    std::fs::remove_file(object).unwrap();
    assert!(syncer.sync(&repository, key, false).await.is_err());
}

#[tokio::test]
async fn streams_own_separate_helpers_and_idle_or_dropped_streams_close_them() {
    let (root, repository, syncer) = fixture();
    let (key, _) = object(&repository, 0);
    let a = syncer.session().unwrap();
    let b = syncer.session().unwrap();
    let (one, two) = tokio::join!(
        a.sync(&repository, &key, true),
        b.sync(&repository, &key, true)
    );
    assert!(one.unwrap() && two.unwrap());
    assert_eq!(starts(root.path()), 2);
    let sockets = live_sockets(root.path());
    assert_eq!(sockets.len(), 2);
    closed(&sockets).await;
    assert!(a.sync(&repository, &key, true).await.unwrap());
    assert!(b.sync(&repository, &key, true).await.unwrap());
    assert_eq!(starts(root.path()), 4);
    drop(a);
    drop(b);
    closed(&live_sockets(root.path())).await;
}

#[tokio::test]
async fn replacing_a_repository_at_the_same_path_restarts_its_lookup() {
    let (root, repository, syncer) = fixture();
    let (key, _) = object(&repository, 0);
    assert!(syncer.sync(&repository, &key, true).await.unwrap());
    let original = live_sockets(root.path());
    std::fs::rename(&repository.git_dir, root.path().join("old-repository")).unwrap();
    let (new, _) = object(&repository, 1);
    assert!(syncer.sync(&repository, &new, true).await.unwrap());
    assert_eq!(starts(root.path()), 2);
    assert!(syncer.sync(&repository, &key, false).await.is_err());
    closed(&original).await;
}

#[tokio::test]
async fn canceling_a_lookup_cannot_give_its_reply_to_the_next_barrier() {
    let (root, repository, syncer) = fixture();
    let (key, _) = object(&repository, 0);
    let task = tokio::spawn(syncer.sync(&repository, "blocked", false));
    let end = tokio::time::Instant::now() + std::time::Duration::from_secs(8);
    while !root.path().join("waiting").exists() {
        assert!(tokio::time::Instant::now() < end, "lookup did not start");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let original = live_sockets(root.path());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(syncer.sync(&repository, &key, true).await.unwrap());
    assert_eq!(starts(root.path()), 2);
    closed(&original).await;
}
