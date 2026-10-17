//! The pinned annex's keyed JSON progress (Messages/JSON.hs and
//! Messages/Progress.hs, 10.20261006). Only increasing bytes for this
//! operation renew inactivity time; progress never supplies a success or
//! durability verdict. Final output stays byte-for-byte unchanged.

use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncRead, AsyncReadExt};

/// Maximum additional buffered line: oversized output passes through.
const MAX_LINE: usize = 16_384;

#[derive(Debug)]
pub(crate) struct Progress {
    command: &'static str,
    key: String,
    bytes: AtomicU64,
}

fn transfer(args: &[&str]) -> Option<&'static str> {
    let mut args = args;
    while args.first() == Some(&"-c") {
        args = args.get(2..)?;
    }
    if !args.contains(&"--json") {
        return None;
    }
    match args {
        ["annex", "get", ..] => Some("get"),
        ["annex", "copy", ..] => Some("copy"),
        _ => None,
    }
}

pub(crate) fn instrument_batch(args: &mut Vec<String>) {
    let refs: Vec<_> = args.iter().map(String::as_str).collect();
    if transfer(&refs).is_some()
        && refs.contains(&"--batch-keys")
        && !refs.contains(&"--json-progress")
    {
        args.push("--json-progress".into());
    }
}

impl Progress {
    pub(crate) fn one_shot(args: &[&str]) -> Option<Self> {
        let command = transfer(args)?;
        let key = args
            .windows(2)
            .find(|pair| pair[0] == "--key")
            .map(|pair| pair[1])
            .or_else(|| args.iter().find_map(|arg| arg.strip_prefix("--key=")))?;
        Some(Self::new(command, key))
    }

    pub(crate) fn batch(args: &[&str], key: &str) -> Option<Self> {
        if !args.contains(&"--batch-keys") || !args.contains(&"--json-progress") {
            return None;
        }
        Some(Self::new(transfer(args)?, key))
    }

    fn new(command: &'static str, key: &str) -> Self {
        Self {
            command,
            key: key.to_owned(),
            bytes: AtomicU64::new(0),
        }
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    /// Strip only progress records. Unknown/malformed/final output remains
    /// visible to the existing caller's result parser.
    pub(crate) fn consume(&self, line: &[u8]) -> bool {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) else {
            return false;
        };
        let Some(bytes) = value
            .get("byte-progress")
            .and_then(serde_json::Value::as_u64)
        else {
            return false;
        };
        if value.get("success").is_some() {
            return false;
        }
        let action = value.get("action");
        let key = action
            .and_then(|a| a.get("key"))
            .and_then(serde_json::Value::as_str);
        let command = action
            .and_then(|a| a.get("command"))
            .and_then(serde_json::Value::as_str);
        let within_size = value
            .get("total-size")
            .is_none_or(|total| total.as_u64().is_some_and(|total| bytes <= total));
        if key == Some(self.key.as_str()) && command == Some(self.command) && within_size {
            self.bytes.fetch_max(bytes, Ordering::Relaxed);
        }
        true
    }
}

#[derive(Default)]
struct Lines {
    pending: Vec<u8>,
    oversized: bool,
}

impl Lines {
    fn feed(&mut self, input: &[u8], progress: &Progress, output: &mut Vec<u8>) {
        for &byte in input {
            if self.oversized {
                output.push(byte);
                if byte == b'\n' {
                    self.oversized = false;
                }
                continue;
            }
            self.pending.push(byte);
            if byte == b'\n' {
                if !progress.consume(&self.pending) {
                    output.append(&mut self.pending);
                }
                self.pending.clear();
            } else if self.pending.len() > MAX_LINE {
                output.append(&mut self.pending);
                self.oversized = true;
            }
        }
    }
}

pub(crate) async fn drain<R: AsyncRead + Unpin>(
    reader: &mut R,
    progress: Option<&Progress>,
    output: &mut Vec<u8>,
) -> std::io::Result<()> {
    let Some(progress) = progress else {
        reader.read_to_end(output).await?;
        return Ok(());
    };
    let mut lines = Lines::default();
    let mut bytes = vec![0u8; 8192];
    loop {
        let count = reader.read(&mut bytes).await?;
        if count == 0 {
            output.append(&mut lines.pending);
            return Ok(());
        }
        lines.feed(&bytes[..count], progress, output);
    }
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;
