//! `.git/pigeon/conflicts.json`, the divergence table (07 §7.6, 11 §11.3):
//! each path whose file on this laptop differs from the catalogue's
//! version, which install leaves alone (ADR 0034). Rewritten whole, with a
//! write-then-rename; a file of a newer `format` is read as nothing and
//! never rewritten.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::group::Group;

/// The format this release writes.
pub const FORMAT: u32 = 1;

/// Why a path diverges (07 §7.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Capture made a proposal for the path.
    Suggestion,
    /// A merge gave the path to another version, or a save won the name.
    Concurrent,
    /// Install moved aside content not its own and could not put it back.
    Recovered,
}

/// Where a divergence stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Waiting for the deciders.
    Waiting,
    /// The person chooses (ux.md §8.8).
    Conflict,
}

/// One diverging path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Divergence {
    /// Why it diverges.
    pub kind: Kind,
    /// When it started, RFC 3339.
    pub since: String,
    /// Where it stands.
    pub state: State,
    /// The key of the person's version, once captured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mine: Option<String>,
    /// The stat of the person's file when `mine` was captured:
    /// `[dev, ino, size, mtime_ns]`, which *Take the group's version* checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mine_stat: Option<[i128; 4]>,
    /// The latest proposal of the person's chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<String>,
    /// The key of the catalogue's version, if the path has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The key pigeon last wrote at the path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<String>,
    /// Whether the person was told (once, ADR 0034).
    pub told: bool,
}

/// The table.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// Its format.
    pub format: u32,
    /// The diverging paths.
    pub paths: BTreeMap<String, Divergence>,
}

/// The table as JSON.
pub fn render(t: &Table) -> String {
    serde_json::to_string_pretty(t).unwrap_or_default()
}

/// A table read back: `None` if malformed or of a newer format.
pub fn parse(text: &str) -> Option<Table> {
    let t: Table = serde_json::from_str(text).ok()?;
    (t.format <= FORMAT).then_some(t)
}

fn file(g: &Group) -> std::path::PathBuf {
    g.root().join("conflicts.json")
}

/// The group's table: empty when there is none; `None` when it cannot be
/// read, which install then treats as every changed path diverging.
pub fn read(g: &Group) -> Option<Table> {
    match std::fs::read_to_string(file(g)) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(Table {
            format: FORMAT,
            paths: BTreeMap::new(),
        }),
        Err(_) => None,
    }
}

/// Writes the group's table whole.
pub fn write(g: &Group, t: &Table) -> Result<(), String> {
    let _ = std::fs::create_dir_all(g.root());
    let t = Table {
        format: FORMAT,
        paths: t.paths.clone(),
    };
    pigeon_sys::fs::write_atomic(&file(g), render(&t).as_bytes()).map_err(|e| e.to_string())
}
