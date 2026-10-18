//! A real resource reload requests native modes only after complete validation.
//! This does not start a supervisor or measure CPU, disk or HDD budgets.

use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use crate::daemon::{Daemon, dirs_under};
use crate::net::{NetConfig, Node, Reach};

struct Running(tokio::task::JoinHandle<()>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn observed(path: &Path, bytes: &[u8]) -> bool {
    tokio::time::timeout(Duration::from_secs(5), async {
        while std::fs::read(path).ok().as_deref() != Some(bytes) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok()
}

#[tokio::test]
#[allow(clippy::unwrap_used, reason = "bounded resource reload fixture")]
async fn live_reload_preserves_equal_and_invalid_modes_then_requests_fast_and_background() {
    let temp = tempfile::tempdir().unwrap();
    let dirs = dirs_under(&temp.path().join("home"));
    std::fs::create_dir_all(&dirs.config).unwrap();
    let node = Node::bind(&NetConfig {
        secret_key: [74; 32],
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
        env: Default::default(),
        groups: RwLock::new(Vec::new()),
        node,
        net: OnceLock::new(),
        web: OnceLock::new(),
        wake: Arc::default(),
        debug: Default::default(),
        admissions: crate::admission_round::Store::open(temp.path().join("admission.toml")),
        tracer: crate::trace::Tracer::new(temp.path().join("traces"), "fixture"),
    });
    let config = daemon.dirs.config_file();
    let hint = daemon.dirs.state.join("resource-priority");
    let edit = |body: &str| {
        pigeon_sys::fs::write_atomic(&config, body.as_bytes()).unwrap();
    };
    edit("[resources]\npreset = 'gentle'\n");
    let running = Running(tokio::spawn(crate::resource_control::run(daemon.clone())));
    let gentle = observed(&hint, b"1 background\n").await;
    let original = std::fs::metadata(&hint).map(|s| s.ino()).ok();
    edit("[resources]\npreset = 'balanced'\n");
    tokio::time::sleep(Duration::from_millis(2300)).await;
    let balanced_kept = std::fs::metadata(&hint).map(|s| s.ino()).ok() == original;
    edit("[resources]\npreset = 'fast'\non_metered = 'invalid'\n");
    tokio::time::sleep(Duration::from_millis(2300)).await;
    let invalid_kept = std::fs::metadata(&hint).map(|s| s.ino()).ok() == original;
    edit("[resources]\npreset = 'fast'\n");
    let fast = observed(&hint, b"1 normal\n").await;
    edit("[resources]\npreset = 'gentle'\n");
    let restored = observed(&hint, b"1 background\n").await;
    drop(running);
    daemon.node.close().await;
    assert!(gentle && balanced_kept && invalid_kept && fast && restored);
}
