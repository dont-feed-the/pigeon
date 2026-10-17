//! Higher-class pending work in one group must suspend lower work in
//! another, and ending it must not leave a stale admission claim.
#![allow(clippy::unwrap_used, reason = "isolated native child-process fixtures")]

use crate::resource_control::Controller;
use pigeon_git::work::Class;
use pigeon_git::{GitEnv, Repo};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[test]
fn pending_capture_preempts_other_groups_transfer_and_scrub() {
    let controller = Arc::new(Controller::default());
    let capture = controller.for_group("saving".into());
    let other = controller.for_group("receiving".into());
    assert!(other.running(Class::Transfer));
    assert!(other.running(Class::Scrub));
    capture.began(Class::Capture);
    assert!(capture.running(Class::Capture));
    assert!(!other.running(Class::Transfer));
    assert!(!other.running(Class::Scrub));
    assert_eq!(
        controller.waiting(),
        Some(pigeon_core::resources::Wait::Saving)
    );
    capture.ended(Class::Capture);
    assert!(other.running(Class::Transfer));
    assert!(other.running(Class::Scrub));
    assert_eq!(controller.waiting(), None);
}

fn fixture(controller: &Arc<Controller>, group: &str) -> (tempfile::TempDir, Repo) {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("git");
    std::fs::write(
        &program,
        r#"#!/bin/sh
if [ "$1" = --version ]; then printf metadata; exit 0; fi
if [ "$1" = annex ] && [ "$2" = contentlocation ]; then exit 1; fi
echo $$ > parent.pid
while [ ! -e finish ]; do
    printf x >> beats
    sleep 0.01
done
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
    repo.set_work_control(controller.for_group(group.to_owned()));
    (dir, repo)
}

fn beats(root: &Path) -> u64 {
    std::fs::metadata(root.join("beats")).map_or(0, |m| m.len())
}

async fn eventually(mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "native priority checkpoint not reached"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn operation(
    repo: Repo,
    command: &'static str,
    budget: Duration,
) -> tokio::task::JoinHandle<Result<pigeon_git::Output, pigeon_git::Error>> {
    tokio::spawn(async move {
        let args: &[&str] = if command == "add" {
            &[
                "annex",
                "add",
                "--no-check-gitignore",
                "--json",
                "--",
                "file",
            ]
        } else {
            &["annex", command, "--key", "fixture-key"]
        };
        let result = repo.run(args, None, budget).await;
        assert!(result.is_ok(), "native fixture command failed: {result:?}");
        result
    })
}

async fn preempts(lower: &'static str, higher: &'static str) {
    let controller = Arc::new(Controller::default());
    let (low_dir, low_repo) = fixture(&controller, "lower-group");
    let (high_dir, high_repo) = fixture(&controller, "higher-group");
    let low = operation(low_repo.clone(), lower, Duration::from_secs(2));
    eventually(|| beats(low_dir.path()) >= 2).await;
    let high = operation(high_repo, higher, Duration::from_secs(10));
    eventually(|| beats(high_dir.path()) >= 2).await;
    // Let the ordinary 20 ms policy poll and signal delivery settle.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let before = beats(low_dir.path());
    let high_before = beats(high_dir.path());
    tokio::time::sleep(Duration::from_millis(2200)).await;
    assert_eq!(
        beats(low_dir.path()),
        before,
        "lower-class child kept running"
    );
    assert!(
        beats(high_dir.path()) > high_before,
        "higher-class child stopped"
    );
    assert!(
        !low.is_finished(),
        "priority wait consumed the active deadline"
    );
    assert_eq!(
        low_repo
            .run(&["--version"], None, Duration::from_secs(3))
            .await
            .unwrap()
            .stdout,
        b"metadata"
    );
    std::fs::write(high_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(high.await.unwrap().unwrap().stdout, b"exact output");
    std::fs::write(low_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(low.await.unwrap().unwrap().stdout, b"exact output");
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
}

#[tokio::test]
async fn capture_suspends_another_groups_transfer_without_spending_its_deadline() {
    preempts("get", "add").await;
}

#[tokio::test]
async fn transfer_suspends_another_groups_scrub_without_spending_its_deadline() {
    preempts("fsck", "get").await;
}

#[tokio::test]
async fn canceling_the_higher_child_releases_lower_work() {
    let controller = Arc::new(Controller::default());
    let (low_dir, low_repo) = fixture(&controller, "scrubbing");
    let (high_dir, high_repo) = fixture(&controller, "saving");
    let low = operation(low_repo, "fsck", Duration::from_secs(3));
    eventually(|| beats(low_dir.path()) >= 2).await;
    let high = operation(high_repo, "add", Duration::from_secs(10));
    eventually(|| beats(high_dir.path()) >= 2).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let before = beats(low_dir.path());
    high.abort();
    assert!(high.await.unwrap_err().is_cancelled());
    eventually(|| beats(low_dir.path()) > before).await;
    std::fs::write(low_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(low.await.unwrap().unwrap().stdout, b"exact output");
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
}

#[tokio::test]
async fn same_class_children_do_not_block_themselves_or_each_other() {
    let controller = Arc::new(Controller::default());
    let (a_dir, a_repo) = fixture(&controller, "one");
    let (b_dir, b_repo) = fixture(&controller, "two");
    let a = operation(a_repo, "get", Duration::from_secs(3));
    let b = operation(b_repo, "get", Duration::from_secs(3));
    eventually(|| beats(a_dir.path()) >= 2 && beats(b_dir.path()) >= 2).await;
    for dir in [&a_dir, &b_dir] {
        std::fs::write(dir.path().join("finish"), b"done").unwrap();
    }
    assert_eq!(a.await.unwrap().unwrap().stdout, b"exact output");
    assert_eq!(b.await.unwrap().unwrap().stdout, b"exact output");
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
}

#[tokio::test]
async fn serving_side_repair_waits_for_power_and_other_groups_capture() {
    use crate::net::sync::{AnnexSyncer, Syncer};
    use pigeon_core::resources::Conditions;
    let controller = Arc::new(Controller::default());
    controller.observe(
        Conditions {
            battery: Some(1),
            ..Default::default()
        },
        false,
    );
    let (dir, repo) = fixture(&controller, "repairing");
    let repository = crate::net::Repository {
        group: "repairing".into(),
        git_dir: dir.path().join(".git"),
        worktree: Some(dir.path().to_owned()),
        role: pigeon_core::filter::Role::Laptop,
        auth_token: None,
        checking: false,
    };
    let syncer = AnnexSyncer::new(controller.clone(), repo.env.clone());
    let repair = tokio::spawn(syncer.repair(&repository, "fixture-key"));
    eventually(|| {
        controller.children[Class::Transfer as usize].load(Ordering::Relaxed) == 1
            || beats(dir.path()) > 0
    })
    .await;
    assert_eq!(beats(dir.path()), 0, "repair started despite low battery");
    let capture = controller.for_group("saving".into());
    capture.began(Class::Capture);
    controller.observe(Conditions::default(), false);
    assert!(
        !controller
            .for_repair_group("repairing".into())
            .running(Class::Scrub)
    );
    assert_eq!(
        repo.run(&["--version"], None, Duration::from_secs(3))
            .await
            .unwrap()
            .stdout,
        b"metadata"
    );
    assert_eq!(
        beats(dir.path()),
        0,
        "repair bypassed another group's capture"
    );
    capture.ended(Class::Capture);
    eventually(|| beats(dir.path()) >= 2).await;
    std::fs::write(dir.path().join("finish"), b"done").unwrap();
    tokio::time::timeout(Duration::from_secs(5), repair)
        .await
        .unwrap()
        .unwrap();
    controller.observe(
        Conditions {
            battery: Some(1),
            ..Default::default()
        },
        false,
    );
    let queued = tokio::spawn(syncer.repair(&repository, "fixture-key"));
    eventually(|| controller.children[Class::Transfer as usize].load(Ordering::Relaxed) == 1).await;
    queued.abort();
    assert!(queued.await.unwrap_err().is_cancelled());
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
}

#[tokio::test]
async fn serving_side_repair_does_not_wait_behind_ongoing_backup_transfers() {
    use crate::net::sync::{AnnexSyncer, Syncer};
    let controller = Arc::new(Controller::default());
    let content = controller.for_group("backing-up".into());
    content.began(Class::Transfer);
    let (dir, repo) = fixture(&controller, "repairing");
    let repository = crate::net::Repository {
        group: "repairing".into(),
        git_dir: dir.path().join(".git"),
        worktree: Some(dir.path().to_owned()),
        role: pigeon_core::filter::Role::Laptop,
        auth_token: None,
        checking: false,
    };
    let syncer = AnnexSyncer::new(controller.clone(), repo.env.clone());
    let repair = tokio::spawn(syncer.repair(&repository, "fixture-key"));
    let progressed = tokio::time::timeout(Duration::from_secs(2), async {
        while beats(dir.path()) < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok();
    // Always release/join the original native operation before asserting,
    // including the old scheduler's denied-launch reproduction.
    content.ended(Class::Transfer);
    std::fs::write(dir.path().join("finish"), b"done").unwrap();
    tokio::time::timeout(Duration::from_secs(5), repair)
        .await
        .unwrap()
        .unwrap();
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
    assert!(
        progressed,
        "repair was classified as routine scrub behind backup work"
    );
}

#[tokio::test]
async fn protection_holds_the_next_kept_key_without_spending_its_deadline() {
    let controller = Arc::new(Controller::default());
    let (protect_dir, protect_repo) = fixture(&controller, "protecting");
    let (download_dir, download_repo) = fixture(&controller, "keeping");
    let high = operation(protect_repo, "get", Duration::from_secs(10));
    eventually(|| beats(protect_dir.path()) >= 2).await;
    let low = operation(download_repo.for_download(), "get", Duration::from_secs(2));
    eventually(|| controller.children[Class::Download as usize].load(Ordering::Relaxed) == 1).await;
    tokio::time::sleep(Duration::from_millis(2200)).await;
    assert_eq!(
        beats(download_dir.path()),
        0,
        "kept key started ahead of protection"
    );
    assert!(
        !low.is_finished(),
        "admission wait consumed the active deadline"
    );
    assert_eq!(
        download_repo
            .run(&["--version"], None, Duration::from_secs(2))
            .await
            .unwrap()
            .stdout,
        b"metadata"
    );
    std::fs::write(protect_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(high.await.unwrap().unwrap().stdout, b"exact output");
    eventually(|| beats(download_dir.path()) >= 2).await;
    std::fs::write(download_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(low.await.unwrap().unwrap().stdout, b"exact output");
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
}

#[tokio::test]
async fn an_active_kept_key_finishes_but_the_next_yields_to_protection() {
    let controller = Arc::new(Controller::default());
    let (download_dir, original_repo) = fixture(&controller, "keeping");
    let download_repo = original_repo.for_download();
    let low = operation(download_repo.clone(), "get", Duration::from_secs(10));
    eventually(|| beats(download_dir.path()) >= 2).await;
    assert_eq!(
        controller.children[Class::Download as usize].load(Ordering::Relaxed),
        1
    );
    let (protect_dir, protect_repo) = fixture(&controller, "protecting");
    let high = operation(protect_repo, "get", Duration::from_secs(10));
    eventually(|| beats(protect_dir.path()) >= 2).await;
    let before = beats(download_dir.path());
    eventually(|| beats(download_dir.path()) > before + 2).await;
    std::fs::write(download_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(low.await.unwrap().unwrap().stdout, b"exact output");
    std::fs::remove_file(download_dir.path().join("finish")).unwrap();
    std::fs::remove_file(download_dir.path().join("beats")).unwrap();
    let next = operation(download_repo, "get", Duration::from_secs(3));
    eventually(|| controller.children[Class::Download as usize].load(Ordering::Relaxed) == 1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(beats(download_dir.path()), 0);
    // The original repository keeps protection priority: it can run beside
    // the other protection operation while its scoped download waits.
    let original = operation(original_repo, "get", Duration::from_secs(3));
    eventually(|| beats(download_dir.path()) >= 2).await;
    std::fs::write(download_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(original.await.unwrap().unwrap().stdout, b"exact output");
    std::fs::write(protect_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(high.await.unwrap().unwrap().stdout, b"exact output");
    assert_eq!(next.await.unwrap().unwrap().stdout, b"exact output");
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
}

#[tokio::test]
async fn a_scoped_download_batch_waits_before_starting_its_key() {
    let controller = Arc::new(Controller::default());
    let (protect_dir, protect_repo) = fixture(&controller, "protecting");
    let (download_dir, repo) = fixture(&controller, "keeping");
    let high = operation(protect_repo, "get", Duration::from_secs(10));
    eventually(|| beats(protect_dir.path()) >= 2).await;
    let mut batch = repo
        .for_download()
        .batch(&["annex", "get", "--batch-keys", "--json"])
        .unwrap();
    let low = tokio::spawn(async move {
        let result = batch.call("fixture-key", Duration::from_secs(2)).await;
        batch.stop(Duration::from_secs(1)).await;
        result
    });
    eventually(|| controller.children[Class::Download as usize].load(Ordering::Relaxed) == 1).await;
    tokio::time::sleep(Duration::from_millis(2200)).await;
    assert_eq!(beats(download_dir.path()), 0);
    assert!(!low.is_finished());
    std::fs::write(protect_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(high.await.unwrap().unwrap().stdout, b"exact output");
    eventually(|| beats(download_dir.path()) >= 2).await;
    std::fs::write(download_dir.path().join("finish"), b"done").unwrap();
    assert_eq!(low.await.unwrap().unwrap(), "exact output");
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
}
