//! `.git/pigeon/capture.json`, a capture batch's intent (07 §7.3, 11 §11.3):
//! the index entry each path of the batch had before capture staged it.
//! Capture writes it before it touches the index and removes it once the
//! batch is in `main` or given up; a capture that finds it, after a crash,
//! puts those entries back first, so that the next scan redoes the batch
//! from the files ("a crash anywhere loses nothing"). The person's files
//! are never touched; what `git annex add` stored stays, under the same
//! keys. Laws (`tests/codecs.rs`): it parses back to what it renders, and
//! one of a newer format reads as nothing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::group::Group;

/// The format this release writes.
pub const FORMAT: u32 = 1;

/// An index entry: its mode and object ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Staged {
    /// Such as `100644`.
    pub mode: String,
    /// The blob's ID.
    pub oid: String,
}

/// One batch's intent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    /// Its format.
    pub format: u32,
    /// `main` when the batch began.
    pub head: Option<String>,
    /// Each path's index entry before the batch; `None` if it had none.
    pub before: BTreeMap<String, Option<Staged>>,
}

/// The intent as JSON.
pub fn render(i: &Intent) -> String {
    serde_json::to_string_pretty(i).unwrap_or_default()
}

/// The intent of `text`, `None` if it is not one this release reads.
pub fn parse(text: &str) -> Option<Intent> {
    serde_json::from_str::<Intent>(text)
        .ok()
        .filter(|i| i.format <= FORMAT)
}

fn file(g: &Group) -> std::path::PathBuf {
    g.root().join("capture.json")
}

/// Whether a batch still owns the index. An unreadable intent is pending
/// too: callers must not mistake a staged save for an installed version.
pub fn pending(g: &Group) -> bool {
    file(g).try_exists().unwrap_or(true)
}

/// The group's pending intent, if a batch did not finish.
pub fn read(g: &Group) -> Option<Intent> {
    parse(&std::fs::read_to_string(file(g)).ok()?)
}

/// Records a batch's intent before it touches the index.
pub fn write(g: &Group, i: &Intent) -> std::io::Result<()> {
    let _ = std::fs::create_dir_all(g.root());
    pigeon_sys::fs::write_atomic(&file(g), render(i).as_bytes()).map_err(std::io::Error::other)
}

/// Forgets the intent once its batch is done.
pub fn clear(g: &Group) {
    let _ = std::fs::remove_file(file(g));
}
