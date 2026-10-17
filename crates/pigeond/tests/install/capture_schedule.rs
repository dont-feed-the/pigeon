//! Native background capture must settle actual stats, without holding up
//! another stable file. These checks use real Git, annex and elapsed time;
//! explicit capture remains available for a person's immediate request.

use super::*;
use std::time::Duration;

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
    assert!(
        pigeond::custody::present(&l.g, &key).await.is_none(),
        "observing an unsettled save ingested it through a Git filter"
    );
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
