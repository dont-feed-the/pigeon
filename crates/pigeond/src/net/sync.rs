//! Syncing an object to disk before a custodian says it holds it (09 §9.4).
//!
//! git-annex never syncs, so the filter holds a custodian's positive answer
//! for a key until its object file and the directories above it, up to
//! `annex/objects`, are synced. A lock that proves another custodian's drop
//! also waits for the object's SHA-256 to match its key (spec 09 §9.4); a
//! mismatch queues `git annex fsck --key`. git-annex runs through
//! `pigeon-git`, with pigeon's environment. On macOS, `sync_all` is
//! `F_FULLFSYNC`.

use std::future::Future;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::pin::Pin;

use pigeon_git::env::GitEnv;
use pigeon_git::run::Repo;
use sha2::{Digest, Sha256};

use super::Repository;

/// A sync of one object: whether its hash matched (true if none was asked).
pub type SyncFuture = Pin<Box<dyn Future<Output = io::Result<bool>> + Send>>;

/// A repair started after a hash mismatch.
pub type RepairFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Makes the object of a key durable in a repository, and checks it.
pub trait Syncer: Send + Sync + 'static {
    /// Syncs the object of `key` and its directories to disk; if `hash`,
    /// also checks the object's SHA-256 against the key.
    fn sync(&self, repository: &Repository, key: &str, hash: bool) -> SyncFuture;
    /// Moves a rotten object out, so that repair starts (08 §8.4).
    fn repair(&self, repository: &Repository, key: &str) -> RepairFuture;
}

/// The deadline of each git-annex call of a sync or a repair.
const CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// How many times a repair runs fsck before it gives up and says so.
const REPAIR_ATTEMPTS: usize = 10;

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

/// The SHA-256 a `SHA256` or `SHA256E` key names, in hex.
pub fn key_sha256(key: &str) -> Option<&str> {
    let (backend, rest) = key.split_once('-')?;
    if backend != "SHA256" && backend != "SHA256E" {
        return None;
    }
    let (_, name) = rest.split_once("--")?;
    let hex = name.split('.').next()?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

fn directory(repository: &Repository) -> PathBuf {
    repository
        .worktree
        .clone()
        .unwrap_or_else(|| repository.git_dir.clone())
}

impl Syncer for AnnexSyncer {
    fn sync(&self, repository: &Repository, key: &str, hash: bool) -> SyncFuture {
        let repo = Repo::new(directory(repository), GitEnv::default());
        let key = key.to_owned();
        let provider = (repository.role == pigeon_core::filter::Role::Custodian)
            .then(|| (repository.git_dir.clone(), repository.group.clone()));
        Box::pin(async move {
            if !plausible_key(&key) {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "key"));
            }
            let out = repo
                .run(&["annex", "contentlocation", &key], None, CALL_TIMEOUT)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e.to_string()))?;
            let object = repo.dir.join(out.text().trim_end());
            tokio::task::spawn_blocking(move || {
                if let Some((git_dir, group)) = provider {
                    crate::shared_pool::received(&git_dir, &group, &key, &object)?;
                }
                sync_object(&object)?;
                if !hash {
                    return Ok(true);
                }
                let Some(expected) = key_sha256(&key) else {
                    return Ok(false);
                };
                Ok(sha256_file(&object)? == expected.to_ascii_lowercase())
            })
            .await
            .map_err(io::Error::other)?
        })
    }

    fn repair(&self, repository: &Repository, key: &str) -> RepairFuture {
        let repo = Repo::new(directory(repository), GitEnv::default());
        let key = key.to_owned();
        Box::pin(async move {
            if !plausible_key(&key) {
                return;
            }
            // fsck cannot move the object out while remotedaemon still holds
            // the lock the filter has just asked it to release: retry.
            for _ in 0..REPAIR_ATTEMPTS {
                let _ = repo
                    .run_status(&["annex", "fsck", "--key", &key], None, CALL_TIMEOUT)
                    .await;
                let present = repo
                    .run_status(&["annex", "contentlocation", &key], None, CALL_TIMEOUT)
                    .await
                    .map(|(code, _)| code == Some(0))
                    .unwrap_or(true);
                if !present {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            tracing::error!(key = %key, "repair-failed");
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

/// The SHA-256 of a file, in lowercase hex.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = pigeon_sys::fs::streaming_read(path).map_err(native_io)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn native_io(error: pigeon_sys::Error) -> io::Error {
    io::Error::new(error.kind().unwrap_or(io::ErrorKind::Other), error)
}
