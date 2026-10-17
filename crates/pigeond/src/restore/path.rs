//! Resolve deleted nested paths under the real worktree. Never traverse
//! a symlink or nested repository while restoring (07 §7.9, ADR 0034).

use crate::group::Group;
use std::path::{Component, Path};

pub(crate) fn relative(g: &Group, path: &Path) -> Option<String> {
    let root = g.worktree.as_ref()?;
    let path = std::path::absolute(path).ok()?;
    let rel = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<_>> = rel
        .components()
        .map(|part| match part {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    let rel = parts?.join("/");
    if !rel.is_empty()
        && (pigeon_core::names::portable(&rel).is_err() || crate::git::is_pigeon_path(&rel))
    {
        return None;
    }
    parents_clear(g, &rel).then_some(rel)
}

pub(crate) fn parents_clear(g: &Group, rel: &str) -> bool {
    let Some(root) = &g.worktree else {
        return false;
    };
    if !std::fs::metadata(root).is_ok_and(|m| m.is_dir()) {
        return false;
    }
    let mut current = root.clone();
    let parts: Vec<_> = rel.split('/').filter(|s| !s.is_empty()).collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "." || *part == ".." {
            return false;
        }
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => return false,
            Ok(meta) if meta.is_dir() => {
                if current.join(".git").exists() {
                    return false;
                }
            }
            Ok(meta) if i + 1 < parts.len() || !meta.is_file() => return false,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return false,
        }
    }
    true
}
