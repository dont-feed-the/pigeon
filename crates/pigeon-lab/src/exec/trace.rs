//! Trace logs of a run: every device's `<state>/traces/*.ndjson` (12 §12.3),
//! merged in Lamport-clock order, ties broken by device ID, and read back as
//! labels for the projection check. The merged file is what
//! `scripts/check-lab-traces` hands to TLC.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use super::observe::walk;
use crate::scenario::Label;
use crate::world::Device;

/// One trace event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// Its specification.
    pub spec: String,
    /// The device ID that logged it.
    pub device: String,
    /// Its Lamport clock.
    pub clock: u64,
    /// Its action.
    pub action: String,
    /// The line itself.
    pub raw: String,
}

/// The trace files under a device's disk.
pub fn files(root: &Path) -> Vec<PathBuf> {
    walk(root, true)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "ndjson"))
        .filter(|p| p.parent().is_some_and(|d| d.ends_with("pigeon/traces")))
        .collect()
}

/// Every event of these files, merged in clock order.
pub fn merged(files: &[PathBuf]) -> Vec<Line> {
    let mut lines = Vec::new();
    for f in files {
        let Ok(text) = fs::read_to_string(f) else {
            continue;
        };
        for raw in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(raw) else {
                continue;
            };
            let field = |k: &str| {
                v.get(k)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            lines.push(Line {
                spec: field("spec"),
                device: field("device"),
                clock: v.get("clock").and_then(Value::as_u64).unwrap_or(0),
                action: field("action"),
                raw: raw.to_owned(),
            });
        }
    }
    lines.sort_by(|a, b| (a.clock, &a.device).cmp(&(b.clock, &b.device)));
    lines
}

/// Writes the merged events, one per line.
pub fn write(lines: &[Line], path: &Path) -> std::io::Result<()> {
    let mut text = String::new();
    for l in lines {
        text.push_str(&l.raw);
        text.push('\n');
    }
    fs::write(path, text)
}

static NAMES: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());

/// A name of the trace vocabulary, kept for the life of the process.
fn intern(name: &str) -> &'static str {
    let Ok(mut names) = NAMES.lock() else {
        return "?";
    };
    if let Some(n) = names.get(name) {
        return n;
    }
    let n: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.insert(n);
    n
}

/// The labels of the events of known devices.
pub fn labels(lines: &[Line], ids: &BTreeMap<String, Device>) -> Vec<Label> {
    lines
        .iter()
        .filter_map(|l| {
            Some(Label {
                spec: intern(&l.spec),
                action: intern(&l.action),
                device: *ids.get(&l.device)?,
            })
        })
        .collect()
}
