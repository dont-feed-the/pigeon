//! The running configuration loop applies valid folder priorities, preserves
//! equal snapshots and refuses an invalid complete edit without a restart.

use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use super::{Priorities, Priority};
use crate::daemon::{Daemon, dirs_under};
use crate::net::{NetConfig, Node, Reach};

struct Running(tokio::task::JoinHandle<()>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
#[allow(
    clippy::unwrap_used,
    reason = "bounded native configuration-loop fixture"
)]
async fn live_reload_preserves_equal_and_invalid_readings_then_applies_the_next_valid_edit() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("files");
    std::fs::create_dir(&repository).unwrap();
    let env = pigeon_git::GitEnv::default();
    crate::git::Git::new(&repository, env.clone())
        .run(&["init", "-q"])
        .await
        .unwrap();
    let group = Arc::new(
        crate::group::Group::open("fixture", &repository, &env)
            .await
            .unwrap(),
    );
    let dirs = dirs_under(&temp.path().join("home"));
    std::fs::create_dir_all(&dirs.config).unwrap();
    let node = Node::bind(&NetConfig {
        secret_key: [71; 32],
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: Reach::Local,
    })
    .await
    .unwrap();
    let daemon = Arc::new(Daemon {
        dirs,
        config: Default::default(),
        device: node.device_id(),
        env,
        groups: RwLock::new(vec![group.clone()]),
        node,
        net: OnceLock::new(),
        web: OnceLock::new(),
        wake: Arc::default(),
        debug: Default::default(),
        admissions: crate::admission_round::Store::open(temp.path().join("admission.toml")),
        tracer: crate::trace::Tracer::new(temp.path().join("traces"), "fixture"),
    });
    let config = daemon.dirs.config_file();
    let edit = |value: &str| {
        let body = format!(
            "format = 1\n[[groups]]\nid = 'fixture'\npath = '/unused'\n[groups.priority]\nPhotos = '{value}'\n"
        );
        pigeon_sys::fs::write_atomic(&config, body.as_bytes()).unwrap();
    };
    edit("high");
    let running = Running(tokio::spawn(crate::resource_control::run(daemon.clone())));
    let expected: Priorities = [("Photos".into(), Priority::High)].into();
    let high_applied = tokio::time::timeout(Duration::from_secs(5), async {
        while *group.lock().download_priorities != expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok();
    let snapshot = group.lock().download_priorities.clone();
    // Cross a full ordinary two-second reload without changing the reading.
    tokio::time::sleep(Duration::from_millis(2300)).await;
    let equal_preserved = Arc::ptr_eq(&snapshot, &group.lock().download_priorities);
    edit("urgent");
    tokio::time::sleep(Duration::from_millis(2300)).await;
    let invalid_preserved = Arc::ptr_eq(&snapshot, &group.lock().download_priorities);
    edit("low");
    let low_applied = tokio::time::timeout(Duration::from_secs(5), async {
        while group.lock().download_priorities.get("Photos") != Some(&Priority::Low) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok();
    drop(running);
    daemon.node.close().await;
    assert!(
        high_applied,
        "the initial complete reading was never applied"
    );
    assert!(
        equal_preserved,
        "an equal reload interrupted the queue snapshot"
    );
    assert!(
        invalid_preserved,
        "an invalid complete reading replaced the last valid one"
    );
    assert!(low_applied, "a later valid edit needed a restart");
}
