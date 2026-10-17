//! Files: stat tuples, renames that never replace a name, durable writes and
//! appends, and reads that leave the page cache alone (07 §7.5, 11 §11.3,
//! ADR 0037, spec 07 §7.5).

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rustix::fs::{CWD, RenameFlags, renameat_with};

use crate::{Error, Result};

/// A file's identity: its device and inode numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId {
    /// The device number (`st_dev`).
    pub dev: u64,
    /// The inode number (`st_ino`).
    pub ino: u64,
}

/// Native metadata for live save/restore preconditions and observations.
/// The durable journal retains its `(dev, ino, size, mtime in ns)` tuple
/// (spec 07 §7.5, 11 §11.3); ctime also detects preserved-mtime saves here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stat {
    /// Device and inode.
    pub id: FileId,
    /// Size in bytes.
    pub size: u64,
    /// Modification time in nanoseconds since the epoch.
    pub mtime_ns: i64,
    /// Inode-change time, including same-size writes that preserve mtime.
    pub ctime_ns: i64,
    /// Number of hard links.
    pub nlink: u64,
    /// The file type and permission bits (`st_mode`).
    pub mode: u32,
    /// The owner's user ID.
    pub uid: u32,
    /// The file's group ID.
    pub gid: u32,
}

impl Stat {
    /// The tuple the install journal records: `[dev, ino, size, mtime_ns]`.
    pub fn tuple(&self) -> [i128; 4] {
        [
            i128::from(self.id.dev),
            i128::from(self.id.ino),
            i128::from(self.size),
            i128::from(self.mtime_ns),
        ]
    }

    /// Whether this is a regular file (not a link, folder or device).
    pub fn is_regular(&self) -> bool {
        self.mode & 0o170000 == 0o100000
    }

    /// Whether the owner's execute bit is set (the executable bit, spec 07 §7.9).
    pub fn is_executable(&self) -> bool {
        self.mode & 0o100 != 0
    }

    fn of(meta: &std::fs::Metadata) -> Stat {
        Stat {
            id: FileId {
                dev: meta.dev(),
                ino: meta.ino(),
            },
            size: meta.size(),
            mtime_ns: meta
                .mtime()
                .saturating_mul(1_000_000_000)
                .saturating_add(meta.mtime_nsec()),
            ctime_ns: meta
                .ctime()
                .saturating_mul(1_000_000_000)
                .saturating_add(meta.ctime_nsec()),
            nlink: meta.nlink(),
            mode: meta.mode(),
            uid: meta.uid(),
            gid: meta.gid(),
        }
    }
}

/// `lstat`: the path's own stat, not following a symbolic link; `None` when
/// nothing has the name.
pub fn lstat(path: &Path) -> Result<Option<Stat>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(Stat::of(&meta))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::path("lstat", path, e)),
    }
}

/// The stat of an open file.
pub fn fstat(file: &File) -> Result<Stat> {
    file.metadata()
        .map(|m| Stat::of(&m))
        .map_err(|e| Error::op("fstat", e))
}

/// What a rename that never replaces did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Renamed {
    /// The file now has the new name.
    Done,
    /// Something already had the new name; nothing moved.
    TargetExists,
    /// Nothing had the old name; nothing moved.
    SourceMissing,
}

/// The space of a file system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Space {
    /// Bytes an unprivileged writer may still use.
    pub available: u64,
    /// The file system's size in bytes.
    pub total: u64,
}

/// The space of the file system that holds `path` (`statvfs`), which
/// pigeon checks itself before it receives content (08 §8.9).
pub fn space(path: &Path) -> Result<Space> {
    let s = rustix::fs::statvfs(path).map_err(|e| Error::path("statvfs", path, e.into()))?;
    Ok(Space {
        available: s.f_bavail.saturating_mul(s.f_frsize),
        total: s.f_blocks.saturating_mul(s.f_frsize),
    })
}

/// Renames `from` to `to` unless something has the name `to`: Linux
/// `renameat2(RENAME_NOREPLACE)`, macOS `renamex_np(RENAME_EXCL)`. pigeon
/// never renames over a name (spec 07 §7.5).
pub fn rename_noreplace(from: &Path, to: &Path) -> Result<Renamed> {
    match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
        Ok(()) => {
            fsync_dir(&parent_of(from))?;
            if parent_of(from) != parent_of(to) {
                fsync_dir(&parent_of(to))?;
            }
            Ok(Renamed::Done)
        }
        Err(rustix::io::Errno::EXIST) => Ok(Renamed::TargetExists),
        Err(rustix::io::Errno::NOENT) if lstat(from)?.is_none() => Ok(Renamed::SourceMissing),
        Err(e) => Err(Error::path("rename-noreplace", to, e.into())),
    }
}

/// Swaps the names `a` and `b` atomically: Linux `RENAME_EXCHANGE`, macOS
/// `RENAME_SWAP`. Both must exist.
pub fn rename_exchange(a: &Path, b: &Path) -> Result<()> {
    renameat_with(CWD, a, CWD, b, RenameFlags::EXCHANGE)
        .map_err(|e| Error::path("rename-exchange", b, e.into()))
}

/// Flushes a file's data and metadata to stable storage. On macOS the
/// standard library uses `F_FULLFSYNC`, which also empties the disk's cache.
pub fn fsync_file(file: &File) -> Result<()> {
    file.sync_all().map_err(|e| Error::op("fsync", e))
}

/// Flushes a folder, so that names created, removed or renamed in it survive
/// a power cut.
pub fn fsync_dir(dir: &Path) -> Result<()> {
    let handle = File::open(dir).map_err(|e| Error::path("open-dir", dir, e))?;
    #[cfg(target_vendor = "apple")]
    if rustix::fs::fcntl_fullfsync(&handle).is_ok() {
        return Ok(());
    }
    rustix::fs::fsync(&handle).map_err(|e| Error::path("fsync-dir", dir, e.into()))
}

/// The folder holding `path`, or `.` for a bare name.
fn parent_of(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Replaces `path` with `bytes` atomically and durably: a temporary file in
/// the same folder, fsynced, renamed over `path`, then the folder fsynced.
/// For pigeon's own state files only (11 §11.3); never for a person's file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = parent_of(path);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    let result = (|| {
        let mut file = File::create(&tmp).map_err(|e| Error::path("create", &tmp, e))?;
        file.write_all(bytes)
            .map_err(|e| Error::path("write", &tmp, e))?;
        fsync_file(&file)?;
        std::fs::rename(&tmp, path).map_err(|e| Error::path("rename", path, e))?;
        fsync_dir(&dir)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Appends `line` and a newline to `path` and fsyncs it before returning, as
/// the install journal requires before the step it announces (11 §11.3). The
/// file is created, and its folder fsynced, if it did not exist.
pub fn append_durable(path: &Path, line: &[u8]) -> Result<()> {
    let existed = lstat(path)?.is_some();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::path("open-append", path, e))?;
    let mut buf = Vec::with_capacity(line.len() + 1);
    buf.extend_from_slice(line);
    buf.push(b'\n');
    file.write_all(&buf)
        .map_err(|e| Error::path("append", path, e))?;
    file.sync_data()
        .map_err(|e| Error::path("fsync", path, e))?;
    if !existed {
        fsync_dir(&parent_of(path))?;
    }
    Ok(())
}

/// Opens a file for one streaming read that should not evict others' pages:
/// on macOS with `F_NOCACHE`; on Linux call [`forget_pages`] after reading
/// (ADR 0037).
pub fn open_uncached(path: &Path) -> Result<File> {
    let file = File::open(path).map_err(|e| Error::path("open", path, e))?;
    #[cfg(target_vendor = "apple")]
    rustix::fs::fcntl_nocache(&file, true).map_err(|e| Error::path("nocache", path, e.into()))?;
    Ok(file)
}

/// A bounded streaming read that advises completed cache pages on Linux.
/// On macOS, the file is opened with F_NOCACHE instead. This does not keep
/// content bytes or wait until a large file's complete hash is finished.
pub struct StreamingRead {
    file: File,
    #[cfg(target_os = "linux")]
    position: u64,
    #[cfg(target_os = "linux")]
    forgotten: u64,
    #[cfg(target_os = "linux")]
    page: u64,
}

/// Open an ordinary streaming read with native page-cache advice (ADR 0037).
pub fn streaming_read(path: &Path) -> Result<StreamingRead> {
    let file = open_uncached(path)?;
    #[cfg(target_os = "linux")]
    let page = {
        // SAFETY: sysconf reads a scalar system parameter and takes no pointer.
        let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        u64::try_from(size)
            .ok()
            .filter(|size| *size > 0)
            .ok_or_else(|| Error::op("page-size", io::Error::other("unknown native page size")))?
    };
    Ok(StreamingRead {
        file,
        #[cfg(target_os = "linux")]
        position: 0,
        #[cfg(target_os = "linux")]
        forgotten: 0,
        #[cfg(target_os = "linux")]
        page,
    })
}

impl Read for StreamingRead {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.file.read(bytes)?;
        #[cfg(target_os = "linux")]
        {
            self.position = self.position.saturating_add(count as u64);
            let completed = self.position / self.page * self.page;
            // A real EOF can also release the final partial page. Empty
            // buffer reads are not EOF, and must advise no unread bytes.
            let end = if count == 0 && !bytes.is_empty() && completed < self.position {
                completed.saturating_add(self.page)
            } else {
                completed
            };
            if let Some(length) = std::num::NonZeroU64::new(end - self.forgotten) {
                rustix::fs::fadvise(
                    &self.file,
                    self.forgotten,
                    Some(length),
                    rustix::fs::Advice::DontNeed,
                )?;
                self.forgotten = completed;
            }
        }
        Ok(count)
    }
}

/// Tells the kernel that pigeon will not read this file's pages again:
/// `posix_fadvise(POSIX_FADV_DONTNEED)` on Linux; nothing on macOS, where
/// [`open_uncached`] already kept them out (ADR 0037).
pub fn forget_pages(file: &File) -> Result<()> {
    #[cfg(target_os = "linux")]
    rustix::fs::fadvise(file, 0, None, rustix::fs::Advice::DontNeed)
        .map_err(|e| Error::op("fadvise", e))?;
    #[cfg(not(target_os = "linux"))]
    let _ = file;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_streaming_reads_preserve_every_byte_and_advise_completed_pages() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("content.bin");
        let expected: Vec<u8> = (0..1_048_613).map(|n| (n % 251) as u8).collect();
        std::fs::write(&path, &expected).unwrap();
        let mut reader = streaming_read(&path).unwrap();
        let mut actual = Vec::new();
        let mut chunk = [0; 257];
        loop {
            assert_eq!(reader.read(&mut []).unwrap(), 0);
            let count = reader.read(&mut chunk).unwrap();
            if count == 0 {
                break;
            }
            actual.extend_from_slice(&chunk[..count]);
            #[cfg(target_os = "linux")]
            assert!(
                reader.position - reader.forgotten < reader.page,
                "completed pages must not wait until the whole file is read"
            );
        }
        assert_eq!(actual, expected);
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert_eq!(reader.read(&mut chunk).unwrap(), 0);
    }

    #[test]
    fn noreplace_moves_onto_a_free_name_only() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        std::fs::write(&a, b"one").unwrap();
        std::fs::write(&b, b"two").unwrap();
        assert_eq!(rename_noreplace(&a, &b).unwrap(), Renamed::TargetExists);
        assert_eq!(std::fs::read(&b).unwrap(), b"two");
        std::fs::remove_file(&b).unwrap();
        assert_eq!(rename_noreplace(&a, &b).unwrap(), Renamed::Done);
        assert_eq!(std::fs::read(&b).unwrap(), b"one");
        std::fs::remove_file(&b).unwrap();
        assert_eq!(rename_noreplace(&a, &b).unwrap(), Renamed::SourceMissing);
    }

    #[test]
    fn exchange_swaps_inodes() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        std::fs::write(&a, b"one").unwrap();
        std::fs::write(&b, b"two").unwrap();
        let (ia, ib) = (
            lstat(&a).unwrap().unwrap().id,
            lstat(&b).unwrap().unwrap().id,
        );
        rename_exchange(&a, &b).unwrap();
        assert_eq!(lstat(&a).unwrap().unwrap().id, ib);
        assert_eq!(lstat(&b).unwrap().unwrap().id, ia);
        assert_eq!(std::fs::read(&a).unwrap(), b"two");
    }

    #[test]
    fn durable_writes_and_appends() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.json");
        write_atomic(&p, b"{}").unwrap();
        write_atomic(&p, b"{\"a\":1}").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"{\"a\":1}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        let j = dir.path().join("install.ndjson");
        append_durable(&j, b"{\"seq\":1}").unwrap();
        append_durable(&j, b"{\"seq\":2}").unwrap();
        assert_eq!(
            std::fs::read_to_string(&j).unwrap(),
            "{\"seq\":1}\n{\"seq\":2}\n"
        );
        fsync_dir(dir.path()).unwrap();
    }

    #[test]
    fn stat_reads_kind_and_identity() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        assert_eq!(lstat(&p).unwrap(), None);
        std::fs::write(&p, b"12345").unwrap();
        let st = lstat(&p).unwrap().unwrap();
        assert!(st.is_regular() && st.size == 5 && st.nlink == 1);
        let link = dir.path().join("l");
        std::os::unix::fs::symlink(&p, &link).unwrap();
        assert!(!lstat(&link).unwrap().unwrap().is_regular());
        let f = open_uncached(&p).unwrap();
        assert_eq!(fstat(&f).unwrap().id, st.id);
        forget_pages(&f).unwrap();
    }
}
