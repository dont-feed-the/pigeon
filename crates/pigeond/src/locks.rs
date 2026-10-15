//! Stale git lock files (11 §11.4). git takes a lock by creating
//! `<file>.lock` and renaming it over the file; a git process killed while
//! it holds one, as `kill_on_drop` does when pigeond stops mid-command or a
//! power cut does, leaves the lock behind, and every later command that
//! needs it fails (`index.lock': File exists`). S9's random schedules
//! showed capture failing so for minutes on macOS.
//!
//! As git-annex's assistant does (`repairStaleGitLocks`,
//! `Assistant/Repair.hs`), a lock not written for [`STALE`] is taken as
//! stale and removed: git writes a lock's content before it renames it, so
//! one that a live process holds keeps changing or goes. pigeond looks
//! before each round, at the repository's top-level locks, those under
//! `refs/`, and git-annex's `annex/index.lock`.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How long a lock file may go unwritten before it is stale.
pub const STALE: Duration = Duration::from_secs(60);

fn locks_in(dir: &Path, deep: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let Ok(kind) = e.file_type() else {
            continue;
        };
        if kind.is_dir() && deep {
            locks_in(&path, true, out);
        } else if kind.is_file() && path.extension().is_some_and(|x| x == "lock") {
            out.push(path);
        }
    }
}

/// The lock files of the repository at `git_dir` older than [`STALE`] at
/// `now`.
pub fn stale(git_dir: &Path, now: SystemTime) -> Vec<PathBuf> {
    let mut found = Vec::new();
    locks_in(git_dir, false, &mut found);
    locks_in(&git_dir.join("refs"), true, &mut found);
    let annex = git_dir.join("annex").join("index.lock");
    if annex.is_file() {
        found.push(annex);
    }
    found
        .into_iter()
        .filter(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .is_ok_and(|t| now.duration_since(t).is_ok_and(|age| age >= STALE))
        })
        .collect()
}

/// Removes the stale lock files of the repository at `git_dir`; answers
/// those it removed.
pub fn clear(git_dir: &Path) -> Vec<PathBuf> {
    let gone: Vec<PathBuf> = stale(git_dir, SystemTime::now())
        .into_iter()
        .filter(|p| std::fs::remove_file(p).is_ok())
        .collect();
    if !gone.is_empty() {
        tracing::warn!(locks = ?gone, "stale-locks-removed");
    }
    gone
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn touch(path: &Path, age: Duration) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let f = std::fs::File::create(path).unwrap();
        f.set_modified(SystemTime::now() - age).unwrap();
    }

    #[test]
    fn only_locks_left_unwritten_for_a_minute_are_stale() {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path();
        let old = Duration::from_secs(120);
        touch(&g.join("index.lock"), old);
        touch(&g.join("refs/heads/main.lock"), old);
        touch(&g.join("annex/index.lock"), old);
        touch(&g.join("packed-refs.lock"), Duration::from_secs(5));
        touch(&g.join("objects/ab/cd.lock"), old);
        touch(&g.join("annex/objects/x.lock"), old);
        touch(&g.join("index"), old);
        let mut found = stale(g, SystemTime::now());
        found.sort();
        let want: Vec<PathBuf> = ["annex/index.lock", "index.lock", "refs/heads/main.lock"]
            .iter()
            .map(|p| g.join(p))
            .collect();
        assert_eq!(found, want);
        assert_eq!(clear(g).len(), 3);
        assert!(!g.join("index.lock").exists());
        assert!(g.join("packed-refs.lock").exists());
        assert!(g.join("index").exists());
    }
}
