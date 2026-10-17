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
        Ok(cached_time < self.mtime
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

fn read_selected(g: &Group, wanted: Option<&str>) -> Result<BTreeMap<String, Index>, String> {
    let file = g.git_dir.join("index");
    let before = pigeon_sys::fs::lstat(&file).map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
    if before != pigeon_sys::fs::lstat(&file).map_err(|e| e.to_string())? {
        return Err("index changed during observation".to_owned());
    }
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
    let mtime = before.ok_or("index disappeared")?.mtime_ns;
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
