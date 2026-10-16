//! Shared-storage migration and crash recovery against real daemons. Hiding a
//! captured file must never become a deletion, and shared aliases never count
//! as custody. Explicit saves of hidden paths keep native preconditions.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::{net::UnixStream, process::CommandExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use pigeon_proto::bridge::{Change, Operation, Reply, Snapshot};
use pigeon_proto::{Envelope, Request, Response, encode};

struct Peer {
    child: Child,
    socket: PathBuf,
}

impl Drop for Peer {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", "--", &format!("-{}", self.child.id())])
            .stderr(Stdio::null())
            .status();
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.child.try_wait().is_ok_and(|s| s.is_none()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn start(home: &Path, socket: &Path, crash: Option<&str>) -> Peer {
    std::fs::create_dir_all(home.join("config")).unwrap();
    let config = home.join("config/config.toml");
    if !config.exists() {
        std::fs::write(&config, "notifications=false\n[network]\nbind=\"127.0.0.1:0\"\nmdns=false\nrelays=\"off\"\n[loops]\ninterval_seconds=1\n").unwrap();
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("test.log"))
        .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_pigeond"));
    command
        .arg("--home")
        .arg(home)
        .env("PIGEON_SOCKET", socket)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .process_group(0);
    if let Some(crash) = crash {
        command.env("PIGEON_LAB_CRASH", crash);
    }
    let mut peer = Peer {
        child: command.spawn().unwrap(),
        socket: socket.into(),
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    while UnixStream::connect(socket).is_err() {
        if peer.child.try_wait().unwrap().is_some() && crash.is_some() {
            return peer;
        }
        assert!(Instant::now() < deadline, "daemon did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    peer
}

fn call(peer: &Peer, request: Request) -> Response {
    let mut stream = UnixStream::connect(&peer.socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(180)))
        .unwrap();
    stream.write_all(&encode(1, &request).unwrap()).unwrap();
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0);
        assert!(line.len() <= pigeon_proto::MAX_LINE + 1);
        let reply: Envelope<Response> = serde_json::from_str(&line).unwrap();
        if !matches!(reply.body, Response::Progress { .. }) {
            return reply.body;
        }
    }
}

fn bridge(peer: &Peer, operation: Operation) -> Reply {
    match call(peer, Request::Bridge { operation }) {
        Response::Bridge { result } => result,
        other => panic!("{other:?}"),
    }
}

fn snapshot(peer: &Peer) -> Snapshot {
    match bridge(
        peer,
        Operation::Snapshot {
            cursor: None,
            limit: 1000,
        },
    ) {
        Reply::Snapshot(snapshot) => snapshot,
        other => panic!("{other:?}"),
    }
}

fn aborted(peer: &mut Peer) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while peer.child.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "hide crash point was not reached"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn existing_peer_migrates_and_hiding_recovers_before_capture() {
    let temp = tempfile::Builder::new()
        .prefix("pgnshared")
        .tempdir_in("/tmp")
        .unwrap();
    let home = temp.path().join("home");
    let socket = temp.path().join("peer.sock");
    let files = temp.path().join("files");
    std::fs::create_dir(&files).unwrap();
    std::fs::create_dir(files.join("two")).unwrap();
    for name in ["one.txt", "two/file.txt"] {
        std::fs::write(files.join(name), b"original bytes\n").unwrap();
    }
    let peer = start(&home, &socket, None);
    assert!(matches!(
        call(
            &peer,
            Request::Create {
                name: "Shared migration".into(),
                folder: files.display().to_string(),
                k: 1,
                device_name: "Managed client".into(),
            }
        ),
        Response::Said { .. }
    ));
    let before = snapshot(&peer);
    assert_eq!(before.files.len(), 2);
    let identity = match bridge(&peer, Operation::Info) {
        Reply::Info {
            device,
            annex_uuid,
            shared: false,
            ..
        } => (device, annex_uuid),
        other => panic!("{other:?}"),
    };
    drop(peer);
    // Model a pre-upgrade thinned inode without a custody/quiet-timer fixture.
    // Migration must detach it before any object is shared with the pool.
    let git = |args: &[&str]| {
        let result = Command::new("git")
            .current_dir(&files)
            .args(args)
            .output()
            .unwrap();
        assert!(result.status.success());
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    };
    let key = git(&["annex", "lookupkey", "one.txt"]);
    let object = files.join(git(&["annex", "contentlocation", &key]));
    std::fs::remove_file(files.join("one.txt")).unwrap();
    std::fs::hard_link(&object, files.join("one.txt")).unwrap();
    std::fs::set_permissions(&object, std::fs::Permissions::from_mode(0o644)).unwrap();
    let config = home.join("config/config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(&format!(
        "\n[bridge_storage]\nstore=\"{}\"\nsource_socket=\"{}\"\n",
        temp.path().join("pool").display(),
        temp.path().join("offline.sock").display()
    ));
    std::fs::write(&config, text).unwrap();
    for point in ["bridge-hide-after-flag:1", "bridge-hide-after-remove:1"] {
        let mut peer = start(&home, &socket, Some(point));
        aborted(&mut peer);
        assert!(files.join(".git/pigeon/shared-hide.json").exists());
        drop(peer);
    }
    let peer = start(&home, &socket, None);
    let after = snapshot(&peer);
    assert_eq!(
        before, after,
        "hiding must not create a deletion or new version"
    );
    assert!(!files.join("one.txt").exists());
    assert!(!files.join("two").exists());
    assert!(!files.join(".git/pigeon/shared-hide.json").exists());
    match bridge(&peer, Operation::Info) {
        Reply::Info {
            device,
            annex_uuid,
            shared: true,
            ..
        } => assert_eq!((device, annex_uuid), identity),
        other => panic!("{other:?}"),
    }
    let download = temp.path().join("download");
    assert_eq!(
        bridge(
            &peer,
            Operation::Read {
                path: "one.txt".into(),
                etag: after.files[0].etag.clone(),
                destination: download.display().to_string(),
            }
        ),
        Reply::Read
    );
    assert_eq!(std::fs::read(&download).unwrap(), b"original bytes\n");
    assert!(
        download.metadata().unwrap().nlink() >= 3,
        "downloads should pin shared bytes rather than copy them"
    );
    let upload = temp.path().join("upload");
    std::fs::write(&upload, b"new client version\n").unwrap();
    let saved = bridge(
        &peer,
        Operation::Apply {
            revision: after.revision.clone(),
            expected: Some(
                [("one.txt".into(), Some(after.files[0].etag.clone()))]
                    .into_iter()
                    .collect(),
            ),
            changes: vec![Change::Put {
                path: "one.txt".into(),
                staged: upload.display().to_string(),
            }],
        },
    );
    assert!(
        matches!(saved, Reply::Applied { suggested: 0, .. }),
        "{saved:?}"
    );
    assert!(!files.join("one.txt").exists());
    let latest = snapshot(&peer);
    assert_ne!(latest.files[0].etag, before.files[0].etag);
    assert_eq!(latest.files[1], before.files[1]);
    assert!(matches!(
        bridge(
            &peer,
            Operation::Apply {
                revision: latest.revision.clone(),
                expected: Some(
                    [("one.txt".into(), Some(before.files[0].etag.clone()))]
                        .into_iter()
                        .collect()
                ),
                changes: vec![Change::Remove {
                    path: "one.txt".into()
                }],
            }
        ),
        Reply::Failure { status: 412, .. }
    ));
    assert!(matches!(
        bridge(
            &peer,
            Operation::Apply {
                revision: latest.revision,
                expected: None,
                changes: vec![Change::Remove {
                    path: "two/file.txt".into()
                }],
            }
        ),
        Reply::Applied { .. }
    ));
    assert_eq!(
        snapshot(&peer).files.len(),
        1,
        "deleting a hidden path must still be captured"
    );
    assert!(
        matches!(call(&peer, Request::Status { path: None, not_shared: false }), Response::Status(s) if s.state != pigeon_proto::api::State::Protected)
    );
}

#[test]
fn a_bulk_delete_is_refused_before_it_can_block_journal_recovery() {
    let temp = tempfile::Builder::new()
        .prefix("pgnbulk")
        .tempdir_in("/tmp")
        .unwrap();
    let home = temp.path().join("home");
    let socket = temp.path().join("peer.sock");
    let files = temp.path().join("files");
    std::fs::create_dir(&files).unwrap();
    for i in 0..101 {
        std::fs::write(files.join(format!("file-{i:03}.txt")), b"retained bytes\n").unwrap();
    }
    let peer = start(&home, &socket, None);
    assert!(matches!(
        call(
            &peer,
            Request::Create {
                name: "Bulk deletion".into(),
                folder: files.display().to_string(),
                k: 1,
                device_name: "Managed client".into(),
            }
        ),
        Response::Said { .. }
    ));
    drop(peer);
    let config = home.join("config/config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(&format!(
        "\n[bridge_storage]\nstore=\"{}\"\nsource_socket=\"{}\"\n",
        temp.path().join("pool").display(),
        temp.path().join("offline.sock").display()
    ));
    std::fs::write(config, text).unwrap();
    let peer = start(&home, &socket, None);
    let before = snapshot(&peer);
    assert_eq!(before.files.len(), 101);
    assert!(matches!(
        bridge(
            &peer,
            Operation::Apply {
                revision: before.revision.clone(),
                expected: None,
                changes: before
                    .files
                    .iter()
                    .map(|f| Change::Remove {
                        path: f.path.clone()
                    })
                    .collect(),
            }
        ),
        Reply::Failure { status: 409, .. }
    ));
    assert!(!files.join(".git/pigeon/bridge-apply.json").exists());
    assert!(before.files.iter().all(|f| !files.join(&f.path).exists()));
    assert_eq!(snapshot(&peer), before, "a refused batch changes nothing");
    assert!(matches!(
        bridge(
            &peer,
            Operation::Apply {
                revision: before.revision,
                expected: None,
                changes: vec![Change::Remove {
                    path: before.files[0].path.clone()
                }],
            }
        ),
        Reply::Applied { .. }
    ));
    assert_eq!(snapshot(&peer).files.len(), 100);
}

#[test]
fn pool_registration_deduplicates_before_acknowledging_without_background_rounds() {
    use std::os::unix::fs::DirBuilderExt;
    let temp = tempfile::Builder::new()
        .prefix("pgnregister")
        .tempdir_in("/tmp")
        .unwrap();
    let home = temp.path().join("home");
    let files = temp.path().join("files");
    std::fs::create_dir(&files).unwrap();
    let body = b"already retained before registration\n";
    std::fs::write(files.join("original.txt"), body).unwrap();
    let peer = start(&home, &temp.path().join("peer.sock"), None);
    let created = call(
        &peer,
        Request::Create {
            name: "Register pool".into(),
            folder: files.display().to_string(),
            k: 1,
            device_name: "Provider".into(),
        },
    );
    assert!(matches!(created, Response::Said { .. }), "{created:?}");
    assert!(matches!(
        call(&peer, Request::Pause { duration: None }),
        Response::Said { .. }
    ));
    let group = match bridge(&peer, Operation::Info) {
        Reply::Info { group, .. } => group,
        other => panic!("{other:?}"),
    };
    let before = snapshot(&peer);
    let digest = &before.files[0].etag[..64];
    let store = temp.path().join("pool");
    let pool = store.join(&group).join(&digest[..2]).join(digest);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(pool.parent().unwrap())
        .unwrap();
    std::fs::write(&pool, body).unwrap();
    std::fs::set_permissions(&pool, std::fs::Permissions::from_mode(0o444)).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(&files)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    let key = git(&["annex", "lookupkey", "original.txt"]);
    let object = files.join(git(&["annex", "contentlocation", &key]));
    assert_ne!(
        object.metadata().unwrap().ino(),
        pool.metadata().unwrap().ino()
    );
    assert!(matches!(
        bridge(
            &peer,
            Operation::RegisterPool {
                group,
                store: store.display().to_string(),
            }
        ),
        Reply::Learned
    ));
    assert_eq!(
        object.metadata().unwrap().ino(),
        pool.metadata().unwrap().ino()
    );
    assert_eq!(snapshot(&peer), before);
    std::fs::write(files.join("original.txt"), b"a later in-place save\n").unwrap();
    assert_eq!(std::fs::read(&pool).unwrap(), body);
}
