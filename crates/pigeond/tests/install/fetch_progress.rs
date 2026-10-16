//! A real daemon round installs ready history while another upload-pack
//! waits. The peer wait is released and remotedaemon stopped before asserting.

use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use pigeond::daemon::{Daemon, dirs_under};
use pigeond::net::{NetConfig, Node, Reach};

#[tokio::test]
async fn a_ready_peers_file_is_installed_before_a_blocked_fetch_finishes() {
    let Some(mut l) = laptop().await else { return };
    let old = l.git.rev("refs/heads/main").await.unwrap();
    let mut tree = l.git.tree(&old).await.unwrap();
    let bytes = b"ready content must reach the folder\n";
    tree.insert(
        "ready.txt".into(),
        version(&l, "ready.txt", bytes, true).await,
    );
    let head = bens(&l, &tree, &old, 1).await;
    l.git
        .update_ref("refs/heads/main", &old, Some(&head))
        .await
        .unwrap();
    let source = l.dir.path().join("ready.git");
    std::fs::create_dir(&source).unwrap();
    let remote = Git::new(&source, GitEnv::default());
    remote.run(&["init", "--bare", "-q"]).await.unwrap();
    remote
        .run(&["fetch", "-q", l.wt.to_str().unwrap(), &head])
        .await
        .unwrap();
    remote
        .update_ref("refs/heads/main", &head, None)
        .await
        .unwrap();
    let tools = l.dir.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    let helper = tools.join("pigeon-stalled-upload-pack");
    std::fs::write(&helper, b"#!/bin/sh\n: >\"${0%/*}/started\"\nwhile [ ! -e \"${0%/*}/release\" ]; do sleep 0.01; done\nexit 1\n").unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    l.g.git.repo.env.path_first.push(tools.clone());
    for peer in [dev(2), dev(3)] {
        l.git
            .run(&[
                "config",
                "--add",
                &format!("url.{}.insteadOf", source.display()),
                &format!("p2p-annex::iroh:{}", peer.0),
            ])
            .await
            .unwrap();
    }
    l.git
        .run(&[
            "config",
            &format!("remote.{}.uploadpack", dev(3).0),
            "pigeon-stalled-upload-pack",
        ])
        .await
        .unwrap();
    let group = Arc::new(l.g);
    let node = Node::bind(&NetConfig {
        secret_key: key(1),
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: Reach::Local,
    })
    .await
    .unwrap();
    let daemon = Arc::new(Daemon {
        dirs: dirs_under(&l.dir.path().join("home")),
        config: Default::default(),
        device: dev(1).0.clone(),
        env: GitEnv::default(),
        groups: RwLock::new(vec![group.clone()]),
        node,
        net: OnceLock::new(),
        web: OnceLock::new(),
        wake: Arc::default(),
        debug: Default::default(),
        tracer: l.t,
    });
    let (round, target) = (daemon.clone(), group.clone());
    let task = tokio::spawn(async move { round.round(&target).await });
    let progressed = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if tools.join("started").exists()
                && std::fs::read(l.wt.join("ready.txt")).is_ok_and(|b| b == bytes)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok();
    let accepted = group.git.rev("refs/heads/main").await == Some(head);
    let blocked = tools.join("started").exists();
    let waiting = !task.is_finished();
    // End this fixture's round before placement tries its deliberately absent network.
    daemon.groups.write().unwrap().clear();
    std::fs::write(tools.join("release"), b"release the controlled wait\n").unwrap();
    task.await.unwrap();
    group.stop_remotedaemon().await;
    assert!(accepted, "the ready peer's signed history must be accepted");
    assert!(
        blocked,
        "the slow peer must reach its controlled upload-pack wait"
    );
    assert!(waiting, "the controlled peer must still be fetching");
    assert!(
        progressed,
        "the ready peer's file waited for the blocked fetch"
    );
}
