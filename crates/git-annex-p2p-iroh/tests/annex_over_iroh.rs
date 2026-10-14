//! git-annex over iroh through the helper and two `pigeond` endpoints on
//! one host (09 §9.2–9.4, ADR 0054, 09 §9.2): a laptop copies to a custodian,
//! drops its copy, gets it back byte for byte, cannot remove the custodian's,
//! and, as a custodian peer, cannot drop on the proof of a rotten copy; a
//! device outside the roster and an unknown group are refused. Needs
//! git-annex on PATH; skipped without it unless `PIGEON_REQUIRE_GIT_ANNEX`
//! is set, as in CI.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]

use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use pigeon_core::filter::Role;
use pigeon_proto::stream::Refusal;
use pigeond::net::serve::accept_loop;
use pigeond::net::socket::{Listeners, Net, bind_socket, serve_socket};
use pigeond::net::sync::AnnexSyncer;
use pigeond::net::{Admitted, NetConfig, NetError, Node, Repository, StaticDirectory};
use tokio::process::Command;

fn secret() -> [u8; 32] {
    let mut key = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut key)
        .unwrap();
    key
}

fn path_with_helper() -> String {
    let helper = PathBuf::from(env!("CARGO_BIN_EXE_git-annex-p2p-iroh"));
    let dir = helper.parent().unwrap().to_string_lossy().into_owned();
    format!("{dir}:{}", std::env::var("PATH").unwrap_or_default())
}

/// Runs a command in `dir` with the lab's git environment and `socket`.
/// `pigeond`'s own calls get pigeon's environment instead (ADR 0045), so the
/// repositories carry their identity in `.git/config`, as pigeon writes it.
async fn run(dir: &Path, socket: &Path, args: &[&str]) -> std::process::Output {
    let mut c = Command::new(args[0]);
    c.args(&args[1..])
        .current_dir(dir)
        .env("PATH", path_with_helper())
        .env("PIGEON_SOCKET", socket)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "lab")
        .env("GIT_AUTHOR_EMAIL", "lab@x")
        .env("GIT_COMMITTER_NAME", "lab")
        .env("GIT_COMMITTER_EMAIL", "lab@x")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    tokio::time::timeout(Duration::from_secs(90), c.output())
        .await
        .unwrap_or_else(|_| panic!("{args:?} timed out"))
        .unwrap()
}

async fn ok(dir: &Path, socket: &Path, args: &[&str]) -> String {
    let out = run(dir, socket, args).await;
    assert!(
        out.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

struct Device {
    node: Node,
    net: Arc<Net>,
    directory: Arc<StaticDirectory>,
    socket: PathBuf,
}

async fn device(dir: &Path, name: &str) -> Device {
    let config = NetConfig {
        secret_key: secret(),
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
    };
    let node = Node::bind(&config).await.unwrap();
    let socket = dir.join(format!("{name}-run")).join("pigeond.sock");
    let listener = bind_socket(&socket).unwrap();
    let directory = Arc::new(StaticDirectory::new());
    let net = Arc::new(Net {
        node: node.clone(),
        directory: directory.clone(),
        listeners: Listeners::default(),
        syncer: Arc::new(AnnexSyncer),
        socket_path: socket.clone(),
    });
    tokio::spawn(serve_socket(net.clone(), listener));
    tokio::spawn(accept_loop(net.clone()));
    Device {
        node,
        net,
        directory,
        socket,
    }
}

fn loopback(node: &Node) -> Vec<SocketAddr> {
    node.bound_addresses()
        .into_iter()
        .map(|a| SocketAddr::new("127.0.0.1".parse().unwrap(), a.port()))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn annex_copy_get_and_refused_remove_over_iroh() {
    if std::process::Command::new("git-annex")
        .arg("version")
        .output()
        .is_err()
    {
        assert!(
            std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none(),
            "git-annex is required"
        );
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (laptop, custodian) = (device(&root, "L").await, device(&root, "C").await);
    let (lid, cid) = (laptop.node.device_id(), custodian.node.device_id());

    // A bare custodian repository C and a laptop repository L with one file.
    let (c, l) = (root.join("C.git"), root.join("L"));
    ok(
        &root,
        &custodian.socket,
        &["git", "init", "-q", "--bare", "C.git"],
    )
    .await;
    ok(&c, &custodian.socket, &["git", "annex", "init", "C"]).await;
    for (k, v) in [("user.name", "lab"), ("user.email", "lab@x")] {
        ok(&c, &custodian.socket, &["git", "config", k, v]).await;
    }
    ok(
        &root,
        &laptop.socket,
        &["git", "init", "-q", "-b", "main", "L"],
    )
    .await;
    ok(&l, &laptop.socket, &["git", "annex", "init", "L"]).await;
    for (k, v) in [("user.name", "lab"), ("user.email", "lab@x")] {
        ok(&l, &laptop.socket, &["git", "config", k, v]).await;
    }
    let mut bytes = vec![0u8; 300_000];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut bytes)
        .unwrap();
    bytes.extend_from_slice(b"\nREMOVE k\nDATA 5\nCONNECT git-receive-pack\n");
    std::fs::write(l.join("f"), &bytes).unwrap();
    ok(&l, &laptop.socket, &["git", "annex", "add", "f"]).await;
    ok(&l, &laptop.socket, &["git", "commit", "-q", "-m", "one"]).await;
    let key = ok(&l, &laptop.socket, &["git", "annex", "lookupkey", "f"]).await;
    let c_uuid = ok(&c, &custodian.socket, &["git", "config", "annex.uuid"]).await;
    let l_uuid = ok(&l, &laptop.socket, &["git", "config", "annex.uuid"]).await;

    // C listens on iroh: the helper's address is C's device ID.
    ok(
        &c,
        &custodian.socket,
        &["git", "annex", "p2p", "--enable", "iroh"],
    )
    .await;
    let addresses = ok(
        &c,
        &custodian.socket,
        &["git", "annex", "p2p", "--gen-addresses"],
    )
    .await;
    let line = addresses
        .lines()
        .find(|l| l.starts_with("p2p-annex::iroh:"))
        .unwrap_or_else(|| panic!("an iroh address in {addresses:?}"));
    let (address, token) = line.rsplit_once(':').unwrap();
    assert_eq!(address, format!("p2p-annex::iroh:{cid}"));
    custodian.directory.add_repository(Repository {
        group: "g1".into(),
        git_dir: c.clone(),
        worktree: None,
        role: Role::Custodian,
        auth_token: Some(token.to_owned()),
        checking: false,
    });
    custodian.directory.admit_device(
        "g1",
        &lid,
        Admitted {
            annex_uuid: Some(l_uuid.clone()),
            custodian: false,
        },
    );
    laptop.directory.add_repository(Repository {
        group: "g1".into(),
        git_dir: l.join(".git"),
        worktree: Some(l.clone()),
        role: Role::Laptop,
        auth_token: None,
        checking: false,
    });
    laptop
        .directory
        .set_addresses(&cid, loopback(&custodian.node));
    let mut daemon = Command::new("git")
        .args(["annex", "remotedaemon", "--foreground"])
        .current_dir(&c)
        .env("PATH", path_with_helper())
        .env("PIGEON_SOCKET", &custodian.socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    for _ in 0..200 {
        if custodian.net.listeners.get("g1").is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        custodian.net.listeners.get("g1").is_some(),
        "remotedaemon listens"
    );

    // L stores f on C, drops its own copy (C's lock is checked), gets it back.
    ok(&l, &laptop.socket, &["git", "remote", "add", "c", address]).await;
    ok(
        &l,
        &laptop.socket,
        &["git", "config", "remote.c.annex-uuid", &c_uuid],
    )
    .await;
    ok(
        &l,
        &laptop.socket,
        &["git", "annex", "copy", "--to", "c", "f"],
    )
    .await;
    let held = ok(
        &c,
        &custodian.socket,
        &["git", "annex", "contentlocation", &key],
    )
    .await;
    assert_eq!(
        std::fs::read(c.join(&held)).unwrap(),
        bytes,
        "C holds the same bytes"
    );
    ok(&l, &laptop.socket, &["git", "annex", "drop", "f"]).await;
    let gone = run(
        &l,
        &laptop.socket,
        &["git", "annex", "contentlocation", &key],
    )
    .await;
    assert!(!gone.status.success(), "L dropped its copy");
    ok(
        &l,
        &laptop.socket,
        &["git", "annex", "get", "--from", "c", "f"],
    )
    .await;
    assert_eq!(
        std::fs::read(l.join("f")).unwrap(),
        bytes,
        "L gets the same bytes back"
    );

    // As a custodian peer, L's drop is proved only by a hashed copy on C
    // (ADR 0054): a sound copy proves it, a rotten one does not, and C's
    // fsck then moves the rotten object out.
    custodian.directory.admit_device(
        "g1",
        &lid,
        Admitted {
            annex_uuid: Some(l_uuid.clone()),
            custodian: true,
        },
    );
    ok(&l, &laptop.socket, &["git", "annex", "drop", "f"]).await;
    ok(
        &l,
        &laptop.socket,
        &["git", "annex", "get", "--from", "c", "f"],
    )
    .await;
    let object = c.join(&held);
    let mut rotten = bytes.clone();
    rotten[0] ^= 1;
    for p in [object.parent().unwrap(), &object] {
        let mut perms = std::fs::metadata(p).unwrap().permissions();
        #[allow(
            clippy::permissions_set_readonly_false,
            reason = "a test makes a copy rot"
        )]
        perms.set_readonly(false);
        std::fs::set_permissions(p, perms).unwrap();
    }
    std::fs::write(&object, &rotten).unwrap();
    let refused = run(&l, &laptop.socket, &["git", "annex", "drop", "f"]).await;
    assert!(!refused.status.success(), "a rotten copy proves no drop");
    assert_eq!(
        std::fs::read(l.join("f")).unwrap(),
        bytes,
        "L keeps its copy"
    );
    let mut moved = false;
    for _ in 0..300 {
        let out = run(
            &c,
            &custodian.socket,
            &["git", "annex", "contentlocation", &key],
        )
        .await;
        if !out.status.success() {
            moved = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    if !moved {
        let fsck = run(
            &c,
            &custodian.socket,
            &["git", "annex", "fsck", "--key", &key],
        )
        .await;
        panic!(
            "C's fsck did not move the rotten object out; by hand: {}{}",
            String::from_utf8_lossy(&fsck.stdout),
            String::from_utf8_lossy(&fsck.stderr)
        );
    }
    ok(
        &l,
        &laptop.socket,
        &["git", "annex", "copy", "--to", "c", "f"],
    )
    .await;

    // A removal from C is answered FAILURE by C's filter; C keeps its copy.
    let removed = run(
        &l,
        &laptop.socket,
        &["git", "annex", "drop", "--from", "c", "--force", "f"],
    )
    .await;
    assert!(!removed.status.success(), "a REMOVE on a custodian fails");
    ok(
        &c,
        &custodian.socket,
        &["git", "annex", "contentlocation", &key],
    )
    .await;

    // Admission: a device outside the roster, and an unknown group.
    let stranger = device(&root, "X").await;
    let refused = stranger
        .node
        .open_annex_stream(&cid, "g1", loopback(&custodian.node))
        .await;
    assert!(matches!(
        refused,
        Err(NetError::Refused(Some(Refusal::NotInRoster)))
    ));
    let refused = laptop
        .node
        .open_annex_stream(&cid, "g2", loopback(&custodian.node))
        .await;
    assert!(matches!(
        refused,
        Err(NetError::Refused(Some(Refusal::UnknownGroup)))
    ));

    drop(daemon.stdin.take());
    let _ = tokio::time::timeout(Duration::from_secs(10), daemon.wait()).await;
    for d in [&laptop, &custodian, &stranger] {
        d.node.close().await;
    }
}
