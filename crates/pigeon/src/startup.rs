//! Start the local web launcher when the person's background daemon is not
//! running (ADR 0029, spec 10 §10.4). An explicit socket always belongs to
//! its caller: a launcher never starts a different daemon for that socket.

use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Connect to the person's daemon, starting it if necessary. Its own lock
/// arbitrates simultaneous launchers; startup diagnostics stay on disk.
pub(crate) fn connect() -> Option<UnixStream> {
    if std::env::var_os("PIGEON_SOCKET").is_some_and(|s| !s.is_empty()) {
        return None;
    }
    let socket = pigeon_sys::paths::client_socket();
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let daemon = exe.with_file_name("pigeond");
    let dirs = pigeon_sys::paths::Dirs::for_role(pigeon_sys::paths::Role::Person).ok()?;
    std::fs::create_dir_all(&dirs.state).ok()?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(dirs.state.join("startup.log"))
        .ok()?;
    let mut child = Command::new(daemon)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .process_group(0)
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(stream) = UnixStream::connect(&socket) {
            return Some(stream);
        }
        if Instant::now() >= deadline {
            return None;
        }
        // Another launcher may have won the daemon's lock. Keep trying
        // the socket even if this child exits.
        let _ = child.try_wait();
        std::thread::sleep(Duration::from_millis(50));
    }
}
