//! Trace events (12 §12.3, ADR 0008): one NDJSON line per action of the
//! filesystem drivers and accepted roster history, in `<state>/traces/`,
//! when the build has the `tla-trace` feature and `PIGEON_TRACE=1`.
//! Otherwise nothing is written.
//!
//! The Lamport clock is kept in each group's `clock.json` across restarts
//! (`clock_file`); it is sent to no peer yet (13 §13.12).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Where trace events go.
#[derive(Debug)]
pub struct Tracer {
    out: Option<Mutex<PathBuf>>,
    clock: AtomicU64,
    device: String,
    pending: Mutex<BTreeSet<(String, String)>>,
}

impl Tracer {
    /// Whether this tracer is collecting implementation observations.
    pub fn enabled(&self) -> bool {
        self.out.is_some()
    }

    /// A structured observation, sharing the action log's durable clock.
    pub(crate) fn observed(&self, spec: &str, action: &str, vars: serde_json::Value) {
        let Some(out) = &self.out else { return };
        let out = out.lock().unwrap_or_else(|p| p.into_inner());
        let clock = self.clock.fetch_add(1, Ordering::SeqCst) + 1;
        let event = serde_json::json!({
            "spec": spec, "device": self.device,
            "clock": clock, "action": action, "vars": vars,
        });
        if let Some(dir) = out.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(error) = pigeon_sys::fs::append_durable(&out, event.to_string().as_bytes()) {
            tracing::error!(%error, "implementation-trace-write-failed");
        } else {
            remember(
                &mut self.pending.lock().unwrap_or_else(|p| p.into_inner()),
                &event,
            );
        }
    }

    /// A closed journal entry still needs a checkpoint if a process died
    /// after its disk effect but before its after observation was durable.
    pub(crate) fn unfinished(&self, group: &str, path: &str) -> bool {
        self.enabled()
            && self
                .pending
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .contains(&(group.to_owned(), crate::util::hex(path.as_bytes())))
    }

    /// A tracer writing to `dir`, if this build and the environment ask it.
    pub fn new(dir: PathBuf, device: &str) -> Tracer {
        let on = cfg!(feature = "tla-trace") && std::env::var("PIGEON_TRACE").as_deref() == Ok("1");
        let dir = if on {
            let _ = std::fs::create_dir_all(&dir);
            std::fs::canonicalize(&dir).unwrap_or(dir)
        } else {
            dir
        };
        let dir = match std::env::var_os("PIGEON_TRACE_DIR") {
            Some(root) if on => {
                use sha2::Digest;
                let id = sha2::Sha256::digest(dir.as_os_str().as_encoded_bytes());
                PathBuf::from(root).join(crate::util::hex(&id))
            }
            _ => dir,
        };
        let path = dir.join(format!("{device}.ndjson"));
        // A crash may precede the clock.json flush. The append log itself
        // keeps this device's clock increasing across that restart.
        let mut clock = 0;
        let mut pending = BTreeSet::new();
        if on {
            for line in std::fs::read_to_string(&path).unwrap_or_default().lines() {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                    clock = clock.max(event["clock"].as_u64().unwrap_or(0));
                    remember(&mut pending, &event);
                }
            }
        }
        Tracer {
            out: on.then(|| Mutex::new(path)),
            clock: AtomicU64::new(clock),
            device: device.to_owned(),
            pending: Mutex::new(pending),
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

fn remember(pending: &mut BTreeSet<(String, String)>, event: &serde_json::Value) {
    let v = &event["vars"];
    let (Some(group), Some(path)) = (v["group"].as_str(), v["path"].as_str()) else {
        return;
    };
    let key = (group.to_owned(), path.to_owned());
    match v["edge"].as_str() {
        Some("before") => {
            pending.insert(key);
        }
        Some("after") => {
            pending.remove(&key);
        }
        _ => {}
    }
}
