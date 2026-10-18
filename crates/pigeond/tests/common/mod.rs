//! What the multi-daemon tests share (13 §13.12): `pigeond` processes on
//! one host, each with its own home, driven through the `pigeon` CLI with
//! `--json`, and their logs for a failure's message. Every daemon's person
//! is `ana` (`$USER`, 13 §13.12) unless its join names another.
#![allow(dead_code, reason = "each test binary uses a part")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
#![allow(clippy::print_stderr, reason = "tests")]

pub mod web;

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use pigeon_proto::api::{State, StatusReport};
use pigeon_proto::{Arg, Response};

/// One running `pigeond`, killed with its process group on drop.
pub struct Daemon {
    pub name: &'static str,
    pub home: PathBuf,
    socket: PathBuf,
    child: Child,
}

impl Daemon {
    /// The fixture’s disposable daemon, never a person’s installed service.
    pub fn process_id(&self) -> u32 {
        self.child.id()
    }
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
        // The group is usually gone by now; `kill` would say so on stderr.
        let group = format!("-{pid}");
        let _ = Command::new("kill")
            .args(["-KILL", "--", &group])
            .stderr(Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

pub fn bin(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_BIN_EXE_pigeond")).parent().unwrap();
    let path = dir.join(name);
    assert!(
        path.exists(),
        "{} is missing: cargo build -p pigeon -p git-annex-p2p-iroh",
        path.display()
    );
    path
}

pub fn git_env<'a>(cmd: &'a mut Command, home: &Path) -> &'a mut Command {
    cmd.env("HOME", home)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
}

pub fn start(root: &Path, sockets: &Path, name: &'static str) -> Daemon {
    start_with_crash(root, sockets, name, None)
}

/// Starts a lab daemon whose named transition will be killed with SIGKILL.
pub fn start_with_crash(
    root: &Path,
    sockets: &Path,
    name: &'static str,
    crash: Option<&str>,
) -> Daemon {
    let home = root.join(name);
    std::fs::create_dir_all(home.join("config")).unwrap();
    let config = "[network]\nbind = \"127.0.0.1:0\"\nmdns = false\nrelays = \"off\"\n\n[loops]\ninterval_seconds = 1\n";
    // Kept on a restart: a join adds the group to it.
    let file = home.join("config/config.toml");
    if !file.exists() {
        std::fs::write(file, config).unwrap();
    }
    let socket = sockets.join(format!("{name}.sock"));
    let log = std::fs::File::create(home.join("pigeond.out")).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pigeond"));
    if let Some(point) = crash {
        cmd.env("PIGEON_LAB_CRASH", format!("{point}:1"));
    }
    cmd.arg("--home")
        .arg(&home)
        .env("PIGEON_SOCKET", &socket)
        .env("PIGEON_LOG", "debug")
        .env("PIGEON_NOTIFY_LOG", home.join("notified.log"))
        .env("USER", "ana")
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

/// Confirms the fault seam actually killed the daemon, rather than a
/// command returning early or a different process failure passing a test.
pub fn assert_crashed(d: &mut Daemon) {
    use std::os::unix::process::ExitStatusExt;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = d.child.try_wait().unwrap() {
            assert_eq!(status.signal(), Some(9), "{status}\n{}", logs(&[d]));
            return;
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not crash\n{}",
            logs(&[d])
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Per-connection lines, which fill the tail of a failure's logs and hide
/// what the loops did; the kept directory has them all.
const CHATTER: [&str; 3] = [
    "\"message\":\"relay-ended\"",
    "\"message\":\"connect-ended\"",
    "\"message\":\"stream-admitted\"",
];

/// Each daemon's own log lines (not its libraries' nor per-connection ones)
/// and its output, for a failure's message.
pub fn logs(all: &[&Daemon]) -> String {
    let mut out = String::new();
    for d in all {
        out.push_str(&format!("--- {}\n", d.name));
        let dir = d.home.join("state/logs");
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            let lines: Vec<&str> = text
                .lines()
                .filter(|l| l.contains("\"target\":\"pigeond") || l.contains("\"WARN\""))
                .filter(|l| !CHATTER.iter().any(|m| l.contains(m)))
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
pub fn pigeon(d: &Daemon, args: &[&str]) -> Vec<Response> {
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

/// Sends `request` to `d`'s socket, for a request no command sends; its
/// result, progress left out.
pub fn ask(d: &Daemon, request: &pigeon_proto::Request) -> Vec<Response> {
    use std::io::{BufRead, Write};
    let mut stream = std::os::unix::net::UnixStream::connect(&d.socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(180)))
        .unwrap();
    stream
        .write_all(&pigeon_proto::encode(1, request).unwrap())
        .unwrap();
    let mut reader = std::io::BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        assert!(reader.read_line(&mut line).unwrap() > 0, "no answer");
        let reply: pigeon_proto::Envelope<Response> = serde_json::from_str(&line).unwrap();
        if !matches!(reply.body, Response::Progress { .. }) {
            return vec![reply.body];
        }
    }
}

/// The text argument `name` of the message `id` that `rs` say.
pub fn said(rs: &[Response], id: &str, name: &str) -> Option<String> {
    rs.iter().find_map(|r| match r {
        Response::Said { message } if message.id == id => match message.args.get(name) {
            Some(Arg::Text(t)) => Some(t.clone()),
            Some(Arg::Number(n)) => Some(n.to_string()),
            None => Some(String::new()),
        },
        _ => None,
    })
}

pub fn status(d: &Daemon) -> StatusReport {
    match pigeon(d, &["status"]).as_slice() {
        [Response::Status(s)] => s.clone(),
        other => panic!("status: {other:?}"),
    }
}

/// Waits for captured versions to reach `want`. A pending save is local
/// too, but is not yet a history entry that the next test step may use.
pub fn until(lap: &Daemon, want: State, all: &[&Daemon]) -> StatusReport {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let s = status(lap);
        let capturing = s
            .versions
            .iter()
            .any(|v| v.reason.as_ref().is_some_and(|r| r.id == "why-capturing"));
        if s.state == want && !capturing {
            return s;
        }
        if Instant::now() > deadline {
            panic!("never {want:?}; last {s:?}\n{}", logs(all));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// The group's invitation, from the line `pigeon invite` prints on `d`.
pub fn invitation(d: &Daemon) -> String {
    let answer = pigeon(d, &["invite"]);
    let line = said(&answer, "invitation", "command").unwrap_or_else(|| panic!("{answer:?}"));
    pigeon_proto::join::invitation(&line)
}

/// The name of `d`'s person, as `pigeon people` lists it.
pub fn person(d: &Daemon) -> String {
    match pigeon(d, &["people"]).as_slice() {
        [Response::People { people }] => people
            .iter()
            .find(|p| p.this_person)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| panic!("no person of this computer: {people:?}")),
        other => panic!("people: {other:?}"),
    }
}

/// Joins `c` to `lap`'s group as a backup machine of `lap`'s person, named
/// `c.name`.
pub fn join(lap: &Daemon, c: &Daemon, all: &[&Daemon]) {
    let code = invitation(lap);
    let owner = person(lap);
    let joined = pigeon(
        c,
        &[
            "join",
            "--backup-machine",
            &code,
            "--name",
            c.name,
            "--person",
            &owner,
            "--yes",
        ],
    );
    assert!(
        said(&joined, "joined", "group").is_some(),
        "{joined:?}\n{}",
        logs(all)
    );
}

pub fn annex_present() -> bool {
    Command::new("git")
        .args(["annex", "version", "--raw"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// One test at a time: each runs three daemons.
pub static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A generated annex fixture: git-annex makes object directories read-only.
/// Restore owner traversal/write permission before TempDir removes our tree;
/// otherwise successful suites leak enough fixtures to hit the disk reserve.
/// A failed test keeps it, with each daemon's full logs, and says where.
pub struct FixtureDir(tempfile::TempDir);
impl Drop for FixtureDir {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        if std::thread::panicking() {
            self.0.disable_cleanup(true);
            eprintln!(
                "kept the failed test's directory: {} (each daemon's logs in <name>/state/logs)",
                self.0.path().display()
            );
            return;
        }
        let mut dirs = vec![self.0.path().to_path_buf()];
        while let Some(dir) = dirs.pop() {
            let Ok(meta) = std::fs::symlink_metadata(&dir) else {
                continue;
            };
            if !meta.is_dir() {
                continue;
            }
            let _ = std::fs::set_permissions(
                &dir,
                std::fs::Permissions::from_mode(meta.permissions().mode() | 0o700),
            );
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    dirs.push(entry.path());
                }
            }
        }
    }
}

/// A temporary root and a short private directory for the sockets: a unix
/// socket's path holds about 100 bytes, and macOS's temporary directory is
/// long. `None` without git-annex, unless it is required.
pub fn dirs() -> Option<(FixtureDir, PathBuf, tempfile::TempDir)> {
    if !annex_present() {
        assert!(
            std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none(),
            "git-annex is required"
        );
        eprintln!("skipped: no git-annex");
        return None;
    }
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let sockets = tempfile::Builder::new()
        .prefix("pgn")
        .tempdir_in("/tmp")
        .unwrap();
    Some((FixtureDir(tmp), root, sockets))
}

/// Waits up to 60 s for `done`, else fails with `what` and the logs.
pub fn eventually(done: impl Fn() -> bool, what: &str, all: &[&Daemon]) {
    eventually_within(60, done, what, all);
}

/// Waits up to `secs` for `done`, else fails with `what` and the logs.
pub fn eventually_within(secs: u64, done: impl Fn() -> bool, what: &str, all: &[&Daemon]) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !done() {
        assert!(Instant::now() < deadline, "{what}\n{}", logs(all));
        std::thread::sleep(Duration::from_millis(250));
    }
}
