//! Sparse bridge working trees contain only pending changes. A hide journal
//! makes removal of accepted files replayable before capture can mistake a
//! crash for a deletion. Content is already pinned in the immutable pool.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{group::Group, shared_pool as pool};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    format: u32,
    paths: BTreeMap<String, String>,
}

fn journal(g: &Group) -> PathBuf {
    g.root().join("shared-hide.json")
}

/// Complete a hide before capture, including when the file is already absent.
pub(crate) async fn recover(g: &Group) -> Result<(), String> {
    let Some(storage) = &g.shared else {
        return Ok(());
    };
    let body = match std::fs::read(journal(g)) {
        Ok(body) => body,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    let saved: Journal = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    if saved.format != 1 {
        return Err("Unsupported shared hide journal".into());
    }
    let wt = g.worktree.as_ref().ok_or("No client working tree")?;
    let index = g.git.index().await.map_err(|e| e.to_string())?;
    for (path, key) in &saved.paths {
        if !crate::bridge::valid_path(path) {
            return Err("Invalid shared hide path".into());
        }
        let entry = index.get(path).ok_or("A hiding file left the index")?;
        let blob = g.git.object(&entry.oid).await.map_err(|e| e.to_string())?;
        if crate::git::pointer_key(&blob).as_ref() != Some(key) {
            return Err("A newer index version conflicts with shared hide recovery".into());
        }
        let object = pool::path(&storage.store, &g.id, key).map_err(|e| e.to_string())?;
        pool::verify(&object, key).map_err(|e| e.to_string())?;
        let file = wt.join(path);
        if file.exists() {
            pool::verify(&file, key)
                .map_err(|_| "A newer local save blocks shared hide recovery")?;
        }
        g.git
            .run(&["update-index", "--skip-worktree", "--", path])
            .await
            .map_err(|e| e.to_string())?;
        pigeon_sys::lab::crash_point("bridge-hide-after-flag");
        if file.exists() {
            std::fs::remove_file(&file).map_err(|e| e.to_string())?;
            if let Some(parent) = file.parent() {
                pigeon_sys::fs::fsync_dir(parent).map_err(|e| e.to_string())?;
            }
        }
        pigeon_sys::lab::crash_point("bridge-hide-after-remove");
        let mut parent = file.parent();
        while let Some(directory) = parent.filter(|p| *p != wt) {
            if std::fs::remove_dir(directory).is_err() {
                break;
            }
            parent = directory.parent();
        }
    }
    let hidden = crate::evict::hidden_paths(g).await;
    crate::evict::write_sparse(g, &hidden);
    std::fs::remove_file(journal(g)).map_err(|e| e.to_string())?;
    pigeon_sys::fs::fsync_dir(&g.root()).map_err(|e| e.to_string())
}

/// Unhide exactly the explicit save/delete paths before normal capture. The
/// enclosing bridge apply journal is already durable and replays after a crash.
pub(crate) async fn unhide(g: &Group, paths: &[String]) -> Result<(), String> {
    if g.shared.is_none() {
        return Ok(());
    }
    let index = g.git.index().await.map_err(|e| e.to_string())?;
    let mut hidden = crate::evict::hidden_paths(g).await;
    for path in paths {
        if !crate::bridge::valid_path(path) {
            return Err("Invalid shared save path".into());
        }
        if index.contains_key(path) {
            g.git
                .run(&["update-index", "--no-skip-worktree", "--", path])
                .await
                .map_err(|e| e.to_string())?;
        }
        hidden.remove(path);
    }
    crate::evict::write_sparse(g, &hidden);
    Ok(())
}

pub(crate) async fn compact(g: &Group) -> Result<(), String> {
    let Some(storage) = &g.shared else {
        return Ok(());
    };
    recover(g).await?;
    let wt = g.worktree.as_ref().ok_or("No client working tree")?;
    let mut table = crate::conflicts_file::read(g).ok_or("Cannot read client overlays")?;
    let old = table.clone();
    for (path, divergence) in &mut table.paths {
        let Some(key) = &divergence.mine else {
            continue;
        };
        let file = wt.join(path);
        if !file.exists() {
            return Err("A pending client file is missing".into());
        }
        if !crate::shared::unchanged(g, path, divergence) {
            return Err("A pending client file has an uncaptured save".into());
        }
        let object = pool::path(&storage.store, &g.id, key).map_err(|e| e.to_string())?;
        // Execution permission is per path; keep a private executable overlay
        // rather than changing a shared inode's permissions.
        if object.exists()
            && file
                .metadata()
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o111
                == 0
        {
            pool::relink(&object, &file).map_err(|e| e.to_string())?;
            divergence.mine_stat = crate::evict::stat_of(&file)
                .map(|s| [s.dev.into(), s.ino.into(), s.size.into(), s.mtime_ns]);
        }
    }
    if table != old {
        crate::conflicts_file::write(g, &table)?;
    }
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return Ok(());
    };
    let keys = g.git.keys(&main).await.map_err(|e| e.to_string())?;
    let mut paths = BTreeMap::new();
    for (path, key) in keys {
        if table.paths.contains_key(&path) || !crate::bridge::valid_path(&path) {
            continue;
        }
        let file = wt.join(&path);
        if file.is_dir() {
            std::fs::remove_dir(&file)
                .map_err(|_| "Uncaptured local files block shared compaction")?;
            continue;
        }
        if !file.exists() {
            continue;
        }
        pool::verify(&file, &key)
            .map_err(|_| "An uncaptured local save blocks client compaction")?;
        let object = pool::path(&storage.store, &g.id, &key).map_err(|e| e.to_string())?;
        pool::verify(&object, &key).map_err(|e| e.to_string())?;
        paths.insert(path, key);
    }
    if !paths.is_empty() {
        pigeon_sys::fs::write_atomic(
            &journal(g),
            &serde_json::to_vec(&Journal { format: 1, paths }).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        recover(g).await?;
    }
    // Empty directories carry no file bodies and remain bridge DAV metadata.
    Ok(())
}

/// Accepted remote files stay hidden even when no explicit free target exists.
pub(crate) fn targets(g: &Group) -> pigeon_core::keep_targets::Targets {
    if g.shared.is_some() {
        pigeon_core::keep_targets::Targets([(String::new(), true)].into())
    } else {
        crate::evict::targets(g)
    }
}
