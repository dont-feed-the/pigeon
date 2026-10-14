//! Syncing an object to disk before a custodian says it holds it (09 §9.4).
//!
//! git-annex never syncs, so the filter holds a custodian's positive answer
//! for a key until its object file and the directories above it, up to
//! `annex/objects`, are synced. On macOS, `File::sync_all` is `F_FULLFSYNC`.

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use super::Repository;

/// A sync of one object, run by the relay when the filter asks for it.
pub type SyncFuture = Pin<Box<dyn Future<Output = io::Result<()>> + Send>>;

/// Makes the object of a key durable in a repository.
pub trait Syncer: Send + Sync + 'static {
    /// Syncs the object of `key` and its directories to disk.
    fn sync(&self, repository: &Repository, key: &str) -> SyncFuture;
}

/// Finds the object with `git annex contentlocation`, then syncs it.
#[derive(Clone, Copy, Debug, Default)]
pub struct AnnexSyncer;

/// Whether `key` may be passed to git-annex: it comes from a peer.
pub fn plausible_key(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('-')
        && key.len() <= 1024
        && key
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b'/' && b != b'\\')
}

impl Syncer for AnnexSyncer {
    fn sync(&self, repository: &Repository, key: &str) -> SyncFuture {
        let dir = repository
            .worktree
            .clone()
            .unwrap_or_else(|| repository.git_dir.clone());
        let key = key.to_owned();
        Box::pin(async move {
            if !plausible_key(&key) {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "key"));
            }
            let out = tokio::process::Command::new("git")
                .args(["annex", "contentlocation", &key])
                .current_dir(&dir)
                .kill_on_drop(true)
                .output()
                .await?;
            if !out.status.success() {
                return Err(io::Error::new(io::ErrorKind::NotFound, "contentlocation"));
            }
            let relative = String::from_utf8_lossy(&out.stdout).trim_end().to_owned();
            let object = dir.join(relative);
            tokio::task::spawn_blocking(move || sync_object(&object))
                .await
                .map_err(io::Error::other)?
        })
    }
}

/// Syncs `object`, then each directory above it up to `objects`.
pub fn sync_object(object: &Path) -> io::Result<()> {
    std::fs::File::open(object)?.sync_all()?;
    let mut dir: Option<PathBuf> = object.parent().map(Path::to_path_buf);
    for _ in 0..6 {
        let Some(d) = dir else { break };
        std::fs::File::open(&d)?.sync_all()?;
        if d.file_name().is_some_and(|n| n == "objects") {
            break;
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    Ok(())
}
