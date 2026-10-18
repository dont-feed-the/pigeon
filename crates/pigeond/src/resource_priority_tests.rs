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

async fn pending_batch_keeps_priority(cancel: bool) {
    let controller = Arc::new(Controller::default());
    let (low_dir, low_repo) = fixture(&controller, "scrubbing-between-keys");
    let (high_dir, high_repo) = fixture(&controller, "pending-protection");
    std::fs::write(
        high_dir.path().join("git"),
        "#!/bin/sh\nwhile IFS= read -r key; do\n printf '{\"command\":\"copy\",\"success\":true,\"key\":\"%s\"}\\n' \"$key\"\ndone\n",
    )
    .unwrap();
    let git = crate::git::Git {
        repo: high_repo,
        purge_gate: Arc::new(crate::purge_gate::Gate::default()),
    };
    let mut pool = crate::custody_transfer::Pool::default();
    pool.copy(&git, "fixture-peer", "first", Duration::from_secs(3))
        .await
        .unwrap();
    // The first response is complete; the same protection queue still has
    // another key to revalidate. Scrub must not run in this gap.
    let low = operation(low_repo.clone(), "fsck", Duration::from_secs(2));
    tokio::time::sleep(Duration::from_millis(2200)).await;
    assert_eq!(
        beats(low_dir.path()),
        0,
        "scrub ran between protection keys"
    );
    assert!(
        !low.is_finished(),
        "pending protection spent scrub's deadline"
    );
    assert_eq!(
        low_repo
            .run(&["--version"], None, Duration::from_secs(2))
            .await
            .unwrap()
            .stdout,
        b"metadata"
    );
    pool.copy(&git, "fixture-peer", "second", Duration::from_secs(3))
        .await
        .unwrap();
    if cancel {
        drop(pool);
    } else {
        pool.stop().await;
        // A stopped, still-live pool must release the completed queue too.
        eventually(|| beats(low_dir.path()) >= 2).await;
        drop(pool);
    }
    eventually(|| beats(low_dir.path()) >= 2).await;
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
async fn pending_protection_between_batch_keys_keeps_scrub_waiting_until_stop() {
    pending_batch_keeps_priority(false).await;
}

#[tokio::test]
async fn canceling_pending_protection_between_batch_keys_releases_scrub() {
    pending_batch_keeps_priority(true).await;
}

#[tokio::test]
async fn a_pending_batch_uses_its_download_view_and_releases_on_stop() {
    let controller = Arc::new(Controller::default());
    let (dir, repo) = fixture(&controller, "optional-between-keys");
    std::fs::write(
        dir.path().join("git"),
        "#!/bin/sh\nwhile IFS= read -r key; do\n printf '{\"success\":true,\"key\":\"%s\"}\\n' \"$key\"\ndone\n",
    ).unwrap();
    let git = crate::git::Git {
        repo: repo.for_download(),
        purge_gate: Arc::new(crate::purge_gate::Gate::default()),
    };
    let mut pool = crate::custody_transfer::Pool::default();
    pool.get(&git, "kept", Duration::from_secs(3))
        .await
        .unwrap();
    let other = controller.for_group("protection".into());
    assert_eq!(
        controller.children[Class::Transfer as usize].load(Ordering::Relaxed),
        0
    );
    assert_eq!(
        controller.children[Class::Download as usize].load(Ordering::Relaxed),
        1
    );
    assert!(other.running(Class::Transfer));
    assert!(!other.running(Class::Scrub));
    pool.stop().await;
    assert!(other.running(Class::Scrub));
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
    // Reusing the stopped pool starts a fresh, independently scoped queue.
    pool.get(&git, "later", Duration::from_secs(3))
        .await
        .unwrap();
    assert_eq!(
        controller.children[Class::Download as usize].load(Ordering::Relaxed),
        1
    );
    drop(pool);
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
}

#[tokio::test]
async fn capture_releases_a_same_repository_transfer_lock() {
    let controller = Arc::new(Controller::default());
    let (dir, repo) = fixture(&controller, "same-repository");
    std::fs::write(
        dir.path().join("git"),
        r#"#!/bin/sh
if [ "$2" = get ]; then
    exec 9>repository.lock
    flock -x 9
    touch locked
    trap 'exit 0' TERM
    while :; do sleep 0.01; done
else
    exec 9>repository.lock
    flock -x 9
    printf captured
fi
"#,
    )
    .unwrap();
    let lower_repo = repo.for_download();
    let lower = tokio::spawn(async move {
        lower_repo
            .run(
                &["annex", "get", "--key", "fixture-key"],
                None,
                Duration::from_secs(10),
            )
            .await
    });
    eventually(|| dir.path().join("locked").exists()).await;
    let result = repo
        .run(
            &[
                "annex",
                "add",
                "--no-check-gitignore",
                "--json",
                "--",
                "file",
            ],
            None,
            Duration::from_secs(1),
        )
        .await;
    if !matches!(&result, Ok(out) if out.stdout == b"captured") {
        lower.abort();
        let _ = lower.await;
        panic!("capture was blocked by a suspended lock owner: {result:?}");
    }
    let lower = tokio::time::timeout(Duration::from_secs(2), lower)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(lower, Err(pigeon_git::Error::Pipe { source, .. })
        if source.kind() == std::io::ErrorKind::Interrupted));
    assert!(controller.lock().captures.is_empty());
    assert!(
        controller
            .children
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
    );
}

#[test]
fn capture_cancels_only_its_repository_until_the_last_claim_ends() {
    let controller = Arc::new(Controller::default());
    let same = controller.for_group("same".into());
    let other = controller.for_group("other".into());
    same.began(Class::Capture);
    same.began(Class::Capture);
    for class in [Class::Transfer, Class::Download, Class::Scrub] {
        assert!(same.cancel_requested(class));
        assert!(!other.cancel_requested(class));
    }
    assert!(!same.cancel_requested(Class::Capture));
    same.ended(Class::Capture);
    assert!(same.cancel_requested(Class::Transfer));
    same.ended(Class::Capture);
    assert!(!same.cancel_requested(Class::Transfer));
    assert!(controller.lock().captures.is_empty());
}
