//! NUL-batch capture progress belongs to the planned file names, including
//! literal newlines. Progress renews inactivity, never a final result.
#![allow(clippy::unwrap_used, reason = "controlled native capture progress")]
use pigeon_git::{Error, GitEnv, Repo};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};
const LIMIT: Duration = Duration::from_millis(800);
const ARGS: &[&str] = &[
    "-c",
    "annex.thin=false",
    "annex",
    "add",
    "--batch",
    "-z",
    "--no-check-gitignore",
    "--json",
];
fn fixture(mode: &str) -> (tempfile::TempDir, Repo) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("git");
    std::fs::write(&file, r#"#!/usr/bin/env python3
import sys, json, os, time
progress = '--json-progress' in sys.argv
mode = os.environ['PIGEON_CAPTURE_MODE']
files = sys.stdin.buffer.read().decode().split('\0')[:-1]
for name in files:
    for count in range(1, 9):
        value = 1 if mode == 'repeat' else count
        reported = 'other-file' if mode == 'wrong-file' else name
        command = 'copy' if mode == 'wrong-command' else 'add'
        if progress:
            print(json.dumps({'byte-progress':value,'total-size':8,'action':{'command':command,'file':reported}}), flush=True)
        time.sleep(0.2)
    print(json.dumps({'command':'add','file':name,'key':'SHA256E-s8--fixture','success':True}), flush=True)
"#).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    let repo = Repo::new(
        dir.path(),
        GitEnv {
            path_first: vec![dir.path().into()],
            extra: vec![("PIGEON_CAPTURE_MODE".into(), mode.into())],
        },
    );
    (dir, repo)
}
#[tokio::test]
async fn moving_capture_files_outlive_the_deadline_and_keep_each_final_answer() {
    let (_dir, repo) = fixture("moving");
    let started = Instant::now();
    let output = repo
        .run(ARGS, Some(b"first.bin\0second\nfile.bin\0"), LIMIT)
        .await
        .unwrap();
    assert!(started.elapsed() > LIMIT);
    let answers: Vec<pigeon_git::annex::Answer> = output
        .text()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(answers.len(), 2);
    for (answer, name) in answers.iter().zip(["first.bin", "second\nfile.bin"]) {
        assert!(answer.success);
        assert_eq!(answer.file.as_deref(), Some(name));
    }
    assert!(!output.text().contains("byte-progress"));
}
async fn rejected(mode: &str) {
    let (_dir, repo) = fixture(mode);
    let result = repo.run(ARGS, Some(b"first.bin\0"), LIMIT).await;
    assert!(matches!(result, Err(Error::Timeout { .. })), "{result:?}");
}
#[tokio::test]
async fn an_unplanned_files_progress_cannot_extend_capture() {
    rejected("wrong-file").await;
}
#[tokio::test]
async fn repeated_capture_counts_are_not_progress() {
    rejected("repeat").await;
}
#[tokio::test]
async fn another_commands_progress_cannot_extend_capture() {
    rejected("wrong-command").await;
}
