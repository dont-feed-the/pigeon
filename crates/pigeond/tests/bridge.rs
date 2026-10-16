//! Real-daemon bridge safety: a crash after physical saves replays the batch
//! before capture, stale file versions are refused, and unrelated edits can be
//! rebased without overwriting a newer version of the requested file.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use pigeon_proto::bridge::{Change, Cursor, MAX_PAGE_BYTES, MAX_PAGE_FILES, Operation, Reply};
use pigeon_proto::{Envelope, Request, Response, encode};

struct Peer {
    child: Child,
    socket: PathBuf,
}

impl Drop for Peer {
    fn drop(&mut self) {
        let group = format!("-{}", self.child.id());
        let _ = Command::new("kill")
            .args(["-TERM", "--", &group])
            .stderr(Stdio::null())
            .status();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn start(home: &Path, socket: &Path, crash: bool) -> Peer {
    std::fs::create_dir_all(home.join("config")).unwrap();
    let config = home.join("config/config.toml");
    if !config.exists() {
        std::fs::write(config,"notifications=false\n[network]\nbind=\"127.0.0.1:0\"\nmdns=false\nrelays=\"off\"\n[loops]\ninterval_seconds=1\n").unwrap();
    }
    let log = std::fs::File::create(home.join("test.log")).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_pigeond"));
    command
        .arg("--home")
        .arg(home)
        .env("PIGEON_SOCKET", socket)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .process_group(0);
    if crash {
        command.env("PIGEON_LAB_CRASH", "bridge-before-capture:1");
    }
    let child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while UnixStream::connect(socket).is_err() {
        assert!(Instant::now() < deadline, "peer did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    Peer {
        child,
        socket: socket.into(),
    }
}

fn call(peer: &Peer, request: &Request) -> std::io::Result<Response> {
    let mut stream = UnixStream::connect(&peer.socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(600)))?;
    stream.write_all(&encode(1, request).unwrap())?;
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        assert!(
            line.len() <= pigeon_proto::MAX_LINE + 1,
            "oversized socket reply"
        );
        let response: Envelope<Response> = serde_json::from_str(&line)?;
        if !matches!(response.body, Response::Progress { .. }) {
            return Ok(response.body);
        }
    }
}

#[test]
fn large_catalogues_page_over_the_real_socket_and_writes_stay_bounded() {
    let tmp = tempfile::Builder::new()
        .prefix("pgnpages")
        .tempdir_in("/tmp")
        .unwrap();
    let files = tmp.path().join("files");
    // Long portable paths cross the message limit with fewer native captures.
    // Each capture checks open writers across every visible process on the host.
    let folder = files
        .join("Catalogue")
        .join("a".repeat(230))
        .join("b".repeat(230))
        .join("c".repeat(230));
    std::fs::create_dir_all(&folder).unwrap();
    for i in 0..1200 {
        std::fs::write(
            folder.join(format!("{i:06}-{}.txt", "x".repeat(210))),
            b"same content\n",
        )
        .unwrap();
    }
    let peer = start(
        &tmp.path().join("home"),
        &tmp.path().join("peer.sock"),
        false,
    );
    assert!(matches!(
        call(
            &peer,
            &Request::Create {
                name: "Large catalogue".into(),
                folder: files.display().to_string(),
                k: 1,
                device_name: "Paged peer".into(),
                person: None,
            }
        )
        .unwrap(),
        Response::Said { .. }
    ));
    let mut cursor = None;
    let mut all = Vec::new();
    let mut first_cursor = None;
    let mut revision = String::new();
    loop {
        let result = bridge(
            &peer,
            Operation::Snapshot {
                cursor,
                limit: MAX_PAGE_FILES,
            },
        );
        assert!(
            encode(
                2,
                &Response::Bridge {
                    result: result.clone()
                }
            )
            .unwrap()
            .len()
                <= MAX_PAGE_BYTES
        );
        let Reply::Snapshot(page) = result else {
            panic!("{result:?}")
        };
        if revision.is_empty() {
            revision = page.revision.clone();
            first_cursor = page.next.clone();
        }
        assert_eq!(page.revision, revision);
        all.extend(page.files);
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(all.len(), 1200);
    assert!(all.windows(2).all(|w| w[0].path < w[1].path));
    assert!(serde_json::to_vec(&all).unwrap().len() > pigeon_proto::MAX_LINE);
    let staged = tmp.path().join("upload");
    std::fs::write(&staged, b"new version\n").unwrap();
    let changed = &all[600];
    let result = bridge(
        &peer,
        Operation::Apply {
            revision: revision.clone(),
            expected: Some(
                [(changed.path.clone(), Some(changed.etag.clone()))]
                    .into_iter()
                    .collect(),
            ),
            changes: vec![Change::Put {
                path: changed.path.clone(),
                staged: staged.display().to_string(),
            }],
        },
    );
    assert!(
        encode(
            2,
            &Response::Bridge {
                result: result.clone()
            }
        )
        .unwrap()
        .len()
            < 1024,
        "unexpected bridge write response: {result:?}"
    );
    let Reply::Applied {
        revision: updated,
        suggested: 0,
    } = result
    else {
        panic!("{result:?}")
    };
    assert_ne!(updated, revision);
    assert!(matches!(
        bridge(
            &peer,
            Operation::Snapshot {
                cursor: first_cursor,
                limit: MAX_PAGE_FILES
            }
        ),
        Reply::Failure { status: 412, .. }
    ));
    assert!(matches!(
        bridge(
            &peer,
            Operation::Snapshot {
                cursor: Some(Cursor {
                    revision: updated,
                    after: None
                }),
                limit: 1
            }
        ),
        Reply::Snapshot(_)
    ));
    assert_eq!(
        std::fs::read(files.join(&changed.path)).unwrap(),
        b"new version\n"
    );
}

fn bridge(peer: &Peer, operation: Operation) -> Reply {
    match call(peer, &Request::Bridge { operation }).unwrap() {
        Response::Bridge { result } => result,
        other => panic!("Unexpected response: {other:?}"),
    }
}

fn snapshot(peer: &Peer) -> pigeon_proto::bridge::Snapshot {
    match bridge(
        peer,
        Operation::Snapshot {
            cursor: None,
            limit: 1000,
        },
    ) {
        Reply::Snapshot(s) => s,
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_interrupted_bridge_batch_replays_and_file_preconditions_survive_rebase() {
    assert!(
        Command::new("git-annex")
            .arg("version")
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|s| s.success()),
        "git-annex is required for the bridge integration test"
    );
    let tmp = tempfile::Builder::new()
        .prefix("pgnbridge")
        .tempdir_in("/tmp")
        .unwrap();
    let root = tmp.path();
    let home = root.join("home");
    let socket = root.join("peer.sock");
    let files = root.join("files");
    std::fs::create_dir(&files).unwrap();
    let peer = start(&home, &socket, false);
    assert!(matches!(
        call(
            &peer,
            &Request::Create {
                name: "Bridge test".into(),
                folder: files.display().to_string(),
                k: 1,
                device_name: "Bridge peer".into(),
                person: None,
            }
        )
        .unwrap(),
        Response::Said { .. }
    ));
    let old = snapshot(&peer);
    drop(peer);
    let mut fault = start(&home, &socket, true);
    let staged = root.join("upload");
    std::fs::write(&staged, b"a complete upload\n").unwrap();
    let changes = ["one.txt", "two.txt"]
        .into_iter()
        .map(|path| Change::Put {
            path: path.into(),
            staged: staged.display().to_string(),
        })
        .collect();
    assert!(
        call(
            &fault,
            &Request::Bridge {
                operation: Operation::Apply {
                    revision: old.revision.clone(),
                    expected: None,
                    changes
                }
            }
        )
        .is_err()
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while fault.child.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "crash injection did not stop the daemon"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(files.join(".git/pigeon/bridge-apply.json").exists());
    drop(fault);
    let peer = start(&home, &socket, false);
    let recovered = snapshot(&peer);
    assert_eq!(recovered.files.len(), 2);
    assert_eq!(
        std::fs::read(files.join("one.txt")).unwrap(),
        b"a complete upload\n"
    );
    assert!(!files.join(".git/pigeon/bridge-apply.json").exists());

    // An old whole-tree revision can safely add a different, still-absent slot.
    let result = bridge(
        &peer,
        Operation::Apply {
            revision: old.revision.clone(),
            expected: Some([("three.txt".into(), None)].into_iter().collect()),
            changes: vec![Change::Put {
                path: "three.txt".into(),
                staged: staged.display().to_string(),
            }],
        },
    );
    assert!(matches!(result, Reply::Applied { .. }), "{result:?}");
    std::fs::write(&staged, b"a newer upload\n").unwrap();
    let result = bridge(
        &peer,
        Operation::Apply {
            revision: recovered.revision.clone(),
            expected: Some(
                [("one.txt".into(), Some("obsolete".into()))]
                    .into_iter()
                    .collect(),
            ),
            changes: vec![Change::Put {
                path: "one.txt".into(),
                staged: staged.display().to_string(),
            }],
        },
    );
    assert!(
        matches!(result, Reply::Failure { status: 412, .. }),
        "{result:?}"
    );
    assert_eq!(
        std::fs::read(files.join("one.txt")).unwrap(),
        b"a complete upload\n"
    );
    let destination = root.join("download");
    let result = bridge(
        &peer,
        Operation::Read {
            path: "one.txt".into(),
            etag: recovered
                .files
                .iter()
                .find(|f| f.path == "one.txt")
                .unwrap()
                .etag
                .clone(),
            destination: destination.display().to_string(),
        },
    );
    assert_eq!(result, Reply::Read);
    assert_eq!(std::fs::read(destination).unwrap(), b"a complete upload\n");
}
