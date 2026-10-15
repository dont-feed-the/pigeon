//! Managed bridge storage: laptop identities retain their own Git history and
//! annex UUID, while file bodies live in an immutable pool and accepted files
//! stay hidden. Existing storage nodes export verified snapshots into that pool.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use pigeon_core::filter::Role;
use pigeon_core::roster::DeviceId;
use pigeon_proto::bridge::{Operation, Reply};

use crate::{daemon::Daemon, group::Group, shared_pool as pool};

pub(crate) fn configure(
    g: &mut Group,
    storage: Option<&crate::config::BridgeStorage>,
) -> Result<(), String> {
    if let Some(storage) = storage {
        if g.role != Role::Laptop
            || !storage.store.is_absolute()
            || !storage.source_socket.is_absolute()
        {
            return Err("Bridge storage requires a laptop identity and absolute paths".into());
        }
        pool::private_dir(&storage.store).map_err(|e| e.to_string())?;
        if storage.store.metadata().map_err(|e| e.to_string())?.dev()
            != g.git_dir.metadata().map_err(|e| e.to_string())?.dev()
        {
            return Err(
                "The shared content pool and managed peers must be on the same filesystem".into(),
            );
        }
        if storage.store.starts_with(&g.git_dir)
            || g.worktree
                .as_ref()
                .is_some_and(|p| storage.store.starts_with(p))
        {
            return Err("The shared pool must be outside the client working tree".into());
        }
        g.shared = Some(storage.clone());
    }
    Ok(())
}

/// A storage provider may be a laptop or custodian, but not another client
/// overlay: this avoids recursive provider calls and repository-lock cycles.
pub(crate) async fn export(
    d: &Daemon,
    group: &str,
    key: &str,
    store: &Path,
) -> Result<Reply, String> {
    let g = d
        .group(group)
        .ok_or("The storage node does not serve this group")?;
    if g.shared.is_some() {
        return Err("A managed client cannot be a content provider".into());
    }
    let _busy = g.busy.lock().await;
    if !g
        .roster()
        .is_some_and(|r| r.roster.devices.contains_key(&DeviceId(d.device.clone())))
    {
        return Err("The storage node is no longer a group member".into());
    }
    if store.starts_with(&g.git_dir) || g.worktree.as_ref().is_some_and(|p| store.starts_with(p)) {
        return Err("The content pool must be outside the provider repository".into());
    }
    if g.role == Role::Laptop && (crate::evict::unsettled(&g) || crate::capture_file::pending(&g)) {
        return Err("The storage node must finish recovering its installation".into());
    }
    pool::register(&g.git_dir, store).map_err(|e| e.to_string())?;
    crate::settings::check(&g).await;
    let target = pool::path(store, group, key).map_err(|e| e.to_string())?;
    if target.exists() {
        pool::verify(&target, key).map_err(|e| e.to_string())?;
        return Ok(Reply::Content {
            path: target.display().to_string(),
        });
    }
    if g.role == Role::Laptop {
        crate::capture::capture(&g, &d.device, &d.tracer)
            .await
            .map_err(|e| e.to_string())?;
    }
    let source = crate::custody::present(&g, key)
        .await
        .ok_or("The storage node does not retain this version")?;
    let (store, group, key, immutable) = (store.to_owned(), group.to_owned(), key.to_owned(), true);
    let target =
        tokio::task::spawn_blocking(move || pool::retain(&store, &group, &key, &source, immutable))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    Ok(Reply::Content {
        path: target.display().to_string(),
    })
}

pub(crate) async fn register(d: &Daemon, group: &str, store: &Path) -> Result<Reply, String> {
    let g = d
        .group(group)
        .ok_or("The storage node does not serve this group")?;
    if g.shared.is_some() {
        return Err("A managed client cannot be a content provider".into());
    }
    let _busy = g.busy.lock().await;
    if !g
        .roster()
        .is_some_and(|r| r.roster.devices.contains_key(&DeviceId(d.device.clone())))
    {
        return Err("The storage node is no longer a group member".into());
    }
    if store.starts_with(&g.git_dir) || g.worktree.as_ref().is_some_and(|p| store.starts_with(p)) {
        return Err("The content pool must be outside the provider repository".into());
    }
    if g.role == Role::Laptop && (crate::evict::unsettled(&g) || crate::capture_file::pending(&g)) {
        return Err("The storage node must finish recovering its installation".into());
    }
    pool::register(&g.git_dir, store).map_err(|e| e.to_string())?;
    crate::settings::check(&g).await;
    d.wake.notify_one();
    Ok(Reply::Learned)
}

/// Makes a cached/source version available to this client's annex without
/// copying its bytes. The client's untrusted laptop UUID remains independent.
pub(crate) async fn hydrate(g: &Group, key: &str) -> Result<bool, String> {
    let Some(storage) = &g.shared else {
        return Ok(false);
    };
    let target = pool::path(&storage.store, &g.id, key).map_err(|e| e.to_string())?;
    if !target.exists() {
        let result = crate::shared_rpc::call(
            &storage.source_socket,
            Operation::ExportContent {
                group: g.id.clone(),
                key: key.to_owned(),
                store: storage.store.display().to_string(),
            },
        )
        .await;
        match result {
            Ok(Reply::Content { path }) if Path::new(&path) == target => {}
            Ok(Reply::Content { .. }) => {
                return Err("The content node returned a path outside the pool".into());
            }
            // Normal network retrieval remains available when the local source
            // is offline, lacks this version, or does not serve this group.
            _ => return Ok(false),
        }
    }
    attach(g, key, &target).await?;
    Ok(true)
}

async fn attach(g: &Group, key: &str, target: &Path) -> Result<(), String> {
    pool::verify(target, key).map_err(|e| e.to_string())?;
    let folder = g.root().join("shared-inject");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let stage = folder.join(key);
    if stage.exists() {
        std::fs::remove_file(&stage).map_err(|e| e.to_string())?;
    }
    std::fs::hard_link(target, &stage).map_err(|e| e.to_string())?;
    let result = g
        .git
        .run(&[
            "annex",
            "reinject",
            "--guesskeys",
            &stage.display().to_string(),
        ])
        .await;
    let _ = std::fs::remove_file(&stage);
    result.map_err(|e| e.to_string())?;
    let object = crate::custody::present(g, key)
        .await
        .ok_or("The shared object was not attached")?;
    pool::relink(target, &object).map_err(|e| e.to_string())?;
    Ok(())
}

/// Normal custodian admission still chooses the keys to retain. An admitted
/// key already in the pool can be pinned without receiving another byte copy.
pub(crate) async fn provider_hydrate(g: &Group, key: &str) -> Result<bool, String> {
    if g.role != Role::Custodian {
        return Ok(false);
    }
    let Some(store) = pool::registration(&g.git_dir).map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    let object = pool::path(&store, &g.id, key).map_err(|e| e.to_string())?;
    if !object.exists() {
        return Ok(false);
    }
    attach(g, key, &object).await?;
    Ok(true)
}

/// Reconcile versions received before the pool was registered as well. A
/// registered laptop no longer re-thins objects into writable worktree links.
pub(crate) async fn provider_compact(g: &Group) -> Result<(), String> {
    if g.role == Role::Laptop && (crate::evict::unsettled(g) || crate::capture_file::pending(g)) {
        return Ok(());
    }
    let Some(store) = pool::registration(&g.git_dir).map_err(|e| e.to_string())? else {
        return Ok(());
    };
    compact_objects(
        g.git_dir.join("annex/objects"),
        store,
        g.id.clone(),
        g.role == Role::Laptop,
    )
    .await
}

/// Canonicalize an object obtained through normal annex transport as well.
pub(crate) async fn adopt(g: &Group, key: &str) -> Result<(), String> {
    let Some(storage) = &g.shared else {
        return Ok(());
    };
    let source = crate::custody::present(g, key)
        .await
        .ok_or("Content unavailable")?;
    let (store, group, key) = (storage.store.clone(), g.id.clone(), key.to_owned());
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let target = pool::path(&store, &group, &key)?;
        if !pool::same_inode(&source, &target) {
            let target = pool::retain(&store, &group, &key, &source, true)?;
            pool::relink(&target, &source)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

fn local_objects(
    directory: &Path,
    out: &mut Vec<(String, PathBuf)>,
    depth: usize,
) -> std::io::Result<()> {
    if depth > 5 || !directory.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            local_objects(&entry.path(), out, depth + 1)?;
        } else if kind.is_file() {
            let key = entry.file_name().to_string_lossy().into_owned();
            if crate::net::sync::plausible_key(&key) && crate::net::sync::key_sha256(&key).is_some()
            {
                out.push((key, entry.path()));
            }
        }
    }
    Ok(())
}

/// Deduplicates retained annex versions, including history and proposals, then
/// reduces the working tree to this peer's pending changes. Never drops a key.
pub(crate) async fn compact(g: &Group) -> Result<(), String> {
    let Some(storage) = &g.shared else {
        return Ok(());
    };
    if crate::capture_file::pending(g) || crate::evict::unsettled(g) {
        return Err("Shared storage must wait for capture and install recovery".into());
    }
    let (directory, store, group) = (
        g.git_dir.join("annex/objects"),
        storage.store.clone(),
        g.id.clone(),
    );
    compact_objects(directory, store, group, false).await?;
    crate::shared_overlay::compact(g).await
}

async fn compact_objects(
    directory: PathBuf,
    store: PathBuf,
    group: String,
    laptop_provider: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let mut objects = Vec::new();
        local_objects(&directory, &mut objects, 0)?;
        for (key, object) in objects {
            let target = pool::path(&store, &group, &key)?;
            if pool::same_inode(&object, &target) {
                continue;
            }
            // A pre-registration thinned laptop object may already have been
            // edited through its live inode. Never publish those bytes under
            // the old key; its protected custody copies remain authoritative.
            if laptop_provider && !target.exists() && pool::verify(&object, &key).is_err() {
                continue;
            }
            let target = pool::retain(&store, &group, &key, &object, true)?;
            pool::relink(&target, &object)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// A relink may change a pending file's inode/stat across a crash. Its captured
/// bytes remain authoritative; an actual newer save is still refused.
pub(crate) fn unchanged(
    g: &Group,
    rel: &str,
    divergence: &crate::conflicts_file::Divergence,
) -> bool {
    let Some(wt) = &g.worktree else {
        return false;
    };
    match &divergence.mine {
        Some(key) => pool::verify(&wt.join(rel), key).is_ok(),
        None => !wt.join(rel).exists(),
    }
}
