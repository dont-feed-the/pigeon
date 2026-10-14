//! Trace events (12 §12.3, ADR 0008): one NDJSON line per action of the
//! `CaptureInstall` and `Roster` specifications, in `<state>/traces/`, when
//! the build has the `tla-trace` feature and `PIGEON_TRACE=1`. Otherwise
//! nothing is written.
//!
//! The Lamport clock is kept in each group's `clock.json` across restarts
//! (`clock_file`); it is sent to no peer yet (13 §13.12).

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// The actions `pigeond` logs, named as in the specifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// `CaptureInstall.Capture`: capture records a version.
    Capture,
    /// `CaptureInstall.Start`: an install journals `begin`.
    Start,
    /// `CaptureInstall.MoveAside`.
    MoveAside,
    /// `CaptureInstall.Scan`: the writer scan.
    Scan,
    /// `CaptureInstall.Check`: the check, then `proved`.
    Check,
    /// `CaptureInstall.RenameIn`.
    RenameIn,
    /// `CaptureInstall.Index`.
    Index,
    /// `CaptureInstall.Done`.
    Done,
    /// `CaptureInstall.Release`: the aside is released.
    Release,
    /// `CaptureInstall.Recover`: recovery of one entry at start.
    Recover,
    /// `Roster.Commit`: a roster change is committed.
    Commit,
    /// `Roster.Merge`: a fetched commit is fast-forwarded to.
    Merge,
}

/// Where trace events go.
#[derive(Debug)]
pub struct Tracer {
    out: Option<Mutex<PathBuf>>,
    clock: AtomicU64,
    device: String,
}

impl Tracer {
    /// A tracer writing to `dir`, if this build and the environment ask it.
    pub fn new(dir: PathBuf, device: &str) -> Tracer {
        let on = cfg!(feature = "tla-trace") && std::env::var("PIGEON_TRACE").as_deref() == Ok("1");
        Tracer {
            out: on.then(|| Mutex::new(dir.join(format!("{device}.ndjson")))),
            clock: AtomicU64::new(0),
            device: device.to_owned(),
        }
    }

    /// Logs `act` with `vars`.
    pub fn event(&self, act: Act, vars: &[(&str, &str)]) {
        let Some(out) = &self.out else {
            return;
        };
        let out = out.lock().unwrap_or_else(|p| p.into_inner());
        let clock = self.clock.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(line) = line(act, &self.device, clock, vars) {
            if let Some(dir) = out.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = pigeon_sys::fs::append_durable(&out, line.as_bytes());
        }
    }

    /// The Lamport clock: the last event's.
    pub fn clock(&self) -> u64 {
        self.clock.load(Ordering::SeqCst)
    }

    /// Moves the clock to at least `n`, as a restart reads it from
    /// `clock.json`.
    pub fn at_least(&self, n: u64) {
        self.clock.fetch_max(n, Ordering::SeqCst);
    }
}

#[cfg(feature = "tla-trace")]
fn line(act: Act, device: &str, clock: u64, vars: &[(&str, &str)]) -> Option<String> {
    use pigeon_core::trace::{CaptureInstall as C, Event, Roster as R, Value};
    let vars: Vec<(String, Value)> = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), Value::Str((*v).to_owned())))
        .collect();
    let e = match act {
        Act::Capture => Event::new(C::Capture, device, clock, vars),
        Act::Start => Event::new(C::Start, device, clock, vars),
        Act::MoveAside => Event::new(C::MoveAside, device, clock, vars),
        Act::Scan => Event::new(C::Scan, device, clock, vars),
        Act::Check => Event::new(C::Check, device, clock, vars),
        Act::RenameIn => Event::new(C::RenameIn, device, clock, vars),
        Act::Index => Event::new(C::Index, device, clock, vars),
        Act::Done => Event::new(C::Done, device, clock, vars),
        Act::Release => Event::new(C::Release, device, clock, vars),
        Act::Recover => Event::new(C::Recover, device, clock, vars),
        Act::Commit => Event::new(R::Commit, device, clock, vars),
        Act::Merge => Event::new(R::Merge, device, clock, vars),
    };
    let mut s = e.to_ndjson();
    if !s.ends_with('\n') {
        s.push('\n');
    }
    Some(s)
}

#[cfg(not(feature = "tla-trace"))]
fn line(_: Act, _: &str, _: u64, _: &[(&str, &str)]) -> Option<String> {
    None
}
