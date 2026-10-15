//! A folder of a refounded group joins the new group in place (09 §9.9,
//! ADR 0047): the repository keeps its annex UUID, token and objects, the
//! old group is forgotten, the old `main` stays as the archive, and once
//! welcomed `main` becomes the new group's. The index takes the new tree
//! without touching the folder: an annexed file the folder lacks is hidden,
//! as on a join, and what differs in it is a save the loops capture.

use std::collections::BTreeSet;
use std::path::Path;

use crate::daemon::Daemon;
use crate::group::Group;
use crate::invite::{Invitation, dial, fetch_welcome, inviter_addresses, request};
use crate::settings::ensure_remote;

/// Whether `path` holds a repository of the group `inv` refounds.
pub async fn applies(d: &Daemon, inv: &Invitation, path: &Path) -> bool {
    let Some(r) = &inv.refounded else {
        return false;
    };
    if !path.join(".git").is_dir() {
        return false;
    }
    let git = crate::git::Git::new(path, d.env.clone());
    git.root("refs/heads/main").await.as_deref() == Some(r.from.as_str())
}

/// Joins the group of `inv` with the repository of the old group at
/// `path`: answers the group's name.
pub async fn laptop(d: &Daemon, inv: &Invitation, path: &Path) -> Result<String, String> {
    let r = inv.refounded.clone().ok_or("invite-unknown")?;
    if let Some(old) = d.group(&r.from) {
        crate::refound::forget(d, &old).await;
    }
    let g = Group::open(&inv.group, path, &d.env)
        .await
        .ok_or("not-a-repository")?;
    let uuid = g.lock().annex_uuid.clone().ok_or("not-a-repository")?;
    let name = crate::invite::device_name(d, inv);
    let request = request(d, inv, &name, &uuid)?;
    let (commit, inviter_uuid, peers) = dial(d, inv, &request).await?;
    let fail = |e: crate::git::Error| e.to_string();
    ensure_remote(&g.git, &inv.inviter, &inviter_uuid)
        .await
        .map_err(fail)?;
    g.set_addresses(&inv.inviter, inviter_addresses(inv));
    g.learn_peers(&peers);
    let g = d.add_group(g, path).await;
    let busy = g.busy.lock().await;
    let theirs = fetch_welcome(&g, &inv.inviter, &inv.group, &commit, &d.device).await?;
    let ours = g.git.rev("refs/heads/main").await.ok_or("no main")?;
    let archive = crate::refound::archive_ref(&r.from);
    if g.git.rev(&archive).await.is_none() {
        g.git
            .update_ref(&archive, &ours, None)
            .await
            .map_err(fail)?;
    }
    g.git
        .update_ref("refs/heads/main", &theirs, Some(&ours))
        .await
        .map_err(fail)?;
    g.reload_roster().await;
    // Refounding starts a new accepted history, as a fresh join does.
    crate::trace_roster::moved(&d.tracer, &g, "Merge", None).await;
    take_tree(&g, &theirs).await?;
    crate::refound::remember(&g).await;
    drop(busy);
    tracing::info!(group = %g.id, old = %r.from, "joined-in-place");
    d.wake.notify_one();
    Ok(inv.name.clone())
}

/// Takes `main`'s tree into the index, keeping the folder: pigeon's own
/// files are written, an annexed file absent from the folder is hidden,
/// and what was hidden stays hidden.
async fn take_tree(g: &Group, main: &str) -> Result<(), String> {
    let wt = g.worktree.clone().ok_or("not-in-group")?;
    let mut hidden = crate::evict::hidden_paths(g).await;
    let fail = |e: crate::git::Error| e.to_string();
    // `-m` keeps the stat data of the entries that do not change.
    g.git.run(&["read-tree", "-m", main]).await.map_err(fail)?;
    let files = g.git.tree_files(main).await.map_err(fail)?;
    let paths: BTreeSet<String> = files.iter().map(|f| f.path.clone()).collect();
    hidden.retain(|p| paths.contains(p));
    for f in files {
        let blob = g
            .git
            .blob(&format!("{main}:{}", f.path))
            .await
            .unwrap_or_default();
        let file = wt.join(&f.path);
        if crate::git::pointer_key(&blob).is_some() {
            if std::fs::symlink_metadata(&file).is_err() {
                hidden.insert(f.path);
            }
            continue;
        }
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        pigeon_sys::fs::write_atomic(&file, &blob).map_err(|e| e.to_string())?;
    }
    crate::evict::write_sparse(g, &hidden);
    for chunk in hidden.iter().collect::<Vec<_>>().chunks(64) {
        let mut args = vec!["update-index", "--skip-worktree", "--"];
        args.extend(chunk.iter().map(|p| p.as_str()));
        g.git.run(&args).await.map_err(fail)?;
    }
    let _ = g.git.run(&["update-index", "-q", "--refresh"]).await;
    Ok(())
}
