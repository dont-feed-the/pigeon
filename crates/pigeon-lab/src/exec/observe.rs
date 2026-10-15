//! What the executor reads from a device's disk: file bytes, pointer files,
//! git-annex objects and partial transfers, and disk use by part (S11).
//!
//! git-annex names an object file by its key, whose size and hash fields
//! (`-s<len>--<sha256>`) the executor knows from the content it wrote, so it
//! finds objects without knowing which backend pigeon chose.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::world::hex;

/// The SHA-256 of a file, or `None` if it is not a regular file.
pub fn sha256_file(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    let mut f = fs::File::open(path).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(hex(&h.finalize()))
}

/// [`sha256_file`] once `path` holds a file: an install, or a re-thin,
/// moves a file aside for an instant (07 §7.5), so an absence is looked at
/// again for up to a second before it counts.
pub fn sha256_settled(path: &Path) -> Option<String> {
    for _ in 0..5 {
        if let Some(s) = sha256_file(path) {
            return Some(s);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    None
}

/// Whether `path` is a git-annex pointer: a link into the annex, or a small
/// file that starts with `/annex/objects/`.
pub fn is_pointer(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if meta.file_type().is_symlink() {
        return fs::read_link(path).is_ok_and(|t| t.to_string_lossy().contains("annex/objects"));
    }
    if meta.file_type().is_file() && meta.len() < 2048 {
        return fs::read(path).is_ok_and(|b| b.starts_with(b"/annex/objects/"));
    }
    false
}

/// Every path under `dir`, `.git` excluded, links not followed.
pub fn walk(dir: &Path, skip_git: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if skip_git && e.file_name() == ".git" {
                continue;
            }
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p.clone());
            }
            out.push(p);
        }
    }
    out
}

/// The pointer files under `dir`.
pub fn pointers(dir: &Path) -> Vec<PathBuf> {
    walk(dir, true)
        .into_iter()
        .filter(|p| is_pointer(p))
        .collect()
}

/// Bytes of the regular files under `dir`, `.git` excluded: what it keeps.
pub fn kept(dir: &Path) -> u64 {
    walk(dir, true)
        .iter()
        .filter_map(|p| fs::symlink_metadata(p).ok())
        .filter(|m| m.file_type().is_file())
        .map(|m| m.len())
        .sum()
}

fn part(rel: &str) -> &'static str {
    if rel.contains("annex/objects/") {
        "annex"
    } else if rel.contains("annex/tmp/") {
        "annex-tmp"
    } else if rel == "ballast" {
        "ballast"
    } else if rel.contains("/.git/") || rel.starts_with(".git/") || rel.contains(".git/") {
        ".git"
    } else if rel.starts_with("home/.config")
        || rel.starts_with("home/.local")
        || rel.starts_with("home/.cache")
        || rel.starts_with("home/Library")
        || rel.starts_with("service/")
        || rel.starts_with("run/")
    {
        "state"
    } else {
        "folders"
    }
}

/// Disk use under `root` by part, in allocated bytes, each inode once.
pub fn usage(root: &Path) -> BTreeMap<String, u64> {
    let mut seen = BTreeSet::new();
    let mut parts: BTreeMap<String, u64> = BTreeMap::new();
    for p in walk(root, false) {
        let Ok(m) = fs::symlink_metadata(&p) else {
            continue;
        };
        if !m.file_type().is_file() || !seen.insert((m.dev(), m.ino())) {
            continue;
        }
        let rel = p
            .strip_prefix(root)
            .map(|r| r.to_string_lossy().into_owned())
            .unwrap_or_default();
        *parts.entry(part(&rel).to_owned()).or_default() += m.blocks() * 512;
    }
    parts
}

/// The fields of a key that name content of `len` bytes hashing to `sha`.
pub fn key_fields(len: u64, sha: &str) -> String {
    format!("-s{len}--{sha}")
}

/// Whether some git-annex object under `root` is the content `fields` names,
/// complete (`annex/objects`) or partial (`annex/tmp`, non-empty).
pub fn object(root: &Path, fields: &str, partial: bool) -> bool {
    let dir = if partial {
        "annex/tmp/"
    } else {
        "annex/objects/"
    };
    walk(root, false).iter().any(|p| {
        let s = p.to_string_lossy();
        s.contains(dir)
            && p.file_name()
                .is_some_and(|n| n.to_string_lossy().contains(fields))
            && fs::symlink_metadata(p)
                .is_ok_and(|m| m.file_type().is_file() && (!partial || m.len() > 0))
    })
}

/// The git directories of repositories under `root` that have an annex.
pub fn annexed_repositories(root: &Path) -> Vec<PathBuf> {
    walk(root, false)
        .into_iter()
        .filter(|p| p.file_name().is_some_and(|n| n == "annex"))
        .filter_map(|p| p.parent().map(Path::to_path_buf))
        .filter(|g| g.join("HEAD").exists() && g.join("objects").is_dir())
        .collect()
}
