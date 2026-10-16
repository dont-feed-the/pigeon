//! Bounded local OTLP-JSON files (12 §12.2). Rotation retains the newest
//! file and drops the oldest first; no network or telemetry SDK lives here.

use super::{FILE_BYTES, LIMIT_BYTES};
use serde_json::Value;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub(crate) struct FileSink {
    pub(crate) dir: PathBuf,
    pub(crate) file: Mutex<Option<(PathBuf, std::fs::File, u64)>>,
}
fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// The files of `files` (name, bytes; names sort oldest first) to delete so
/// that the rest hold at most `limit` bytes; the newest always stays.
pub fn prune(files: &[(String, u64)], limit: u64) -> Vec<String> {
    let mut sorted: Vec<&(String, u64)> = files.iter().collect();
    sorted.sort();
    let mut total: u64 = sorted.iter().map(|(_, n)| n).sum();
    let mut out = Vec::new();
    for (name, n) in sorted.iter().take(sorted.len().saturating_sub(1)) {
        if total <= limit {
            break;
        }
        total -= n;
        out.push(name.clone());
    }
    out
}

fn listed(dir: &Path) -> Vec<(String, u64)> {
    std::fs::read_dir(dir)
        .map(|it| {
            it.filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
                .map(|e| {
                    let n = e.metadata().map_or(0, |m| m.len());
                    (e.file_name().to_string_lossy().into_owned(), n)
                })
                .collect()
        })
        .unwrap_or_default()
}

impl FileSink {
    pub(crate) fn write(&self, line: &Value) {
        let mut bytes = serde_json::to_vec(line).unwrap_or_default();
        bytes.push(b'\n');
        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        if file.as_ref().is_none_or(|(_, _, n)| *n >= FILE_BYTES) {
            let _ = std::fs::create_dir_all(&self.dir);
            let name = format!("pigeond-{:020}.jsonl", now_ns() / 1_000);
            let path = self.dir.join(name);
            *file = std::fs::OpenOptions::new()
                .mode(0o600)
                .create(true)
                .append(true)
                .open(&path)
                .ok()
                .map(|f| (path, f, 0));
            for old in prune(&listed(&self.dir), LIMIT_BYTES - FILE_BYTES) {
                let _ = std::fs::remove_file(self.dir.join(old));
            }
        }
        if let Some((_, f, n)) = file.as_mut()
            && f.write_all(&bytes).is_ok()
        {
            *n += bytes.len() as u64;
        }
    }
}
/// The last `n` lines written into `dir`, oldest first, for a debug bundle.
pub fn tail(dir: &Path, n: usize) -> Vec<String> {
    let mut files = listed(dir);
    files.sort();
    let mut out: Vec<String> = Vec::new();
    for (name, _) in files.iter().rev() {
        let text = std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        lines.append(&mut out);
        out = lines;
        if out.len() >= n {
            break;
        }
    }
    let skip = out.len().saturating_sub(n);
    out.split_off(skip)
}
