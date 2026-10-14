//! `.git/pigeon/clock.json` (11 §11.3, ADR 0036): when each peer was last
//! heard of, and the Lamport clock of trace events (12 §12.3), so that a
//! restart keeps both. It is written after a round that changed them, and
//! read when the group is opened; losing it only forgets when peers were
//! heard of and weakens trace ordering.
//!
//! The schema is closed (ADR 0048): a file of a newer `format`, or with a
//! field this release does not know, is not read and not overwritten.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::group::Group;

/// The format this release writes.
pub const FORMAT: u32 = 1;

/// The file's content.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    /// Its format.
    pub format: u32,
    /// The Lamport clock.
    pub clock: u64,
    /// When each peer was last heard of, Unix seconds, by device ID.
    pub heard: BTreeMap<String, i64>,
}

/// What reading the file found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read {
    /// No file, or one that does not parse: start afresh.
    Missing,
    /// A file this release reads.
    Known(Clock),
    /// A newer format or an unknown field: kept as it is.
    Unknown,
}

/// Parses the file's text.
pub fn parse(text: &str) -> Read {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Read::Missing;
    };
    if value.get("format").and_then(serde_json::Value::as_u64) != Some(u64::from(FORMAT)) {
        return Read::Unknown;
    }
    match serde_json::from_value::<Clock>(value) {
        Ok(c) => Read::Known(c),
        Err(_) => Read::Unknown,
    }
}

fn path(g: &Group) -> std::path::PathBuf {
    g.root().join("clock.json")
}

/// Reads `g`'s file.
pub fn read(g: &Group) -> Read {
    match std::fs::read_to_string(path(g)) {
        Ok(text) => parse(&text),
        Err(_) => Read::Missing,
    }
}

/// Writes `g`'s file with `clock` and what `g` knows of its peers, unless
/// it holds the same or a format this release does not read.
pub fn save(g: &Group, clock: u64) {
    let heard = g.lock().heard.clone();
    let new = Clock {
        format: FORMAT,
        clock,
        heard,
    };
    match read(g) {
        Read::Unknown => return,
        Read::Known(old) if old == new => return,
        _ => {}
    }
    let _ = std::fs::create_dir_all(g.root());
    if let Ok(text) = serde_json::to_vec(&new) {
        let _ = pigeon_sys::fs::write_atomic(&path(g), &text);
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, Read, parse};

    #[test]
    fn a_file_reads_back_and_a_newer_or_wider_one_is_unknown() {
        let c = Clock {
            format: 1,
            clock: 42,
            heard: [("d1".to_owned(), 7)].into_iter().collect(),
        };
        let text = serde_json::to_string(&c).unwrap_or_default();
        assert_eq!(parse(&text), Read::Known(c));
        assert_eq!(parse(r#"{"format":2,"clock":1,"heard":{}}"#), Read::Unknown);
        assert_eq!(
            parse(r#"{"format":1,"clock":1,"heard":{},"new":true}"#),
            Read::Unknown
        );
        assert_eq!(parse("{torn"), Read::Missing);
    }
}
