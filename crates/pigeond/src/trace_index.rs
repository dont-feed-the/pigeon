//! Read Git's on-disk index without running a command that refreshes it.
//! In particular ls-files can clear a visible file's skip-worktree bit;
//! an observation must not perform the transition it is meant to check.

use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::group::Group;

/// The pointer and cache data actually stored by Git for one path.
#[derive(Clone)]
pub(crate) struct Index {
    pub oid: Option<String>,
    pub hidden: bool,
    cache: Option<[u32; 10]>,
    mtime: i64,
}

impl Index {
    pub(crate) fn entry(&self) -> Option<pigeon_core::authority::Entry> {
        let mode = format!("{:o}", self.cache?[6]);
        Some(pigeon_core::authority::Entry {
            kind: if mode == "160000" { "commit" } else { "blob" }.to_owned(),
            mode,
            oid: self.oid.clone()?,
        })
    }

    pub fn clean(&self, file: &Path) -> Result<bool, String> {
        let stat = match std::fs::symlink_metadata(file) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(self.oid.is_none()),
            Err(e) => return Err(e.to_string()),
        };
        let Some(c) = self.cache else {
            return Ok(false);
        };
        let cached_time = i64::from(c[2]) * 1_000_000_000 + i64::from(c[3]);
        let cached_change = i64::from(c[0]) * 1_000_000_000 + i64::from(c[1]);
        Ok(cached_time.max(cached_change) < self.mtime
            && u64::from(c[0]) == stat.ctime() as u64
            && u64::from(c[1]) == stat.ctime_nsec() as u64
            && u64::from(c[2]) == stat.mtime() as u64
            && u64::from(c[3]) == stat.mtime_nsec() as u64
            && c[4] == stat.dev() as u32
            && c[5] == stat.ino() as u32
            && c[9] == stat.size() as u32)
    }
}

fn take<'a>(bytes: &'a [u8], at: &mut usize, count: usize) -> Result<&'a [u8], String> {
    let end = at.checked_add(count).ok_or("index offset overflow")?;
    let value = bytes.get(*at..end).ok_or("truncated index")?;
    *at = end;
    Ok(value)
}

fn integer(bytes: &[u8], at: &mut usize) -> Result<u32, String> {
    let value = take(bytes, at, 4)?;
    Ok(u32::from_be_bytes(
        value.try_into().map_err(|_| "bad index integer")?,
    ))
}

/// Formats 2/3 use padded paths; format 4 uses Git's prefix-compressed paths.
pub(crate) fn read(g: &Group, path: &str) -> Result<Index, String> {
    Ok(read_selected(g, Some(path))?.remove(path).unwrap_or(Index {
        oid: None,
        hidden: false,
        cache: None,
        mtime: 0,
    }))
}

/// One immutable index reading for the whole background scan. No Git
/// clean filter runs, even for a same-size edit or a racy stat cache.
pub(crate) fn read_all(g: &Group) -> Result<BTreeMap<String, Index>, String> {
    read_selected(g, None)
}

#[derive(Debug, PartialEq, Eq)]
enum SnapshotError {
    Changed,
    Other(String),
}

impl SnapshotError {
    fn detail(self) -> String {
        match self {
            Self::Changed => "index changed during observation".to_owned(),
            Self::Other(detail) => detail,
        }
    }
}

fn index_snapshot(file: &Path) -> Result<(pigeon_sys::fs::Stat, Vec<u8>), SnapshotError> {
    let other = |error: String| SnapshotError::Other(error);
    let before = pigeon_sys::fs::lstat(file).map_err(|e| other(e.to_string()))?;
    let bytes = std::fs::read(file).map_err(|e| other(e.to_string()))?;
    if before != pigeon_sys::fs::lstat(file).map_err(|e| other(e.to_string()))? {
        return Err(SnapshotError::Changed);
    }
    Ok((
        before.ok_or_else(|| other("index disappeared".into()))?,
        bytes,
    ))
}

fn stable_snapshot<T>(mut observe: impl FnMut() -> Result<T, SnapshotError>) -> Result<T, String> {
    // Git replaces index.lock atomically. A concurrent read is not corrupt;
    // restart the complete fenced observation, at most twice. All other
    // failures and persistent instability remain errors, never empty data.
    let mut retries = 2;
    loop {
        match observe() {
            Err(SnapshotError::Changed) if retries > 0 => {
                retries -= 1;
                tracing::debug!(remaining = retries, "index-observation-retried");
            }
            result => return result.map_err(SnapshotError::detail),
        }
    }
}

fn read_selected(g: &Group, wanted: Option<&str>) -> Result<BTreeMap<String, Index>, String> {
    let file = g.git_dir.join("index");
    let (before, bytes) = stable_snapshot(|| index_snapshot(&file))?;
    let mut at = 0;
    if take(&bytes, &mut at, 4)? != b"DIRC" {
        return Err("invalid index signature".into());
    }
    let version = integer(&bytes, &mut at)?;
    if !(2..=4).contains(&version) {
        return Err("unsupported index version".into());
    }
    let count = integer(&bytes, &mut at)?;
    let mut previous = Vec::new();
    let mtime = before.mtime_ns;
    let mut result = BTreeMap::new();
    for _ in 0..count {
        let start = at;
        let mut cache = [0; 10];
        for n in &mut cache {
            *n = integer(&bytes, &mut at)?;
        }
        let oid = crate::util::hex(take(&bytes, &mut at, 20)?);
        let flags = u16::from_be_bytes(
            take(&bytes, &mut at, 2)?
                .try_into()
                .map_err(|_| "bad flags")?,
        );
        let extended = if flags & 0x4000 != 0 {
            if version == 2 {
                return Err("extended flags in index version 2".into());
            }
            u16::from_be_bytes(
                take(&bytes, &mut at, 2)?
                    .try_into()
                    .map_err(|_| "bad extended flags")?,
            )
        } else {
            0
        };
        let drop = if version == 4 {
            let mut b = take(&bytes, &mut at, 1)?[0];
            let mut n = usize::from(b & 0x7f);
            while b & 0x80 != 0 {
                b = take(&bytes, &mut at, 1)?[0];
                n = n
                    .checked_add(1)
                    .and_then(|v| v.checked_mul(128))
                    .and_then(|v| v.checked_add(usize::from(b & 0x7f)))
                    .ok_or("index prefix overflow")?;
            }
            n
        } else {
            previous.len()
        };
        let keep = previous
            .len()
            .checked_sub(drop)
            .ok_or("invalid index prefix")?;
        previous.truncate(keep);
        let len = bytes
            .get(at..)
            .and_then(|v| v.iter().position(|b| *b == 0))
            .ok_or("unterminated index path")?;
        previous.extend_from_slice(take(&bytes, &mut at, len)?);
        take(&bytes, &mut at, 1)?;
        if version != 4 {
            let padding = (8 - (at - start) % 8) % 8;
            take(&bytes, &mut at, padding)?;
        }
        if wanted.is_none_or(|path| previous == path.as_bytes()) {
            if flags & 0x3000 != 0 {
                return Err("unmerged index entry".into());
            }
            let path = String::from_utf8(previous.clone()).map_err(|_| "non-UTF-8 index path")?;
            if result
                .insert(
                    path,
                    Index {
                        oid: Some(oid),
                        hidden: extended & 0x4000 != 0,
                        cache: Some(cache),
                        mtime,
                    },
                )
                .is_some()
            {
                return Err("duplicate index path".into());
            }
        }
    }
    let end = bytes.len().checked_sub(20).ok_or("index has no checksum")?;
    while at < end {
        let signature = take(&bytes, &mut at, 4)?;
        if !signature[0].is_ascii_uppercase() {
            return Err("required index extension is outside the trace projection".into());
        }
        let size = integer(&bytes, &mut at)? as usize;
        take(&bytes, &mut at, size)?;
    }
    if at != end {
        return Err("invalid index extension length".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_index_replacement_gets_a_fresh_complete_read() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let file = temp.path().join("index");
        std::fs::write(&file, b"fresh complete bytes")?;
        let mut observations = 0;
        let result = stable_snapshot(|| {
            observations += 1;
            if observations <= 2 {
                Err(SnapshotError::Changed)
            } else {
                index_snapshot(&file)
            }
        });
        let (_, bytes) = result?;
        assert_eq!(bytes, b"fresh complete bytes");
        assert_eq!(observations, 3);
        Ok(())
    }

    #[test]
    fn persistent_changes_are_bounded_and_other_errors_are_not_retried() {
        let mut observations = 0;
        let result: Result<(), _> = stable_snapshot(|| {
            observations += 1;
            Err(SnapshotError::Changed)
        });
        assert_eq!(result, Err("index changed during observation".into()));
        assert_eq!(observations, 3);
        observations = 0;
        let result: Result<(), _> = stable_snapshot(|| {
            observations += 1;
            Err(SnapshotError::Other("unreadable".into()))
        });
        assert_eq!(result, Err("unreadable".into()));
        assert_eq!(observations, 1);
    }

    #[test]
    fn preserved_mtime_does_not_make_a_racy_ctime_clean() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let file = temp.path().join("saved.txt");
        std::fs::write(&file, b"captured bytes")?;
        std::fs::File::options()
            .write(true)
            .open(&file)?
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1))?;
        let stat = std::fs::symlink_metadata(&file)?;
        let ctime = stat.ctime() * 1_000_000_000 + stat.ctime_nsec();
        let mut index = Index {
            oid: Some("a".repeat(40)),
            hidden: false,
            cache: Some([
                stat.ctime() as u32,
                stat.ctime_nsec() as u32,
                stat.mtime() as u32,
                stat.mtime_nsec() as u32,
                stat.dev() as u32,
                stat.ino() as u32,
                stat.mode(),
                stat.uid(),
                stat.gid(),
                stat.size() as u32,
            ]),
            mtime: ctime,
        };
        assert!(
            !index.clean(&file)?,
            "a later same-tick save can preserve the cached old mtime and ctime"
        );
        index.mtime = ctime + 1;
        assert!(index.clean(&file)?, "an older complete stat remains usable");
        Ok(())
    }
}
