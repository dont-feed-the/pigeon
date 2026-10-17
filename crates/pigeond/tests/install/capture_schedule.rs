//! Native background capture must settle actual stats, without holding up
//! another stable file. These checks use real Git, annex and elapsed time;
//! explicit capture remains available for a person's immediate request.

use super::*;
use std::time::Duration;

#[tokio::test]
async fn preserved_timestamps_do_not_hide_new_suggestion_bytes_or_mode() {
    use std::os::unix::fs::PermissionsExt;
    let Some(l) = laptop().await else { return };
    let mut doc = l.g.roster().unwrap();
    doc.roster
        .persons
        .get_mut(&PersonId("ana".into()))
        .unwrap()
        .admin = false;
    let base = pigeond::change::commit_roster(&l.g, &doc, "retire Ana", &l.t)
        .await
        .unwrap();
    let mut tree = l.git.tree(&base).await.unwrap();
    tree.insert(
        "foreign.txt".into(),
        version(&l, "foreign.txt", b"Ben's version\n", true).await,
    );
    bens(&l, &tree, &base, 60).await;
    install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    l.g.lock().installed.clear();
    write(&l, "foreign.txt", b"private one\n");
    assert_eq!(
        capture::capture(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .suggested,
        1
    );
    let old = pigeond::conflicts_file::read(&l.g).unwrap().paths["foreign.txt"].clone();
    let file = l.wt.join("foreign.txt");
    let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
    write(&l, "foreign.txt", b"private two\n");
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert_eq!(
        capture::capture(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .suggested,
        1
    );
    let new = pigeond::conflicts_file::read(&l.g).unwrap().paths["foreign.txt"].clone();
    assert_ne!(new.mine, old.mine);
    let object = pigeond::custody::present(&l.g, new.mine.as_deref().unwrap())
        .await
        .unwrap();
    assert_eq!(std::fs::read(object).unwrap(), b"private two\n");
    let object = pigeond::custody::present(&l.g, old.mine.as_deref().unwrap())
        .await
        .unwrap();
    assert_eq!(std::fs::read(object).unwrap(), b"private one\n");
    // Rebuilding a missing observation cache must not stack an empty proposal.
    let refs = pigeond::proposal::kept(&l.git).await;
    std::fs::remove_file(l.g.git_dir.join("pigeon/capture-verified.json")).unwrap();
    let reopened = Group::open(&l.g.id, &l.wt, &GitEnv::default())
        .await
        .unwrap();
    assert_eq!(
        capture::capture(&reopened, &dev(1).0, &l.t)
            .await
            .unwrap()
            .suggested,
        0
    );
    assert_eq!(pigeond::proposal::kept(&l.git).await, refs);
    assert_eq!(
        l.git.index().await.unwrap()["foreign.txt"],
        tree["foreign.txt"]
    );
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        capture::capture(&reopened, &dev(1).0, &l.t)
            .await
            .unwrap()
            .suggested,
        1
    );
    let d = pigeond::conflicts_file::read(&reopened).unwrap().paths["foreign.txt"].clone();
    let tree = l.git.tree(d.proposal.as_deref().unwrap()).await.unwrap();
    assert_eq!(tree["foreign.txt"].mode, "100755");
}

#[tokio::test]
async fn verifying_unchanged_foreign_bytes_never_creates_an_empty_suggestion() {
    let Some(l) = laptop().await else { return };
    let mut doc = l.g.roster().unwrap();
    doc.roster
        .persons
        .get_mut(&PersonId("ana".into()))
        .unwrap()
        .admin = false;
    let base = pigeond::change::commit_roster(&l.g, &doc, "retire Ana", &l.t)
        .await
        .unwrap();
    let mut tree = l.git.tree(&base).await.unwrap();
    tree.insert(
        "foreign.txt".into(),
        version(&l, "foreign.txt", b"Ben's version\n", true).await,
    );
    bens(&l, &tree, &base, 60).await;
    assert_eq!(
        install::incoming(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .written,
        1
    );
    // No native stat cache is assumed. A restart with no private observation
    // must verify the actual bytes instead of proposing a nonexistent edit.
    std::fs::remove_file(l.g.git_dir.join("pigeon/capture-verified.json")).unwrap();
    let reopened = Group::open(&l.g.id, &l.wt, &GitEnv::default())
        .await
        .unwrap();
    let done = capture::capture(&reopened, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(done.suggested, 0);
    assert!(done.commit.is_none());
    assert!(pigeond::proposal::kept(&l.git).await.is_empty());
    assert!(
        pigeond::conflicts_file::read(&reopened)
            .unwrap()
            .paths
            .is_empty()
    );
    assert_eq!(read(&l, "foreign.txt"), "Ben's version\n");
}

#[tokio::test]
async fn exact_observations_survive_restart_but_not_a_preserved_mtime_save() {
    let Some(l) = laptop().await else { return };
    write(&l, "restart.txt", b"old contents\n");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let reopened = Group::open(&l.g.id, &l.wt, &GitEnv::default())
        .await
        .unwrap();
    assert_eq!(
        capture::background(&reopened, &dev(1).0, &l.t)
            .await
            .unwrap()
            .busy,
        0
    );
    let file = l.wt.join("restart.txt");
    let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
    write(&l, "restart.txt", b"new contents\n");
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    let reopened = Group::open(&l.g.id, &l.wt, &GitEnv::default())
        .await
        .unwrap();
    assert_eq!(
        capture::background(&reopened, &dev(1).0, &l.t)
            .await
            .unwrap()
            .busy,
        1
    );
}

#[tokio::test]
async fn malformed_and_future_observation_caches_do_not_mark_a_file_clean() {
    let Some(l) = laptop().await else { return };
    write(&l, "cache.txt", b"captured contents\n");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    // A valid native Git stat cache is an independent clean observation.
    // Clear that cache while preserving the exact pointer, so only the
    // malformed private cache could incorrectly justify skipping this file.
    let index = l.git.index().await.unwrap();
    let entry = &index["cache.txt"];
    let info = format!("{},{},cache.txt", entry.mode, entry.oid);
    l.git
        .run(&["update-index", "--cacheinfo", &info])
        .await
        .unwrap();
    let path = l.g.git_dir.join("pigeon/capture-verified.json");
    for bytes in [b"not json".as_slice(), b"{\"format\":999,\"paths\":{}}"] {
        std::fs::write(&path, bytes).unwrap();
        let reopened = Group::open(&l.g.id, &l.wt, &GitEnv::default())
            .await
            .unwrap();
        assert_eq!(
            capture::background(&reopened, &dev(1).0, &l.t)
                .await
                .unwrap()
                .busy,
            1
        );
    }
}

#[tokio::test]
async fn explicit_captures_also_heat_later_background_work_without_blocking_other_files() {
    let Some(l) = laptop().await else { return };
    for i in 0..7 {
        write(&l, "hot.txt", format!("captured save {i}\n").as_bytes());
        assert!(
            capture::capture(&l.g, &dev(1).0, &l.t)
                .await
                .unwrap()
                .commit
                .is_some()
        );
    }
    write(&l, "hot.txt", b"the eighth save waits\n");
    write(&l, "other.txt", b"unrelated stable save\n");
    assert_eq!(
        capture::background(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .busy,
        2
    );
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    let done = capture::background(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(done.busy, 1);
    assert_eq!(done.added, 1);
    let keys = l.git.keys(&done.commit.unwrap()).await.unwrap();
    let object = pigeond::custody::present(&l.g, &keys["hot.txt"])
        .await
        .unwrap();
    assert_eq!(std::fs::read(object).unwrap(), b"captured save 6\n");
    assert!(keys.contains_key("other.txt"));
    assert_eq!(read(&l, "hot.txt"), "the eighth save waits\n");
}

#[tokio::test]
async fn explicit_capture_reads_a_same_size_save_with_preserved_mtime() {
    let Some(l) = laptop().await else { return };
    write(&l, "preserved.txt", b"old contents\n");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let file = l.wt.join("preserved.txt");
    let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
    write(&l, "preserved.txt", b"new contents\n");
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    let done = capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let keys = l.git.keys(&done.commit.unwrap()).await.unwrap();
    let object = pigeond::custody::present(&l.g, &keys["preserved.txt"])
        .await
        .unwrap();
    assert_eq!(std::fs::read(object).unwrap(), b"new contents\n");
}

#[tokio::test]
async fn native_capture_batch_keeps_literal_names_and_leaves_unportable_ones_held() {
    let Some(l) = laptop().await else { return };
    let names = ["-capture.txt", "two words.txt", "café.txt"];
    write(&l, "line\nbreak.txt", b"unportable name stays here\n");
    for name in names {
        write(&l, name, format!("literal file {name}\n").as_bytes());
    }
    let done = capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let keys = l.git.keys(&done.commit.unwrap()).await.unwrap();
    assert!(!keys.contains_key("line\nbreak.txt"));
    assert!(
        pigeond::held_file::read(&l.g)
            .names()
            .contains("line\nbreak.txt")
    );
    assert_eq!(read(&l, "line\nbreak.txt"), "unportable name stays here\n");
    for name in names {
        let object = pigeond::custody::present(&l.g, &keys[name]).await.unwrap();
        assert_eq!(
            std::fs::read(object).unwrap(),
            format!("literal file {name}\n").as_bytes()
        );
    }
}

#[tokio::test]
async fn preserving_mtime_cannot_make_a_same_size_save_look_captured() {
    let Some(l) = laptop().await else { return };
    write(&l, "preserved.txt", b"old contents\n");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let file = l.wt.join("preserved.txt");
    let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
    assert_eq!(
        capture::background(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .busy,
        0,
        "the verified captured file must start settled"
    );
    write(&l, "preserved.txt", b"new contents\n");
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert_eq!(
        pigeond::status::report(&l.g, &dev(1).0, "", false)
            .await
            .state,
        pigeon_proto::api::State::Local,
        "ctime must invalidate a cached capture even when size and mtime agree"
    );
    assert_eq!(
        capture::background(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .busy,
        1
    );
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    let done = capture::background(&l.g, &dev(1).0, &l.t).await.unwrap();
    let keys = l.git.keys(&done.commit.unwrap()).await.unwrap();
    let object = pigeond::custody::present(&l.g, &keys["preserved.txt"])
        .await
        .unwrap();
    assert_eq!(std::fs::read(object).unwrap(), b"new contents\n");
}

#[tokio::test]
async fn background_observation_does_not_ingest_a_save_before_its_quiet_period() {
    use sha2::{Digest, Sha256};
    let Some(l) = laptop().await else { return };
    let original = b"the initial captured version\n";
    write(&l, "observed.txt", original);
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let bytes = b"the changed captured version\n";
    assert_eq!(
        original.len(),
        bytes.len(),
        "exercise same-size clean-filter comparison"
    );
    let key = format!(
        "SHA256E-s{}--{}.txt",
        bytes.len(),
        pigeond::util::hex(&Sha256::digest(bytes))
    );
    write(&l, "observed.txt", bytes);
    assert!(pigeond::custody::present(&l.g, &key).await.is_none());
    let waiting = capture::background(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(waiting.busy, 1);
    assert!(waiting.commit.is_none());
    let report = pigeond::status::report(&l.g, &dev(1).0, "", false).await;
    assert_eq!(report.state, pigeon_proto::api::State::Local);
    assert!(
        pigeond::custody::present(&l.g, &key).await.is_none(),
        "a status observation ingested an unsettled save through a Git filter"
    );
}

#[tokio::test]
async fn installing_another_path_cannot_ingest_an_unsettled_save() {
    use sha2::{Digest, Sha256};
    let Some(l) = laptop().await else { return };
    write(&l, "settling.txt", b"old contents\n");
    let base = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&base).await.unwrap();
    tree.insert(
        "incoming.txt".into(),
        version(&l, "incoming.txt", b"incoming bytes\n", true).await,
    );
    bens(&l, &tree, &base, 60).await;
    let bytes = b"new contents\n";
    let key = format!(
        "SHA256E-s{}--{}.txt",
        bytes.len(),
        pigeond::util::hex(&Sha256::digest(bytes))
    );
    write(&l, "settling.txt", bytes);
    assert_eq!(
        capture::background(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .busy,
        1
    );
    assert!(pigeond::custody::present(&l.g, &key).await.is_none());
    pigeond::evict::recover(&l.g, &l.t).await;
    assert!(
        pigeond::custody::present(&l.g, &key).await.is_none(),
        "an idle recovery round must not refresh and ingest unsettled files"
    );
    assert_eq!(
        install::incoming(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .written,
        1
    );
    assert_eq!(read(&l, "incoming.txt"), "incoming bytes\n");
    assert!(
        pigeond::custody::present(&l.g, &key).await.is_none(),
        "install's global dirty observation ingested another path before it settled"
    );
    assert_eq!(std::fs::read(l.wt.join("settling.txt")).unwrap(), bytes);
}

#[tokio::test]
async fn background_capture_restarts_settling_and_captures_an_independent_stable_file() {
    let Some(l) = laptop().await else { return };
    write(&l, "changing.txt", b"first save\n");
    let original = l.git.rev("refs/heads/main").await.unwrap();
    let first = capture::background(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(first.busy, 1);
    assert!(first.commit.is_none());

    write(&l, "changing.txt", b"a different second save\n");
    write(&l, "stable.txt", b"this independent save is finished\n");
    let second = capture::background(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(second.busy, 2);
    assert!(second.commit.is_none());
    assert_eq!(
        l.git.rev("refs/heads/main").await.as_deref(),
        Some(original.as_str())
    );

    tokio::time::sleep(Duration::from_millis(2_100)).await;
    write(
        &l,
        "changing.txt",
        b"the final save has a new fingerprint\n",
    );
    let third = capture::background(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(third.busy, 1);
    assert_eq!(third.added, 1);
    let main = third.commit.unwrap();
    let keys = l.git.keys(&main).await.unwrap();
    assert!(keys.contains_key("stable.txt"));
    assert!(!keys.contains_key("changing.txt"));

    tokio::time::sleep(Duration::from_millis(2_100)).await;
    let last = capture::background(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(last.busy, 0);
    assert_eq!(last.added, 1);
    assert!(last.commit.is_some());
    let key = l.git.keys(&last.commit.unwrap()).await.unwrap()["changing.txt"].clone();
    let object = pigeond::custody::present(&l.g, &key).await.unwrap();
    assert_eq!(
        std::fs::read(object).unwrap(),
        b"the final save has a new fingerprint\n"
    );
}

#[tokio::test]
async fn an_explicit_capture_can_save_a_file_before_the_background_quiet_period() {
    let Some(l) = laptop().await else { return };
    write(&l, "immediate.txt", b"an explicit request\n");
    assert_eq!(
        capture::background(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .busy,
        1
    );
    let explicit = capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(explicit.added, 1);
    assert!(explicit.commit.is_some());
    assert!(
        capture::background(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .commit
            .is_none()
    );
}

#[tokio::test]
async fn settling_preserves_a_save_observed_soon_after_install() {
    let Some(l) = laptop().await else { return };
    write(&l, "a.txt", b"old application version\n");
    let first = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&first).await.unwrap();
    let theirs = version(&l, "a.txt", b"new incoming version\n", true).await;
    tree.insert("a.txt".into(), theirs.clone());
    bens(&l, &tree, &first, 60).await;
    assert_eq!(
        install::incoming(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .written,
        1
    );
    l.g.lock().installed.get_mut("a.txt").unwrap().0 = pigeond::util::now_ns();
    write(&l, "a.txt", b"old application's later save\n");
    std::fs::File::options()
        .write(true)
        .open(l.wt.join("a.txt"))
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(86_400))
        .unwrap();
    assert_eq!(
        capture::background(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .busy,
        1
    );
    // The ordinary install hint can expire during a large or hot-file
    // wait. Classification already observed for this stat must survive it.
    l.g.lock().installed.clear();
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    let done = capture::background(&l.g, &dev(1).0, &l.t).await.unwrap();
    let tree = l.git.tree(&done.commit.unwrap()).await.unwrap();
    assert_eq!(tree.get("a.txt"), Some(&theirs));
    let copies: Vec<_> = tree
        .keys()
        .filter(|p| pigeon_core::names::is_copy_of(p, "a.txt"))
        .collect();
    assert_eq!(copies.len(), 1, "{tree:?}");
    let key = l
        .git
        .keys(&l.git.rev("refs/heads/main").await.unwrap())
        .await
        .unwrap()[copies[0]]
        .clone();
    let object = pigeond::custody::present(&l.g, &key).await.unwrap();
    assert_eq!(
        std::fs::read(object).unwrap(),
        b"old application's later save\n"
    );
}

#[tokio::test]
async fn a_late_save_cannot_look_concurrent_by_preserving_the_install_mtime() {
    let Some(l) = laptop().await else { return };
    write(&l, "a.txt", b"original version\n");
    let first = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&first).await.unwrap();
    let theirs = version(&l, "a.txt", b"incoming version\n", true).await;
    tree.insert("a.txt".into(), theirs.clone());
    bens(&l, &tree, &first, 60).await;
    assert_eq!(
        install::incoming(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .written,
        1
    );
    let hint = pigeond::util::now_ns() - 45_000_000_000;
    l.g.lock().installed.get_mut("a.txt").unwrap().0 = hint;
    write(&l, "a.txt", b"a later independent edit\n");
    std::fs::File::options()
        .write(true)
        .open(l.wt.join("a.txt"))
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + Duration::from_nanos(u64::try_from(hint).unwrap()))
        .unwrap();
    let done = capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let tree = l.git.tree(&done.commit.unwrap()).await.unwrap();
    assert_ne!(tree.get("a.txt"), Some(&theirs));
    assert!(
        !tree
            .keys()
            .any(|path| pigeon_core::names::is_copy_of(path, "a.txt"))
    );
    let keys = l
        .git
        .keys(&l.git.rev("refs/heads/main").await.unwrap())
        .await
        .unwrap();
    let object = pigeond::custody::present(&l.g, &keys["a.txt"])
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(object).unwrap(),
        b"a later independent edit\n"
    );
}

#[tokio::test]
async fn actual_successful_background_captures_heat_only_the_changed_path() {
    let Some(l) = laptop().await else { return };
    for i in 0..7 {
        write(
            &l,
            "hot.txt",
            format!("real captured save {i}\n").as_bytes(),
        );
        assert_eq!(
            capture::background(&l.g, &dev(1).0, &l.t)
                .await
                .unwrap()
                .busy,
            1
        );
        tokio::time::sleep(Duration::from_millis(2_100)).await;
        assert!(
            capture::background(&l.g, &dev(1).0, &l.t)
                .await
                .unwrap()
                .commit
                .is_some()
        );
    }
    write(
        &l,
        "hot.txt",
        b"eighth save waits for the hot-path interval\n",
    );
    write(&l, "other.txt", b"an unrelated finished file\n");
    assert_eq!(
        capture::background(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .busy,
        2
    );
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    let done = capture::background(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(done.busy, 1);
    assert_eq!(done.added, 1);
    let keys = l.git.keys(&done.commit.unwrap()).await.unwrap();
    let object = pigeond::custody::present(&l.g, &keys["hot.txt"])
        .await
        .unwrap();
    assert_eq!(std::fs::read(object).unwrap(), b"real captured save 6\n");
    assert!(keys.contains_key("other.txt"));
    assert_eq!(
        std::fs::read(l.wt.join("hot.txt")).unwrap(),
        b"eighth save waits for the hot-path interval\n"
    );
}
