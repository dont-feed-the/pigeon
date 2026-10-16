//! A group-scoped immutable pool. Only qualified erasure releases names.
//! Files are verified before
//! publication; hard links pin immutable source objects independently of drops.
//! Mutable sources get independent snapshots. Per-key locks serialize writers.

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::net::sync::{key_sha256, plausible_key, sha256_file, sync_object};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    format: u32,
    store: PathBuf,
}

pub(crate) fn registration(git_dir: &Path) -> io::Result<Option<PathBuf>> {
    let body = match std::fs::read(git_dir.join("pigeon/shared-provider.json")) {
        Ok(body) => body,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let registration: Registration = serde_json::from_slice(&body)?;
    if registration.format != 1 || !registration.store.is_absolute() {
        return Err(io::Error::other("Unsupported shared provider registration"));
    }
    Ok(Some(registration.store))
}

pub(crate) fn register(git_dir: &Path, store: &Path) -> io::Result<()> {
    private_dir(store)?;
    if store.metadata()?.dev() != git_dir.metadata()?.dev() {
        return Err(io::Error::other(
            "A storage provider and its pool must share a filesystem",
        ));
    }
    let store = store.canonicalize()?;
    if let Some(existing) = registration(git_dir)? {
        if existing != store {
            return Err(io::Error::other(
                "This group already uses a different shared pool",
            ));
        }
        return Ok(());
    }
    let body = serde_json::to_vec(&Registration { format: 1, store })?;
    pigeon_sys::fs::write_atomic(&git_dir.join("pigeon/shared-provider.json"), &body)
        .map_err(io::Error::other)
}

/// Run before a custodian's network acknowledgement. Its retained reference
/// and the bridge pool must name the same verified immutable inode.
pub(crate) fn received(git_dir: &Path, group: &str, key: &str, object: &Path) -> io::Result<()> {
    let Some(store) = registration(git_dir)? else {
        return Ok(());
    };
    let target = retain(&store, group, key, object, true)?;
    relink(&target, object)
}

pub(crate) fn private_dir(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || path.parent().is_none() {
        return Err(io::Error::other(
            "Content directories must be absolute private paths",
        ));
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    let m = path.symlink_metadata()?;
    if !m.is_dir() || m.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::other(
            "The content directory must be a private directory (0700)",
        ));
    }
    Ok(())
}

pub(crate) fn path(store: &Path, group: &str, key: &str) -> io::Result<PathBuf> {
    let hash = key_sha256(key)
        .filter(|_| plausible_key(key))
        .ok_or_else(|| io::Error::other("Invalid content key"))?;
    if !matches!(group.len(), 40 | 64) || !group.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::other("Invalid group ID"));
    }
    let hash = hash.to_ascii_lowercase();
    Ok(store.join(group).join(&hash[..2]).join(hash))
}

pub(crate) fn verify(file: &Path, key: &str) -> io::Result<()> {
    let meta = file.symlink_metadata()?;
    if !meta.is_file()
        || crate::restore::key_size(key) != Some(meta.len())
        || key_sha256(key)
            .is_none_or(|h| !sha256_file(file).is_ok_and(|got| got.eq_ignore_ascii_case(h)))
    {
        return Err(io::Error::other("Shared content does not match its key"));
    }
    Ok(())
}

fn temporary(path: &Path) -> io::Result<PathBuf> {
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(io::Error::other)?;
    Ok(path.with_extension(crate::util::hex(&random)))
}

struct Pending(PathBuf);
impl Drop for Pending {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Retains a verified version. A writable inode is never shared with the pool.
pub(crate) fn retain(
    store: &Path,
    group: &str,
    key: &str,
    source: &Path,
    immutable: bool,
) -> io::Result<PathBuf> {
    retain_with_sync(store, group, key, source, immutable, |directory| {
        pigeon_sys::fs::fsync_dir(directory).map_err(io::Error::other)
    })
}

fn retain_with_sync(
    store: &Path,
    group: &str,
    key: &str,
    source: &Path,
    immutable: bool,
    sync_directory: impl Fn(&Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
    private_dir(store)?;
    let target = path(store, group, key)?;
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::other("No pool directory"))?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(target.with_extension("lock"))?;
    lock.lock()?;
    if target.exists() {
        verify(&target, key)?;
        if target.metadata()?.permissions().mode() & 0o222 != 0 {
            return Err(io::Error::other("A shared object became writable"));
        }
        // A previous attempt can fail after rename but before directory sync.
        // Reuse must finish that publication before promising a durable pin.
        sync_directory(parent)?;
        return Ok(target);
    }
    let stage = Pending(temporary(&target)?);
    let hardlinked = immutable
        && source.metadata()?.permissions().mode() & 0o333 == 0
        && std::fs::hard_link(source, &stage.0).is_ok();
    if !hardlinked {
        pigeon_sys::snapshot::copy(source, &stage.0)?;
        std::fs::set_permissions(&stage.0, std::fs::Permissions::from_mode(0o444))?;
    }
    verify(&stage.0, key)?;
    sync_object(&stage.0)?;
    std::fs::rename(&stage.0, &target)?;
    sync_directory(parent)?;
    Ok(target)
}

pub(crate) fn same_inode(a: &Path, b: &Path) -> bool {
    match (a.symlink_metadata(), b.symlink_metadata()) {
        (Ok(a), Ok(b)) => a.is_file() && b.is_file() && a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// Atomically replaces a private reference by a hard link to immutable bytes.
/// Permission changes apply only to this repository's directory, never the file.
pub(crate) fn relink(source: &Path, target: &Path) -> io::Result<()> {
    if same_inode(source, target) {
        return Ok(());
    }
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::other("No reference directory"))?;
    std::fs::create_dir_all(parent)?;
    let permissions = parent.metadata()?.permissions();
    std::fs::set_permissions(
        parent,
        std::fs::Permissions::from_mode(permissions.mode() | 0o200),
    )?;
    let result = (|| {
        let stage = Pending(temporary(target)?);
        std::fs::hard_link(source, &stage.0)?;
        std::fs::rename(&stage.0, target)?;
        pigeon_sys::fs::fsync_dir(parent).map_err(io::Error::other)
    })();
    std::fs::set_permissions(parent, permissions)?;
    result
}

#[cfg(test)]
#[path = "shared_pool_tests.rs"]
mod tests;

/// Release only this group's pool name after the caller proves every
/// retaining reference to these bytes released. Other groups and active
/// reader/annex hard links keep their own names; this is not secure wiping.
pub(crate) fn release(store: &Path, group: &str, key: &str) -> io::Result<()> {
    let target = path(store, group, key)?;
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::other("No pool parent"))?;
    if !parent.exists() {
        return Ok(());
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(target.with_extension("lock"))?;
    lock.lock()?;
    match std::fs::remove_file(&target) {
        Ok(()) => pigeon_sys::fs::fsync_dir(parent).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "shared_pool_recovery_tests.rs"]
mod recovery_tests;
