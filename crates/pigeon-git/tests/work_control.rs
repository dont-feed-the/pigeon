//! Real process-group suspension, active deadlines and cancellation
//! (ADR 0037, 15 §15.4). A private git stand-in exposes parent and child
//! heartbeats without allowing another production command or environment.
#![allow(clippy::unwrap_used, reason = "isolated child-process fixture")]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use pigeon_git::work::{Class, Control};
use pigeon_git::{Error, GitEnv, Repo};

#[derive(Debug)]
struct Policy {
    run: AtomicBool,
    active: AtomicUsize,
}

impl Policy {
    fn new(run: bool) -> Arc<Self> {
        Arc::new(Self {
            run: AtomicBool::new(run),
            active: AtomicUsize::new(0),
        })
    }
}

impl Control for Policy {
    fn began(&self, _: Class) {
        self.active.fetch_add(1, Ordering::Relaxed);
    }
    fn ended(&self, _: Class) {
        self.active.fetch_sub(1, Ordering::Relaxed);
    }
    fn running(&self, _: Class) -> bool {
        self.run.load(Ordering::Relaxed)
    }
}

fn fixture(run: bool) -> (tempfile::TempDir, Repo, Arc<Policy>) {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("git");
    std::fs::write(
        &program,
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo metadata; exit 0; fi
echo $$ > parent.pid
(
    while :; do printf c >> child.beats; sleep 0.01; done
) &
worker=$!
echo "$worker" > child.pid
trap 'kill "$worker" 2>/dev/null; wait "$worker" 2>/dev/null; exit 143' TERM
i=0
while [ "$i" -lt 12 ] && [ ! -e finish ]; do
    printf p >> parent.beats
    sleep 0.02
    i=$((i + 1))
done
kill "$worker"
wait "$worker" 2>/dev/null
printf 'exact output'
"#,
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let repo = Repo::new(
        dir.path(),
        GitEnv {
            path_first: vec![dir.path().to_path_buf()],
            extra: Vec::new(),
        },
    );
    let policy = Policy::new(run);
    repo.set_work_control(policy.clone());
    (dir, repo, policy)
}

fn beats(root: &Path, name: &str) -> u64 {
    std::fs::metadata(root.join(name)).map_or(0, |m| m.len())
}

fn dead(root: &Path, name: &str) -> bool {
    let pid = std::fs::read_to_string(root.join(name)).unwrap();
    let status = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", pid.trim()])
        .output()
        .unwrap();
    let state = String::from_utf8_lossy(&status.stdout);
    // A killed orphan may wait briefly for the test subreaper. A stopped
    // descendant is still alive and must not satisfy this assertion.
    state.trim().is_empty() || state.trim().starts_with('Z')
}

async fn eventually(mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "child fixture did not reach its checkpoint"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn operation(
    repo: Repo,
    budget: Duration,
) -> tokio::task::JoinHandle<Result<pigeon_git::Output, Error>> {
    tokio::spawn(async move {
        repo.run(
            &[
                "annex",
                "add",
                "--no-check-gitignore",
                "--json",
                "--",
                "file",
            ],
            None,
            budget,
        )
        .await
    })
}

#[tokio::test]
async fn suspension_freezes_the_whole_group_and_spends_no_active_deadline() {
    let (dir, repo, policy) = fixture(true);
    let task = operation(repo, Duration::from_millis(600));
    eventually(|| beats(dir.path(), "parent.beats") >= 2).await;
    policy.run.store(false, Ordering::Relaxed);
    // Allow the controller's 20 ms observation and a scheduling margin.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let before = (
        beats(dir.path(), "parent.beats"),
        beats(dir.path(), "child.beats"),
    );
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(
        before,
        (
            beats(dir.path(), "parent.beats"),
            beats(dir.path(), "child.beats")
        )
    );
    assert!(
        !task.is_finished(),
        "deliberate pause consumed the operation deadline"
    );
    // Finish on the first resumed checkpoint. Twelve shell/sleep forks
    // can themselves exceed 600 ms on a loaded Mac, independently of the
    // deliberate 800 ms pause this test is meant to exclude.
    std::fs::write(dir.path().join("finish"), b"finish after resume").unwrap();
    policy.run.store(true, Ordering::Relaxed);
    assert_eq!(task.await.unwrap().unwrap().stdout, b"exact output");
    assert_eq!(policy.active.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn blocked_work_does_not_start_or_block_metadata_and_cancels_without_a_child() {
    let (dir, repo, policy) = fixture(false);
    let task = operation(repo.clone(), Duration::from_millis(50));
    eventually(|| policy.active.load(Ordering::Relaxed) == 1).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(!dir.path().join("parent.pid").exists());
    assert_eq!(
        repo.run(&["--version"], None, Duration::from_secs(1))
            .await
            .unwrap()
            .text(),
        "metadata\n"
    );
    assert!(matches!(
        repo.run(&["push"], None, Duration::from_secs(1)).await,
        Err(Error::Refused { .. })
    ));
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(policy.active.load(Ordering::Relaxed), 0);
    assert!(!dir.path().join("parent.pid").exists());
}

#[tokio::test]
async fn an_active_deadline_still_terminates_and_reaps_the_child() {
    let (dir, repo, policy) = fixture(true);
    let result = operation(repo, Duration::from_millis(60)).await.unwrap();
    assert!(matches!(result, Err(Error::Timeout { .. })));
    assert!(dir.path().join("parent.pid").exists());
    assert_eq!(policy.active.load(Ordering::Relaxed), 0);
    eventually(|| dead(dir.path(), "parent.pid") && dead(dir.path(), "child.pid")).await;
    let before = (
        beats(dir.path(), "parent.beats"),
        beats(dir.path(), "child.beats"),
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        before,
        (
            beats(dir.path(), "parent.beats"),
            beats(dir.path(), "child.beats")
        )
    );
}

#[tokio::test]
async fn cancellation_of_a_stopped_operation_leaves_no_running_descendant() {
    let (dir, repo, policy) = fixture(true);
    let task = operation(repo, Duration::from_secs(5));
    eventually(|| beats(dir.path(), "parent.beats") >= 2).await;
    policy.run.store(false, Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(100)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(policy.active.load(Ordering::Relaxed), 0);
    eventually(|| dead(dir.path(), "parent.pid") && dead(dir.path(), "child.pid")).await;
    let before = (
        beats(dir.path(), "parent.beats"),
        beats(dir.path(), "child.beats"),
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        before,
        (
            beats(dir.path(), "parent.beats"),
            beats(dir.path(), "child.beats")
        )
    );
}
