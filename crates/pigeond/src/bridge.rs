//! The bridge's local file API. A peer's view is its accepted catalogue with
//! captured divergences overlaid. Explicit saves compare that view, use the
//! repository lock, and are replayed from a durable journal before any loop
//! can interpret an interrupted batch. The bridge never writes Git directly.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use pigeon_proto::Response;
use pigeon_proto::bridge::{Change, File, Operation, Reply};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::daemon::Daemon;
use crate::group::Group;
use crate::net::sync::{key_sha256, sync_object};

type Result<T> = std::result::Result<T, (u16, String)>;

fn err(status: u16, message: impl ToString) -> (u16, String) {
    (status, message.to_string())
}

fn internal(e: impl ToString) -> (u16, String) {
    err(503, e)
}

/// Refuses traversal, internal directories, and nonportable file names.
pub fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.split('/').any(|c| matches!(c, ".git" | ".pigeon"))
        && path != ".gitattributes"
        && pigeon_core::names::portable(path).is_ok()
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    crate::net::sync::sha256_file(path)
}

fn executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

fn etag(key: &str, exec: bool) -> Result<String> {
    let hash = key_sha256(key).ok_or_else(|| internal("Invalid content key"))?;
    Ok(format!("{hash}-{}", u8::from(exec)))
}

/// The visible files with their annex keys; must hold `g.busy`.
async fn visible(g: &Group) -> Result<BTreeMap<String, (File, String)>> {
    let head = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| err(409, "No group history"))?;
    let tree = g.git.tree(&head).await.map_err(internal)?;
    let mut out = BTreeMap::new();
    let paths: Vec<_> = tree.keys().filter(|p| valid_path(p)).cloned().collect();
    let objects: Vec<_> = paths.iter().map(|p| tree[p].oid.clone()).collect();
    let blobs = g.git.objects(&objects).await.map_err(internal)?;
    for (path, blob) in paths.into_iter().zip(blobs) {
        let key = blob
            .and_then(|(_, data)| crate::git::pointer_key(&data))
            .ok_or_else(|| internal("A catalogue file has no content key"))?;
        let file = File {
            etag: etag(&key, tree[&path].mode == "100755")?,
            size: crate::restore::key_size(&key).ok_or_else(|| internal("Invalid file size"))?,
            path: path.clone(),
            diverging: false,
        };
        out.insert(path, (file, key));
    }
    let table = crate::conflicts_file::read(g).ok_or_else(|| internal("Cannot read conflicts"))?;
    for (path, divergence) in table.paths {
        if !valid_path(&path) {
            continue;
        }
        if let Some(key) = divergence.mine {
            let exec = g
                .worktree
                .as_ref()
                .is_some_and(|wt| executable(&wt.join(&path)));
            let file = File {
                etag: etag(&key, exec)?,
                size: crate::restore::key_size(&key)
                    .ok_or_else(|| internal("Invalid file size"))?,
                path: path.clone(),
                diverging: true,
            };
            out.insert(path, (file, key));
        } else {
            out.remove(&path);
        }
    }
    Ok(out)
}

fn revision(files: &BTreeMap<String, (File, String)>) -> String {
    let mut hash = Sha256::new();
    for (path, (file, _)) in files {
        hash.update(path.as_bytes());
        hash.update([0]);
        hash.update(file.etag.as_bytes());
        hash.update([0]);
        hash.update(file.size.to_be_bytes());
        hash.update([u8::from(file.diverging)]);
    }
    crate::util::hex(&hash.finalize())
}

#[path = "bridge_pages.rs"]
mod pages;

/// All bridge operations run behind the daemon's repository lock.
pub async fn handle(d: &Daemon, operation: Operation) -> Vec<Response> {
    let result = match perform(d, operation).await {
        Ok(r) => r,
        Err((status, message)) => Reply::Failure { status, message },
    };
    vec![Response::Bridge { result }]
}

async fn perform(d: &Daemon, operation: Operation) -> Result<Reply> {
    if let Operation::RegisterPool { group, store } = &operation {
        return crate::shared::register(d, group, Path::new(store))
            .await
            .map_err(internal);
    }
    if let Operation::ExportContent { group, key, store } = &operation {
        return crate::shared::export(d, group, key, Path::new(store))
            .await
            .map_err(internal);
    }
    let g = d
        .default_group()
        .ok_or_else(|| err(409, "This account has no Pigeon group"))?;
    // These read/update only advisory endpoint data. They must remain usable
    // while a file operation is waiting for an offline content holder.
    if let Operation::LearnPeers { peers } = &operation {
        for (device, addresses) in peers {
            if device != &d.device
                && device.len() == 64
                && device.bytes().all(|b| b.is_ascii_hexdigit())
            {
                let addresses: Vec<std::net::SocketAddr> =
                    addresses.iter().filter_map(|s| s.parse().ok()).collect();
                if !addresses.is_empty() {
                    g.set_addresses(device, addresses);
                }
            }
        }
        return Ok(Reply::Learned);
    }
    if matches!(operation, Operation::Info) {
        let doc = g.roster().ok_or_else(|| internal("No roster"))?;
        let own = doc
            .roster
            .devices
            .get(&pigeon_core::roster::DeviceId(d.device.clone()))
            .ok_or_else(|| err(403, "This peer is no longer a group member"))?;
        return Ok(Reply::Info {
            group: g.id.clone(),
            person: own.person.0.clone(),
            device: d.device.clone(),
            annex_uuid: g.lock().annex_uuid.clone().unwrap_or_default(),
            root: g
                .worktree
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            addresses: d.addresses().iter().map(ToString::to_string).collect(),
            shared: g.shared.is_some(),
        });
    }
    let _busy = g.busy.lock().await;
    if g.worktree.is_none() {
        return Err(err(
            409,
            "The bridge needs a working peer, not a backup machine",
        ));
    }
    if crate::evict::unsettled(&g) {
        return Err(err(503, "Pigeon is recovering an interrupted installation"));
    }
    recover(d, &g).await.map_err(internal)?;
    if !g.roster().is_some_and(|doc| {
        doc.roster
            .devices
            .contains_key(&pigeon_core::roster::DeviceId(d.device.clone()))
    }) {
        return Err(err(403, "This peer is no longer a group member"));
    }
    let captured = crate::capture::capture(&g, &d.device, &d.tracer)
        .await
        .map_err(internal)?;
    if captured.added + captured.removed + captured.suggested > 0 {
        crate::shared::compact(&g).await.map_err(internal)?;
    }
    let files = visible(&g).await?;
    match operation {
        Operation::Info
        | Operation::LearnPeers { .. }
        | Operation::ExportContent { .. }
        | Operation::RegisterPool { .. } => Err(internal("Unexpected identity operation")),
        Operation::Snapshot { cursor, limit } => pages::page(&files, cursor, limit),
        Operation::Read {
            path,
            etag,
            destination,
        } => {
            let (file, key) = files.get(&path).ok_or_else(|| err(404, "File not found"))?;
            if etag != file.etag {
                return Err(err(412, "The file changed; synchronize again"));
            }
            if !crate::shared::content_allowed(&g, key) {
                return Err(err(
                    503,
                    "Content waits for verified erasure and membership state",
                ));
            }
            crate::restore::bring(&g, key)
                .await
                .map_err(|_| err(503, "Waiting for file content"))?;
            let object = crate::custody::present(&g, key)
                .await
                .ok_or_else(|| err(503, "Content unavailable"))?;
            let destination = Path::new(&destination);
            if !destination.is_absolute() {
                return Err(err(400, "Staging paths must be absolute"));
            }
            if g.shared.is_some() {
                crate::shared_pool::verify(&object, key).map_err(internal)?;
                // The shared object is immutable. Pin it for the response or
                // MOVE/COPY without allocating another complete file body.
                if let Err(error) = std::fs::hard_link(&object, destination) {
                    if error.kind() != std::io::ErrorKind::CrossesDevices {
                        return Err(internal(error));
                    }
                    pigeon_sys::snapshot::copy(&object, destination).map_err(internal)?;
                }
                return Ok(Reply::Read);
            }
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(destination)
                .map_err(internal)?;
            let copied = (|| -> std::io::Result<()> {
                std::io::copy(&mut std::fs::File::open(object)?, &mut out)?;
                out.flush()?;
                if hash_file(destination)? != key_sha256(key).unwrap_or_default() {
                    return Err(std::io::Error::other("Content verification failed"));
                }
                Ok(())
            })();
            if let Err(e) = copied {
                let _ = std::fs::remove_file(destination);
                return Err(internal(e));
            }
            Ok(Reply::Read)
        }
        Operation::Apply {
            revision,
            expected,
            changes,
        } => {
            let rebased = expected.as_ref().is_some_and(|expected| {
                if changes.len() != 1 || expected.len() != 1 {
                    return false;
                }
                let path = match &changes[0] {
                    Change::Put { path, .. } | Change::Remove { path } => path,
                };
                expected.get(path).is_some_and(|before| {
                    before.as_deref() == files.get(path).map(|(f, _)| f.etag.as_str())
                })
            });
            if expected.is_some() && !rebased || revision != self::revision(&files) && !rebased {
                return Err(err(412, "The peer changed; synchronize again"));
            }
            prepare(&g, &files, &changes).await?;
            let suggested = recover(d, &g).await.map_err(internal)?;
            d.wake.notify_one();
            Ok(Reply::Applied {
                revision: self::revision(&visible(&g).await?),
                suggested,
            })
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Journal {
    format: u32,
    changes: Vec<Saved>,
}

#[derive(Serialize, Deserialize)]
struct Saved {
    path: String,
    before: Option<String>,
    after: Option<String>,
    staged: Option<PathBuf>,
    executable: bool,
    #[serde(default)]
    hidden: bool,
}

fn journal(g: &Group) -> PathBuf {
    g.root().join("bridge-apply.json")
}

/// Checks every path and stages the entire batch before changing the tree.
async fn prepare(
    g: &Group,
    files: &BTreeMap<String, (File, String)>,
    changes: &[Change],
) -> Result<()> {
    let wt = g
        .worktree
        .as_ref()
        .ok_or_else(|| err(409, "No working tree"))?;
    if changes.is_empty() || changes.len() > 4096 {
        return Err(err(400, "A batch needs between 1 and 4096 file changes"));
    }
    let mut paths = BTreeSet::new();
    let mut after: BTreeSet<_> = files.keys().cloned().collect();
    let mut bytes = 0u64;
    for change in changes {
        let path = match change {
            Change::Put { path, .. } | Change::Remove { path } => path,
        };
        if !valid_path(path) || !paths.insert(path.clone()) {
            return Err(err(400, "Invalid, reserved, or repeated file path"));
        }
        let mut parent = wt.clone();
        for part in path.split('/') {
            parent.push(part);
            if parent
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                return Err(err(403, "Symbolic links are not shared"));
            }
        }
        match change {
            Change::Remove { .. } => {
                after.remove(path);
            }
            Change::Put { staged, .. } => {
                let staged = Path::new(staged);
                if !staged.is_absolute()
                    || staged.starts_with(wt)
                    || !staged
                        .symlink_metadata()
                        .is_ok_and(|m| m.file_type().is_file())
                {
                    return Err(err(400, "Upload staging file is invalid"));
                }
                bytes = bytes.saturating_add(std::fs::metadata(staged).map_err(internal)?.len());
                after.insert(path.clone());
            }
        }
    }
    pigeon_core::names::portable_tree(after.iter().map(String::as_str))
        .map_err(|_| err(409, "File names collide on another platform"))?;
    for path in &after {
        let mut parent = Path::new(path).parent();
        while let Some(p) = parent {
            if after.contains(&p.to_string_lossy().into_owned()) {
                return Err(err(409, "A file cannot contain another file"));
            }
            parent = p.parent();
        }
    }
    let room = pigeon_sys::fs::space(&g.git_dir)
        .map_err(internal)?
        .available;
    if room
        < bytes
            .saturating_mul(3)
            .saturating_add(crate::settings::reserve_bytes(g.role, &g.git_dir))
    {
        return Err(err(507, "Not enough space to retain the upload safely"));
    }
    let stage = g.root().join("bridge-stage");
    if stage.exists() {
        std::fs::remove_dir_all(&stage).map_err(internal)?;
    }
    std::fs::create_dir_all(&stage).map_err(internal)?;
    std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(0o700)).map_err(internal)?;
    let mut saved = Vec::new();
    let hidden = if g.shared.is_some() {
        crate::evict::hidden_paths(g).await
    } else {
        BTreeSet::new()
    };
    for (i, change) in changes.iter().enumerate() {
        let path = match change {
            Change::Put { path, .. } | Change::Remove { path } => path,
        };
        let before = files
            .get(path)
            .map(|(_, key)| key_sha256(key).unwrap_or_default().to_owned());
        let current = wt.join(path);
        if current.symlink_metadata().is_ok_and(|m| !m.is_file()) {
            return Err(err(409, "A non-file has this name"));
        }
        if current.exists() && hash_file(&current).ok() != before
            || !current.exists() && before.is_some() && !hidden.contains(path)
        {
            return Err(err(409, "A local save is not captured yet; try again"));
        }
        let staged = match change {
            Change::Put { staged, .. } => {
                let to = stage.join(i.to_string());
                std::fs::copy(staged, &to).map_err(internal)?;
                std::fs::File::open(&to)
                    .and_then(|f| f.sync_all())
                    .map_err(internal)?;
                Some(to)
            }
            Change::Remove { .. } => None,
        };
        let after = staged
            .as_ref()
            .map(|p| hash_file(p))
            .transpose()
            .map_err(internal)?;
        saved.push(Saved {
            path: path.clone(),
            before,
            after,
            staged,
            executable: files
                .get(path)
                .is_some_and(|(file, _)| file.etag.ends_with("-1")),
            hidden: hidden.contains(path) && !current.exists(),
        });
    }
    // Do not start a transaction that capture will hold for review: replay
    // must be able to finish before this peer can serve another request.
    let deleted: BTreeSet<String> = saved
        .iter()
        .filter(|s| s.before.is_some() && s.after.is_none())
        .map(|s| s.path.clone())
        .collect();
    let held = crate::held_file::read(g);
    let waiting: BTreeSet<String> = held.held().difference(&held.released).cloned().collect();
    let split = pigeon_core::held::split(
        pigeon_core::held::MASS_DELETION,
        &deleted,
        &held.held(),
        &held.released,
        &BTreeSet::new(),
    );
    if !split.hold.is_empty() || !deleted.is_disjoint(&waiting) {
        return Err(err(
            409,
            "Deletion needs review in Pigeon; use smaller batches or review held deletions",
        ));
    }
    pigeon_sys::fs::fsync_dir(&stage).map_err(internal)?;
    let body = serde_json::to_vec(&Journal {
        format: 1,
        changes: saved,
    })
    .map_err(internal)?;
    pigeon_sys::fs::write_atomic(&journal(g), &body).map_err(internal)
}

/// Replays a batch before capture/install. The caller holds the group lock.
/// Content differing from both the old and requested version is never replaced.
pub async fn recover(d: &Daemon, g: &Group) -> std::result::Result<usize, String> {
    crate::capture::recover(g)
        .await
        .map_err(|e| e.to_string())?;
    crate::shared_overlay::recover(g).await?;
    let body = match std::fs::read(journal(g)) {
        Ok(body) => body,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.to_string()),
    };
    let txn: Journal = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    if txn.format != 1 {
        return Err("Unknown bridge journal format".into());
    }
    crate::shared_overlay::unhide(
        g,
        &txn.changes
            .iter()
            .map(|c| c.path.clone())
            .collect::<Vec<_>>(),
    )
    .await?;
    let wt = g.worktree.as_ref().ok_or("No bridge working tree")?;
    for saved in &txn.changes {
        if !valid_path(&saved.path) {
            return Err("Invalid bridge journal path".into());
        }
        let target = wt.join(&saved.path);
        let current = if target.exists() {
            Some(hash_file(&target).map_err(|e| e.to_string())?)
        } else {
            None
        };
        if saved.hidden && saved.after.is_none() {
            // Sparse directories are virtual. Keep the explicit file delete's
            // parent so capture does not mistake it for a removed top folder.
            let parent = target.parent().ok_or("No parent folder")?;
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if current == saved.after {
            continue;
        }
        if current != saved.before && !(g.shared.is_some() && saved.hidden && current.is_none()) {
            return Err("A save conflicts with an interrupted bridge operation".into());
        }
        if let Some(staged) = &saved.staged {
            let parent = target.parent().ok_or("No parent folder")?;
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            let swap = g.root().join("bridge-swap");
            std::fs::copy(staged, &swap).map_err(|e| e.to_string())?;
            std::fs::set_permissions(
                &swap,
                std::fs::Permissions::from_mode(if saved.executable { 0o755 } else { 0o644 }),
            )
            .map_err(|e| e.to_string())?;
            std::fs::File::open(&swap)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
            std::fs::rename(&swap, &target).map_err(|e| e.to_string())?;
        } else {
            std::fs::remove_file(&target).map_err(|e| e.to_string())?;
        }
        if let Some(parent) = target.parent() {
            pigeon_sys::fs::fsync_dir(parent).map_err(|e| e.to_string())?;
        }
        // This is an explicit conditional save, not an application's stale buffer.
        g.lock().installed.remove(&saved.path);
    }
    pigeon_sys::lab::crash_point("bridge-before-capture");
    let captured = crate::capture::capture(g, &d.device, &d.tracer)
        .await
        .map_err(|e| e.to_string())?;
    let view = visible(g).await.map_err(|(_, e)| e)?;
    for saved in &txn.changes {
        let key = view.get(&saved.path).map(|(_, k)| k);
        let actual = key.and_then(|k| key_sha256(k)).map(str::to_owned);
        if actual != saved.after {
            return Err("Pigeon has not captured every change yet".into());
        }
        if let Some(key) = key {
            let object = crate::custody::present(g, key)
                .await
                .ok_or("Upload content unavailable")?;
            crate::shared_pool::verify(&object, key).map_err(|e| e.to_string())?;
            sync_object(&object).map_err(|e| e.to_string())?;
        }
    }
    std::fs::remove_file(journal(g)).map_err(|e| e.to_string())?;
    pigeon_sys::fs::fsync_dir(&g.root()).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(g.root().join("bridge-stage"));
    crate::shared::compact(g).await?;
    Ok(captured.suggested)
}

#[cfg(test)]
mod tests {
    #[test]
    fn http_paths_cannot_reach_internal_files_or_escape_the_tree() {
        for path in [
            "",
            "/a",
            "../a",
            "a/../../b",
            "a//b",
            "a/.git/config",
            ".pigeon/roster.toml",
            ".gitattributes",
            "a\\b",
        ] {
            assert!(!super::valid_path(path), "{path}");
        }
        assert!(super::valid_path("Photos/holiday.jpg"));
    }
}
