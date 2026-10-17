//! A controlled cold location probe must not own the worktree lock.
//! A real signed capture completes before that probe is released; no
//! timeout widening or skipped transfer can satisfy the regression.

use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use pigeond::daemon::{Daemon, dirs_under};
use pigeond::net::{NetConfig, Node, Reach};

#[tokio::test]
async fn bounded_location_batches_match_real_keys_across_the_batch_boundary() {
    let Some(l) = laptop().await else { return };
    for i in 0..67 {
        write(
            &l,
            &format!("file-{i}.txt"),
            format!("unique bytes {i}\n").as_bytes(),
        );
    }
    let captured = capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(captured.added, 67);
    let main = l.git.rev("refs/heads/main").await.unwrap();
    let mut keys = l
        .git
        .keys(&main)
        .await
        .unwrap()
        .into_values()
        .collect::<std::collections::BTreeSet<_>>();
    let unknown = format!("SHA256E-s1--{}.txt", "0".repeat(64));
    keys.insert(unknown.clone());
    let locations = pigeond::custody::holders_for(&l.git, &keys).await;
    assert_eq!(locations.len(), 68);
    assert!(locations[&unknown].is_empty());
    let objects = pigeond::custody::present_for(&l.git, &keys).await;
    assert_eq!(objects.len(), 67);
    assert!(!objects.contains_key(&unknown));
    let own = l.git.config("annex.uuid").await.unwrap();
    for key in keys.iter().filter(|key| **key != unknown) {
        assert!(
            locations[key].contains(&own),
            "the actual local object lost its location: {key}"
        );
        assert!(objects[key].is_file());
    }
}

#[tokio::test]
async fn a_cold_location_probe_leaves_capture_available_before_it_finishes() {
    let Some(mut l) = laptop_fixture(true).await else {
        return;
    };
    write(&l, "first.txt", b"already captured\n");
    assert!(
        capture::capture(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .commit
            .is_some()
    );
    let tools = l.dir.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    // Keep the real git path before adding our private wrapper to PATH.
    let real = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    assert!(real.status.success());
    let real = String::from_utf8(real.stdout).unwrap();
    assert!(real.trim().starts_with('/'));
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = annex ] && [ \"$2\" = copy ]; then : >\"${{0%/*}}/copy-started\"; exit 1; fi\nif [ \"$1\" = annex ] && [ \"$2\" = whereis ]; then\n : >\"${{0%/*}}/started\"\n while [ ! -e \"${{0%/*}}/release\" ]; do sleep 0.01; done\nfi\nexec '{}' \"$@\"\n",
        real.trim().replace('\'', "'\\''")
    );
    let helper = tools.join("git");
    std::fs::write(&helper, script).unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    l.g.git.repo.env.path_first.insert(0, tools.clone());
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
        admissions: pigeond::admission_round::Store::open(l.dir.path().join("admission-acks.toml")),
        tracer: l.t,
    });
    let worker_group = group.clone();
    let worker_daemon = Arc::downgrade(&daemon);
    let task = tokio::spawn(async move {
        pigeond::custody::place_worker(&worker_group, &dev(1).0, &worker_daemon).await
    });
    let blocked = tokio::time::timeout(Duration::from_secs(10), async {
        while !tools.join("started").exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok();
    let progressed = if blocked {
        tokio::time::timeout(Duration::from_secs(10), async {
            let _busy = group.busy.lock().await;
            std::fs::write(l.wt.join("later.txt"), b"captured while the probe waits\n").unwrap();
            capture::capture(&group, &dev(1).0, &daemon.tracer)
                .await
                .unwrap()
                .commit
                .is_some()
        })
        .await
        .unwrap_or(false)
    } else {
        false
    };
    let waiting = !task.is_finished();
    std::fs::write(tools.join("release"), b"released").unwrap();
    let completed = tokio::time::timeout(Duration::from_secs(10), task).await;
    daemon.node.close().await;
    assert!(
        completed.is_ok(),
        "the changed plan did not finish after the probe was released"
    );
    assert_eq!(completed.unwrap().unwrap(), 0);
    assert!(
        !tools.join("copy-started").exists(),
        "a stale protection plan began a transfer after a new capture"
    );
    assert!(
        blocked,
        "the cold location probe never reached its controlled wait"
    );
    assert!(
        waiting,
        "the probe must remain blocked until the later capture completes"
    );
    assert!(
        progressed,
        "a cold location probe prevented capture of a later save"
    );
}
