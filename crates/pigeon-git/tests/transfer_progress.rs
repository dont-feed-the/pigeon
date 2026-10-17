//! Keyed annex progress renews inactivity time, not authority or success.
//! Controlled native children prove progress, stalls, key isolation and
//! unchanged final JSON for both one-shot and batch transfers (ADR 0037).
#![allow(clippy::unwrap_used, reason = "isolated native progress fixtures")]

use pigeon_git::annex::Answer;
use pigeon_git::{Error, GitEnv, Repo};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_millis(800);

fn fixture(mode: &str) -> (tempfile::TempDir, Repo) {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("git");
    std::fs::write(&script, r#"#!/bin/sh
progress=no
batch=no
command=$2
for arg in "$@"; do
    [ "$arg" != --json-progress ] || progress=yes
    [ "$arg" != --batch-keys ] || batch=yes
done
one() {
    key=$1
    i=1
    while [ "$i" -le 8 ]; do
        value=$i
        reported=$key
        [ "$PIGEON_PROGRESS_MODE" != stale ] || value=1
        [ "$PIGEON_PROGRESS_MODE" != wrong-key ] || reported=other-key
        if [ "$progress" = yes ]; then
            printf '{"byte-progress":%s,"total-size":8,"action":{"command":"%s","key":"%s"}}\n' "$value" "$command" "$reported"
        fi
        sleep 0.2
        i=$((i + 1))
        if [ "$PIGEON_PROGRESS_MODE" = stall ] && [ "$i" -gt 4 ]; then sleep 10; fi
    done
    printf '{"command":"%s","key":"%s","success":true}\n' "$command" "$key"
}
if [ "$batch" = yes ]; then
    while IFS= read -r key; do one "$key"; done
else
    one fixture-key
fi
"#).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let repo = Repo::new(
        dir.path(),
        GitEnv {
            path_first: vec![dir.path().to_path_buf()],
            extra: vec![("PIGEON_PROGRESS_MODE".into(), mode.into())],
        },
    );
    (dir, repo)
}

#[tokio::test]
async fn moving_one_shot_bytes_outlive_the_inactivity_limit_and_keep_final_json() {
    let (_dir, repo) = fixture("moving");
    let started = Instant::now();
    let output = repo
        .run(
            &["annex", "get", "--json", "--key", "fixture-key"],
            None,
            LIMIT,
        )
        .await
        .unwrap();
    assert!(started.elapsed() > LIMIT);
    let answer: Answer = serde_json::from_slice(&output.stdout).unwrap();
    assert!(answer.success && answer.key.as_deref() == Some("fixture-key"));
    assert!(!output.text().contains("byte-progress"));
}

#[tokio::test]
async fn moving_batch_keys_outlive_the_limit_and_reuse_the_same_child() {
    let (_dir, repo) = fixture("moving");
    let mut batch = repo
        .batch(&["annex", "get", "--batch-keys", "--json"])
        .unwrap();
    for key in ["fixture-key", "next-key"] {
        let answer: Answer = batch.call_json(key, LIMIT).await.unwrap().unwrap();
        assert!(answer.success && answer.key.as_deref() == Some(key));
    }
    assert_eq!(batch.starts, 1);
    batch.stop(Duration::from_secs(1)).await;
}

async fn times_out(mode: &str) {
    let (_dir, repo) = fixture(mode);
    let started = Instant::now();
    let result = repo
        .run(
            &["annex", "get", "--json", "--key", "fixture-key"],
            None,
            LIMIT,
        )
        .await;
    assert!(matches!(result, Err(Error::Timeout { .. })), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn another_keys_progress_cannot_extend_this_transfer() {
    times_out("wrong-key").await;
}
#[tokio::test]
async fn repeated_byte_counts_are_not_progress() {
    times_out("stale").await;
}
#[tokio::test]
async fn a_transfer_still_times_out_when_actual_progress_stops() {
    times_out("stall").await;
}

#[tokio::test]
async fn a_scrub_keeps_its_ordinary_active_time_limit() {
    let (_dir, repo) = fixture("moving");
    let result = repo
        .run(&["annex", "fsck", "--key", "fixture-key"], None, LIMIT)
        .await;
    assert!(matches!(result, Err(Error::Timeout { .. })), "{result:?}");
}

#[tokio::test]
async fn moving_copy_bytes_also_keep_their_inactivity_budget() {
    let (_dir, repo) = fixture("moving");
    let output = repo
        .run(
            &[
                "annex",
                "copy",
                "--json",
                "--key",
                "fixture-key",
                "--to",
                "backup",
            ],
            None,
            LIMIT,
        )
        .await
        .unwrap();
    let answer: Answer = serde_json::from_slice(&output.stdout).unwrap();
    assert!(answer.success && answer.key.as_deref() == Some("fixture-key"));
    assert!(!output.text().contains("byte-progress"));
}
