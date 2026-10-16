//! DAV's persistent IDs, times and empty directories. Content and divergences
//! come exclusively from a daemon snapshot. Directory ETags cover descendants,
//! so a deep Pigeon edit is discoverable by a Nextcloud client polling the root.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_proto::bridge::Reply;
use sha2::{Digest, Sha256};

use crate::{
    App, Error, Result,
    catalogue::{self, Catalogue},
    store::{self, Client, Metadata},
    unavailable,
};

#[derive(Clone)]
pub struct Node {
    pub path: String,
    pub directory: bool,
    pub etag: String,
    pub id: u64,
    pub modified: u64,
    pub size: u64,
    pub diverging: bool,
}

pub struct Index {
    pub revision: String,
    pub nodes: BTreeMap<String, Node>,
}

pub fn path_valid(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|p| !matches!(p, "" | "." | ".." | ".git" | ".pigeon"))
        && path != ".gitattributes"
        && pigeon_core::names::portable(path).is_ok()
}

pub fn under(path: &str, directory: &str) -> bool {
    directory.is_empty() || path == directory || path.starts_with(&format!("{directory}/"))
}

pub fn reply(reply: Reply) -> Result<Reply> {
    if let Reply::Failure { status, message } = reply {
        Err(Error(
            axum::http::StatusCode::from_u16(status).map_err(unavailable)?,
            message,
        ))
    } else {
        Ok(reply)
    }
}

pub async fn load(app: &App, client: &Client) -> Result<Index> {
    load_at(app, client, None).await
}

pub async fn load_at(app: &App, client: &Client, revision: Option<&str>) -> Result<Index> {
    let socket = app.peers.checked(client).await.map_err(unavailable)?;
    let snapshot = catalogue::read(&socket, revision).await?;
    build(app, client, snapshot).await
}

pub async fn build(app: &App, client: &Client, snapshot: Catalogue) -> Result<Index> {
    let mut store = app.store.lock().await;
    let old = store
        .clients
        .get(&client.id)
        .ok_or_else(crate::forbidden)?
        .metadata
        .clone();
    let mut dirs: BTreeSet<String> = old
        .iter()
        .filter(|(_, m)| m.directory)
        .map(|(p, _)| p.clone())
        .collect();
    dirs.insert(String::new());
    for file in snapshot.files.values() {
        let mut parent = std::path::Path::new(&file.path).parent();
        while let Some(p) = parent {
            dirs.insert(p.to_string_lossy().into_owned());
            parent = p.parent();
        }
    }
    let file_paths: BTreeSet<_> = snapshot.files.keys().cloned().collect();
    dirs.retain(|d| !file_paths.contains(d));
    let mut hashes: BTreeMap<_, _> = dirs
        .iter()
        .map(|d| (d.clone(), (Sha256::new(), 0u64)))
        .collect();
    for file in snapshot.files.values() {
        let mut parent = std::path::Path::new(&file.path).parent();
        while let Some(p) = parent {
            if let Some((hash, size)) = hashes.get_mut(p.to_string_lossy().as_ref()) {
                hash.update(file.path.as_bytes());
                hash.update([0]);
                hash.update(file.etag.as_bytes());
                hash.update([0]);
                *size = size.saturating_add(file.size);
            }
            parent = p.parent();
        }
    }
    for sub in &dirs {
        let mut parent = std::path::Path::new(sub).parent();
        while let Some(p) = parent {
            if let Some((hash, _)) = hashes.get_mut(p.to_string_lossy().as_ref()) {
                hash.update(sub.as_bytes());
                hash.update([0]);
            }
            parent = p.parent();
        }
    }
    let mut descriptions: BTreeMap<String, (bool, String, u64, bool)> = snapshot
        .files
        .into_values()
        .map(|f| (f.path, (false, f.etag, f.size, f.diverging)))
        .collect();
    for (dir, (hash, size)) in hashes {
        let etag = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        descriptions.insert(dir, (true, etag, size, false));
    }
    let mut renames = BTreeMap::new();
    for (path, meta) in &old {
        if !meta.directory && !file_paths.contains(path) {
            renames
                .entry(meta.etag.clone())
                .and_modify(|candidate| *candidate = None)
                .or_insert_with(|| Some(meta.clone()));
        }
    }
    let mut metadata = BTreeMap::new();
    let mut nodes = BTreeMap::new();
    let mut used_ids = BTreeSet::new();
    for (path, (directory, etag, size, diverging)) in descriptions {
        // A unique incoming rename can retain its numeric ID. Ambiguous copies get a new one.
        let renamed = if directory || old.contains_key(&path) {
            None
        } else {
            renames.get(&etag).cloned().flatten()
        };
        let mut meta = match old
            .get(&path)
            .cloned()
            .or(renamed)
            .filter(|m| m.directory == directory && !used_ids.contains(&m.id))
        {
            Some(m) => m,
            None => {
                let id = store.next_file_id;
                store.next_file_id = store
                    .next_file_id
                    .checked_add(1)
                    .ok_or_else(|| unavailable("File ID space exhausted"))?;
                Metadata {
                    id,
                    etag: etag.clone(),
                    modified: store::now(),
                    directory,
                }
            }
        };
        if meta.etag != etag {
            meta.modified = store::now();
            meta.etag = etag.clone();
        }
        used_ids.insert(meta.id);
        nodes.insert(
            path.clone(),
            Node {
                path: path.clone(),
                directory,
                etag,
                id: meta.id,
                modified: meta.modified,
                size,
                diverging,
            },
        );
        metadata.insert(path, meta);
    }
    if metadata != old {
        store
            .clients
            .get_mut(&client.id)
            .ok_or_else(crate::forbidden)?
            .metadata = metadata;
        store.save(&app.config.state_dir).map_err(unavailable)?;
    }
    Ok(Index {
        revision: snapshot.revision,
        nodes,
    })
}

pub async fn mkdir(app: &App, client: &Client, path: &str) -> Result<()> {
    let mut store = app.store.lock().await;
    let id = store.next_file_id;
    store.next_file_id = store
        .next_file_id
        .checked_add(1)
        .ok_or_else(|| unavailable("File ID space exhausted"))?;
    store
        .clients
        .get_mut(&client.id)
        .ok_or_else(crate::forbidden)?
        .metadata
        .insert(
            path.to_owned(),
            Metadata {
                id,
                etag: store::hash(&[]),
                modified: store::now(),
                directory: true,
            },
        );
    store.save(&app.config.state_dir).map_err(unavailable)
}

pub async fn remove_metadata(app: &App, client: &Client, path: &str) -> Result<()> {
    let mut store = app.store.lock().await;
    store
        .clients
        .get_mut(&client.id)
        .ok_or_else(crate::forbidden)?
        .metadata
        .retain(|p, _| !under(p, path));
    store.save(&app.config.state_dir).map_err(unavailable)
}

pub async fn mtime(app: &App, client: &Client, path: &str, modified: Option<u64>) -> Result<()> {
    if let Some(modified) = modified {
        let mut store = app.store.lock().await;
        if let Some(meta) = store
            .clients
            .get_mut(&client.id)
            .and_then(|c| c.metadata.get_mut(path))
        {
            meta.modified = modified;
        }
        store.save(&app.config.state_dir).map_err(unavailable)?;
    }
    Ok(())
}
