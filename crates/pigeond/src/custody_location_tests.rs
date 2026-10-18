//! Repeated native object-location observations stay fresh without per-key children.
#![allow(
    clippy::unwrap_used,
    reason = "isolated native annex object-location fixture"
)]

use std::os::unix::fs::PermissionsExt;

use super::*;

#[tokio::test]
async fn a_worker_reuses_native_location_queries_without_caching_presence() {
    let root = tempfile::tempdir().unwrap();
    let git = Git::new(root.path(), pigeon_git::GitEnv::default());
    if git.run(&["annex", "version", "--raw"]).await.is_err() {
        assert!(std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none());
        return;
    }
    git.run(&["init", "-q"]).await.unwrap();
    git.run(&["config", "user.name", "location fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "location@example.invalid"])
        .await
        .unwrap();
    git.run(&["annex", "init", "locations"]).await.unwrap();
    let bytes = b"the independently verified native object";
    std::fs::write(root.path().join("file.bin"), bytes).unwrap();
    let added = git
        .run(&[
            "-c",
            "annex.thin=false",
            "annex",
            "add",
            "--no-check-gitignore",
            "--json",
            "--",
            "file.bin",
        ])
        .await
        .unwrap();
    let answer: Answer = serde_json::from_str(added.lines().last().unwrap()).unwrap();
    assert!(answer.success);
    let key = answer.key.unwrap();
    let trace = root.path().join("native-starts.ndjson");
    let git = Git::new(
        root.path(),
        pigeon_git::GitEnv {
            extra: vec![("GIT_TRACE2_EVENT".into(), trace.clone().into_os_string())],
            ..Default::default()
        },
    );
    let mut pool = Pool::default();
    let location = pool.local_object(&git, &key).await.unwrap();
    assert_eq!(std::fs::read(&location).unwrap(), bytes);
    let parent = location.parent().unwrap();
    let permissions = std::fs::metadata(parent).unwrap().permissions();
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o755)).unwrap();
    let kept = root.path().join("kept-object");
    std::fs::rename(&location, &kept).unwrap();
    assert!(pool.local_object(&git, &key).await.is_none());
    std::fs::rename(kept, &location).unwrap();
    std::fs::set_permissions(parent, permissions).unwrap();
    assert_eq!(pool.local_object(&git, &key).await, Some(location.clone()));
    assert_eq!(std::fs::read(location).unwrap(), bytes);
    // The process is also bounded: the 257th ordinary lookup starts a fresh one.
    for _ in 3..257 {
        assert!(pool.local_object(&git, &key).await.is_some());
    }
    pool.stop().await;
    let starts = std::fs::read_to_string(trace)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|row| {
            row["event"] == "start"
                && row["argv"]
                    .as_array()
                    .is_some_and(|args| args.iter().any(|arg| arg == "contentlocation"))
        })
        .count();
    assert_eq!(
        starts, 2,
        "each observed key started another native metadata child"
    );
}

#[tokio::test]
async fn native_location_sessions_rotate_at_the_input_byte_bound_too() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let count = root.path().join("starts");
    let script = bin.join("git");
    let body = r#"#!/usr/bin/env python3
import sys
with open(COUNTER,'a') as f:f.write('start\n')
for line in sys.stdin:print('',flush=True)
"#;
    std::fs::write(
        &script,
        body.replace("COUNTER", &format!("{:?}", count.to_str().unwrap())),
    )
    .unwrap();
    std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let git = Git::new(
        root.path(),
        pigeon_git::GitEnv {
            path_first: vec![bin],
            extra: Vec::new(),
        },
    );
    let mut pool = Pool::default();
    let key = "é".repeat(2500); // 5000 UTF-8 bytes plus one input delimiter.
    for _ in 0..30 {
        assert!(pool.local_object(&git, &key).await.is_none());
    }
    pool.stop().await;
    assert_eq!(std::fs::read_to_string(count).unwrap().lines().count(), 3);
}
