//! Fresh Linux writer observations shared by one capture candidate pass.
//! Unknown/replaced inodes wait for the next pass. This is no lock: the
//! existing settling, exact-byte and complete-stat checks still apply.

use pigeon_git::status::Entry;
use pigeon_sys::{fs::FileId, writers::Snapshot};
use std::path::Path;

#[cfg(target_os = "linux")]
pub(crate) fn snapshot(worktree: &Path, changes: &[Entry]) -> Option<Snapshot> {
    let started = std::time::Instant::now();
    let identities = changes.iter().filter_map(|entry| {
        let path = match entry {
            Entry::Changed { path, .. }
            | Entry::Renamed { path, .. }
            | Entry::Untracked { path }
            | Entry::Ignored { path } => path,
            Entry::Unmerged { .. } => return None,
        };
        if crate::git::is_pigeon_path(path) {
            return None;
        }
        let stat = pigeon_sys::fs::lstat(&worktree.join(path)).ok().flatten()?;
        (stat.mode & 0o170_000 == 0o100_000).then_some(stat.id)
    });
    let observed = pigeon_sys::writers::snapshot(identities);
    if !changes.is_empty() {
        let elapsed_us = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        tracing::info!(
            entries = changes.len(),
            elapsed_us,
            "writer-inventory-complete"
        );
    }
    Some(observed)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn snapshot(_worktree: &Path, _changes: &[Entry]) -> Option<Snapshot> {
    None
}

pub(crate) fn clear(observed: Option<&Snapshot>, id: FileId) -> bool {
    observed.map_or_else(
        || pigeon_sys::writers::scan(id).is_clear(),
        |set| set.is_clear(id),
    )
}
