//! Real status requests must not bypass background settling (15 §15.5).
//! No round task runs in this fixture, so the request is the first stat
//! observation regardless of host load or the request's wall-clock duration.
#![allow(clippy::unwrap_used, reason = "native status API regression")]

use std::sync::{Arc, OnceLock, RwLock};

use pigeon_proto::{Request, Response};

#[tokio::test]
async fn status_observes_a_new_save_without_capturing_it_immediately() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("files")).unwrap();
    let wt = temp.path().join("files").canonicalize().unwrap();
    let env = pigeon_git::GitEnv::default();
    let git = crate::git::Git::new(&wt, env.clone());
    if git.run(&["annex", "version", "--raw"]).await.is_err() {
        assert!(std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none());
        return;
    }
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "Status fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "status@pigeon"])
        .await
        .unwrap();
    git.run(&["annex", "init", "status"]).await.unwrap();
    let g = Arc::new(
        crate::group::Group::open(&"a".repeat(40), &wt, &env)
            .await
            .unwrap(),
    );
    crate::settings::check(&g).await;
    let node = crate::net::Node::bind(&crate::net::NetConfig {
        secret_key: [42; 32],
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: crate::net::Reach::Local,
    })
    .await
    .unwrap();
    let dirs = pigeon_sys::paths::Dirs {
        config: temp.path().join("config"),
        state: temp.path().join("state"),
        cache: temp.path().join("cache"),
        runtime: temp.path().join("run"),
    };
    let device = node.device_id();
    let tracer = crate::trace::Tracer::new(dirs.traces(), &device);
    std::fs::write(wt.join("before.txt"), b"already captured\n").unwrap();
    crate::capture::capture(&g, &device, &tracer).await.unwrap();
    let before = git.rev("refs/heads/main").await.unwrap();
    let daemon = Arc::new(crate::daemon::Daemon {
        admissions: crate::admission_round::Store::open(dirs.state.join("admission.toml")),
        dirs,
        config: crate::config::Config::default(),
        device: device.clone(),
        env,
        groups: RwLock::new(vec![g.clone()]),
        node,
        net: OnceLock::new(),
        web: OnceLock::new(),
        wake: Arc::new(tokio::sync::Notify::new()),
        debug: crate::debug::Permissions::default(),
        tracer,
    });
    let fresh = wt.join("fresh.txt");
    std::fs::write(&fresh, b"a closed, freshly saved file\n").unwrap();
    let answer = super::handle(
        &daemon,
        Request::Status {
            path: Some(fresh.to_string_lossy().into_owned()),
            not_shared: false,
        },
    )
    .await;
    assert!(
        answer.iter().any(|r| matches!(r, Response::Status(_))),
        "{answer:?}"
    );
    assert_eq!(
        git.rev("refs/heads/main").await.as_deref(),
        Some(before.as_str()),
        "status must not turn its first observation into an immediate capture"
    );
    assert!(!git.index().await.unwrap().contains_key("fresh.txt"));
    tokio::time::timeout(std::time::Duration::from_secs(1), daemon.wake.notified())
        .await
        .unwrap();
    // This is a working capture fixture: an explicit data-changing request
    // can still preserve the new bytes immediately.
    crate::capture::capture(&g, &device, &daemon.tracer)
        .await
        .unwrap();
    assert!(git.index().await.unwrap().contains_key("fresh.txt"));
    daemon.node.close().await;
}
