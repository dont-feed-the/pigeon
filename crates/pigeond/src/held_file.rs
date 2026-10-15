//! `.git/pigeon/held.json`, the held mass deletions (07 §7.4, 11 §11.3):
//! each batch capture held, with why and since when, and the paths the
//! person released with *Delete them for everyone*, which the next capture
//! deletes whatever their number. Written whole, atomically. A held path
//! whose file is back, or that `main` no longer has, leaves its batch;
//! an empty batch goes.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::group::Group;

/// The table's format.
pub const FORMAT: u32 = 1;

/// The held deletions of one repository.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// [`FORMAT`].
    pub format: u32,
    /// The held batches, oldest first.
    #[serde(default)]
    pub batches: Vec<Batch>,
    /// Paths released for deletion, until a capture commits it.
    #[serde(default)]
    pub released: BTreeSet<String>,
    /// The last ID given, so that none is given twice.
    #[serde(default)]
    pub last: u64,
}

/// One held batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Batch {
    /// Its ID, a number this repository never gave before.
    pub id: String,
    /// `mass-deletion` or `top-folder`.
    pub why: String,
    /// The removed top-level folder, for `top-folder`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// When it was held, Unix seconds.
    pub since: i64,
    /// Its paths.
    pub paths: BTreeSet<String>,
}

impl Table {
    /// Every held path.
    pub fn held(&self) -> BTreeSet<String> {
        self.batches
            .iter()
            .flat_map(|b| b.paths.iter().cloned())
            .collect()
    }

    /// Holds `paths`, for `why`, under a new ID.
    pub fn hold(&mut self, why: &pigeon_core::held::Why, paths: BTreeSet<String>, now: i64) {
        let next = self
            .batches
            .iter()
            .filter_map(|b| b.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            .max(self.last)
            + 1;
        self.last = next;
        let (why, folder) = match why {
            pigeon_core::held::Why::Mass => ("mass-deletion", None),
            pigeon_core::held::Why::TopFolder(f) => ("top-folder", Some(f.clone())),
        };
        self.batches.push(Batch {
            id: next.to_string(),
            why: why.to_owned(),
            folder,
            since: now,
            paths,
        });
    }

    /// Drops `paths` from the batches, and the batches left empty.
    pub fn forget(&mut self, paths: &BTreeSet<String>) {
        for b in &mut self.batches {
            b.paths.retain(|p| !paths.contains(p));
        }
        self.batches.retain(|b| !b.paths.is_empty());
    }

    /// Takes batch `id` out, if there is one.
    pub fn take(&mut self, id: &str) -> Option<Batch> {
        let at = self.batches.iter().position(|b| b.id == id)?;
        Some(self.batches.remove(at))
    }
}

fn file(g: &Group) -> PathBuf {
    g.root().join("held.json")
}

/// The table, empty when there is none.
pub fn read(g: &Group) -> Table {
    std::fs::read(file(g))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Table {
            format: FORMAT,
            ..Table::default()
        })
}

/// Writes the table, or removes the file when it holds nothing.
pub fn write(g: &Group, t: &Table) -> std::io::Result<()> {
    if t.batches.is_empty() && t.released.is_empty() && t.last == 0 {
        let _ = std::fs::remove_file(file(g));
        return Ok(());
    }
    let _ = std::fs::create_dir_all(g.root());
    let bytes = serde_json::to_vec_pretty(t).map_err(std::io::Error::other)?;
    pigeon_sys::fs::write_atomic(&file(g), &bytes).map_err(std::io::Error::other)
}
