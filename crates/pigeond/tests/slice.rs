//! The safety slice on one host (13 §13.12): three `pigeond`, a laptop
//! and two backup machines, each with its own home and lab fingerprints, so
//! they count as distinct failure domains. Driven through the `pigeon` CLI
//! with `--json`: a file is local after `create`, published once one backup
//! machine joined, protected once the second did; `free` hides it, and
//! `keep` gives back the same bytes.
//!
//! Needs git-annex on PATH, and `pigeon` and `git-annex-p2p-iroh` built next
//! to `pigeond` (`cargo build -p pigeon -p git-annex-p2p-iroh`). Skipped
//! without git-annex unless `PIGEON_REQUIRE_GIT_ANNEX` is set, as in CI.
#![cfg(feature = "lab")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
#![allow(clippy::print_stderr, reason = "tests")]

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use pigeon_proto::api::{State, StatusReport};
use pigeon_proto::{Arg, Response};

/// One running `pigeond`, killed with its process group on drop.
struct Daemon {
    name: &'static str,
    home: PathBuf,
    socket: PathBuf,
    child: Child,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        // SIGTERM first: `pigeond` then stops remotedaemon's process group,
        // which a SIGKILL would leave behind.
        let pid = self.child.id().to_string();
        let _ = Command::new("kill").args(["-TERM", &pid]).status();
        let deadline = Instant::now() + Duration::from_secs(10);
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        let group = format!("-{pid}");
        let _ = Command::new("kill").args(["-KILL", "--", &group]).status();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn bin(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_BIN_EXE_pigeond")).parent().unwrap();
    let path = dir.join(name);
    assert!(
        path.exists(),
        "{} is missing: cargo build -p pigeon -p git-annex-p2p-iroh",
        path.display()
    );
    path
}

fn git_env<'a>(cmd: &'a mut Command, home: &Path) -> &'a mut Command {
    cmd.env("HOME", home)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
}

fn start(root: &Path, sockets: &Path, name: &'static str) -> Daemon {
    let home = root.join(name);
    std::fs::create_dir_all(home.join("config")).unwrap();
    let config =
        "[network]\nbind = \"127.0.0.1:0\"\nmdns = false\n\n[loops]\ninterval_seconds = 1\n";
    std::fs::write(home.join("config/config.toml"), config).unwrap();
    let fp = home.join("fingerprints.json");
    let ids = format!(r#"{{"storage":["disk-{name}"],"host":"host-{name}","virtual":null}}"#);
    std::fs::write(&fp, ids).unwrap();
    let socket = sockets.join(format!("{name}.sock"));
    let log = std::fs::File::create(home.join("pigeond.out")).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pigeond"));
    cmd.arg("--home")
        .arg(&home)
        .env("PIGEON_SOCKET", &socket)
        .env("PIGEON_LAB_FINGERPRINTS", &fp)
        .env("PIGEON_LOG", "debug")
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .process_group(0);
    let child = git_env(&mut cmd, &home).spawn().unwrap();
    let d = Daemon {
        name,
        home,
        socket,
        child,
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while !d.socket.exists() {
        assert!(
            Instant::now() < deadline,
            "{name} did not start\n{}",
            logs(&[&d])
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    d
}

/// Each daemon's own log lines (not its libraries') and its output, for a
/// failure's message.
fn logs(all: &[&Daemon]) -> String {
    let mut out = String::new();
    for d in all {
        out.push_str(&format!("--- {}\n", d.name));
        let dir = d.home.join("state/logs");
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            let lines: Vec<&str> = text
                .lines()
                .filter(|l| l.contains("\"target\":\"pigeond") || l.contains("\"WARN\""))
                .collect();
            for l in lines.iter().rev().take(80).rev() {
                out.push_str(l);
                out.push('\n');
            }
        }
        let output = std::fs::read_to_string(d.home.join("pigeond.out")).unwrap_or_default();
        out.push_str(&output);
    }
    out
}

/// Runs `pigeon --json args…` against `d`; its responses.
fn pigeon(d: &Daemon, args: &[&str]) -> Vec<Response> {
    let began = Instant::now();
    let mut cmd = Command::new(bin("pigeon"));
    cmd.arg("--json")
        .args(args)
        .env("PIGEON_SOCKET", &d.socket)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = git_env(&mut cmd, &d.home).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("pigeon {args:?} did not answer");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    eprintln!("{} pigeon {args:?}: {:?}", d.name, began.elapsed());
    // The result is on stdout, problems and progress on stderr (10 §10.2).
    let mut all: Vec<Response> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect();
    let err = String::from_utf8_lossy(&out.stderr);
    all.extend(err.lines().filter_map(|l| serde_json::from_str(l).ok()));
    all
}

/// The text argument `name` of the message `id` that `rs` say.
fn said(rs: &[Response], id: &str, name: &str) -> Option<String> {
    rs.iter().find_map(|r| match r {
        Response::Said { message } if message.id == id => match message.args.get(name) {
            Some(Arg::Text(t)) => Some(t.clone()),
            Some(Arg::Number(n)) => Some(n.to_string()),
            None => Some(String::new()),
        },
        _ => None,
    })
}

fn status(d: &Daemon) -> StatusReport {
    match pigeon(d, &["status"]).as_slice() {
        [Response::Status(s)] => s.clone(),
        other => panic!("status: {other:?}"),
    }
}

/// Polls the laptop's status until its state is `want`.
fn until(lap: &Daemon, want: State, all: &[&Daemon]) -> StatusReport {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let s = status(lap);
        if s.state == want {
            return s;
        }
        if Instant::now() > deadline {
            panic!("never {want:?}; last {s:?}\n{}", logs(all));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn join(lap: &Daemon, c: &Daemon, all: &[&Daemon]) {
    let answer = pigeon(lap, &["invite", "--backup-machine", c.name]);
    let code = said(&answer, "invitation", "code").unwrap_or_else(|| panic!("{answer:?}"));
    let joined = pigeon(c, &["join", "--backup-machine", &code]);
    assert!(
        said(&joined, "joined", "group").is_some(),
        "{joined:?}\n{}",
        logs(all)
    );
}

fn annex_present() -> bool {
    Command::new("git")
        .args(["annex", "version", "--raw"])
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn local_published_protected_then_free_and_keep() {
    if !annex_present() {
        assert!(
            std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none(),
            "git-annex is required"
        );
        eprintln!("skipped: no git-annex");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    // Sockets in a short private directory: a unix socket's path holds about
    // 100 bytes, and macOS's temporary directory is long.
    let sockets = tempfile::Builder::new()
        .prefix("pgn")
        .tempdir_in("/tmp")
        .unwrap();
    let s = sockets.path();
    let (lap, c1, c2) = (
        start(&root, s, "lap"),
        start(&root, s, "c1"),
        start(&root, s, "c2"),
    );
    let all = [&lap, &c1, &c2];

    let folder = root.join("folder");
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    let big: Vec<u8> = (0u32..300_000)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    std::fs::write(folder.join("sub/big.bin"), &big).unwrap();
    std::fs::write(folder.join("a.txt"), b"hello\n").unwrap();
    let f = folder.to_str().unwrap();
    let created = pigeon(&lap, &["create", "Family", f, "--k", "2", "--name", "lap"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");

    let s = until(&lap, State::Local, &all);
    assert!(s.versions.iter().any(|v| v.path == "sub/big.bin"), "{s:?}");

    join(&lap, &c1, &all);
    until(&lap, State::Published, &all);
    join(&lap, &c2, &all);
    let s = until(&lap, State::Protected, &all);
    assert_eq!(s.headline.id, "headline-safe", "{s:?}");

    let machines = pigeon(&lap, &["machines"]);
    match machines.as_slice() {
        [Response::Machines { machines }] => {
            let mut names: Vec<&str> = machines.iter().map(|m| m.name.as_str()).collect();
            names.sort_unstable();
            assert_eq!(names, ["c1", "c2", "lap"]);
            assert_eq!(machines.iter().filter(|m| m.custodian).count(), 2);
        }
        other => panic!("machines: {other:?}"),
    }

    let sub = folder.join("sub");
    let freed = pigeon(&lap, &["free", sub.to_str().unwrap()]);
    assert_eq!(
        said(&freed, "free-result", "safe").as_deref(),
        Some("1"),
        "{freed:?}"
    );
    assert!(!sub.join("big.bin").exists(), "big.bin is still here");
    assert_eq!(status(&lap).state, State::Protected);

    let kept = pigeon(&lap, &["keep", sub.to_str().unwrap()]);
    assert!(
        said(&kept, "keep-result", "folder").is_some(),
        "{kept:?}\n{}",
        logs(&all)
    );
    assert_eq!(std::fs::read(sub.join("big.bin")).unwrap(), big);

    let history = pigeon(&lap, &["history", folder.join("a.txt").to_str().unwrap()]);
    match history.as_slice() {
        [Response::History { versions }] => {
            assert!(
                versions.iter().any(|v| v.state == State::Protected),
                "{versions:?}"
            );
        }
        other => panic!("history: {other:?}"),
    }
    match pigeon(&lap, &["pause"]).as_slice() {
        [Response::Problem(p)] => assert_eq!(p.code, "not-in-this-build"),
        other => panic!("pause: {other:?}"),
    }
}
