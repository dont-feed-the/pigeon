//! A blocked real Git fetch must not hold back accepted history from a
//! reachable peer. The slow upload-pack is released before any assertion.

use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn a_ready_peers_history_advances_while_another_peer_is_still_fetching() {
    let mut w = world().await;
    pigeond::git::set_signer(key(3));
    let source = tempfile::tempdir().unwrap();
    let remote = Git::new(source.path(), GitEnv::default());
    remote.run(&["init", "--bare", "-q"]).await.unwrap();
    remote
        .run(&[
            "fetch",
            "-q",
            w.git.repo.dir.to_str().unwrap(),
            "+refs/heads/main:refs/heads/main",
        ])
        .await
        .unwrap();
    let mut tree = w.base.clone();
    tree.insert("Shared/a.txt".into(), pointer(&remote, 7).await);
    let head = commit(&remote, &tree, &[&w.genesis], 1, 1).await;
    remote
        .update_ref("refs/heads/main", &head, Some(&w.genesis))
        .await
        .unwrap();
    let tools = w._dir.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    let helper = tools.join("pigeon-stalled-upload-pack");
    std::fs::write(&helper, b"#!/bin/sh\n: >\"${0%/*}/started\"\nwhile [ ! -e \"${0%/*}/release\" ]; do sleep 0.01; done\nexit 1\n").unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    w.g.git.repo.env.path_first.push(tools.clone());
    let peers = vec![dev(1).0, dev(2).0];
    for peer in &peers {
        w.g.git
            .run(&["remote", "add", peer, source.path().to_str().unwrap()])
            .await
            .unwrap();
    }
    w.g.git
        .run(&[
            "config",
            &format!("remote.{}.uploadpack", peers[1]),
            "pigeon-stalled-upload-pack",
        ])
        .await
        .unwrap();
    let group = Arc::new(w.g);
    let runner = group.clone();
    let tracer = pigeond::trace::Tracer::new(w._dir.path().join("traces"), &dev(3).0);
    let task = tokio::spawn(async move {
        pigeond::custody::fetch_each(&runner, &peers, |peers| {
            let (group, tracer) = (&runner, &tracer);
            async move {
                let _busy = group.busy.lock().await;
                pigeond::custody::accept(group, peers, tracer).await;
            }
        })
        .await
    });
    let progressed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if tools.join("started").exists()
                && group.git.rev("refs/heads/main").await.as_deref() == Some(head.as_str())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok();
    let still_fetching = !task.is_finished();
    std::fs::write(tools.join("release"), b"release the controlled stall\n").unwrap();
    let fetched = task.await.unwrap();
    assert!(progressed, "ready peer history waited for the blocked peer");
    assert!(still_fetching, "the fixture must retain its blocked fetch");
    assert_eq!(fetched, vec![dev(1).0]);
    assert!(group.lock().engine.accepted(&head));
}
