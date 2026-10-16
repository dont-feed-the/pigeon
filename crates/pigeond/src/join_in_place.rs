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
use crate::invite::Invitation;

#[cfg(test)]
mod tests;

/// Whether `path` holds a repository of the group `inv` refounds.
pub async fn applies(d: &Daemon, inv: &Invitation, path: &Path) -> bool {
    let Some(r) = &inv.refounded else {
        return false;
    };
    let Some((role, _, _)) = crate::group::locate(path) else {
        return false;
    };
    let expected = match inv.role.as_str() {
        "laptop" => pigeon_core::filter::Role::Laptop,
        "custodian" => pigeon_core::filter::Role::Custodian,
        _ => return false,
    };
    if role != expected {
        return false;
    }
    let git = crate::git::Git::new(path, d.env.clone());
    git.root("refs/heads/main").await.as_deref() == Some(r.from.as_str())
}

/// Joins the group of `inv` with the repository of the old group at
/// `path`: answers the group's name.
pub async fn join(d: &Daemon, inv: &Invitation, path: &Path) -> Result<String, String> {
    let r = inv.refounded.clone().ok_or("invite-unknown")?;
    let old = d
        .group(&r.from)
        .filter(|g| g.worktree.as_deref().unwrap_or(&g.git_dir) == path);
    let old_busy = match &old {
        Some(old) => Some(old.busy.lock().await),
        None => None,
    };
    let g = std::sync::Arc::new(
        Group::open(&inv.group, path, &d.env)
            .await
            .ok_or("not-a-repository")?,
    );
    if crate::join_transition::pending(&g) {
        return Err("join-incomplete".into());
    }
    if crate::evict::unsettled(&g) || crate::capture_file::pending(&g) {
        return Err("repository-busy".into());
    }
    let busy = g.busy.lock().await;
    let uuid = g.lock().annex_uuid.clone().ok_or("not-a-repository")?;
    let ours = g.git.rev("refs/heads/main").await.ok_or("no main")?;
    let verified = crate::join_stage::prepare(d, inv, &uuid).await?;
    let (stage, main, archive_main, source) = crate::join_stage::persist(verified);
    let plan = crate::join_transition::Plan {
        format: 1,
        old_group: r.from.clone(),
        old_main: ours,
        group: inv.group.clone(),
        main: main.clone(),
        archive_main,
        stage: Some(stage.clone()),
        source: Some(source),
    };
    let recorded = crate::join_transition::record(&g, &plan);
    if recorded.is_err() && !crate::join_transition::pending(&g) {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(recorded
            .err()
            .unwrap_or_else(|| "could not record join".into()));
    }
    // No old-group transport can expose imported new-group objects.
    let position = old.as_ref().and_then(|old| {
        d.groups()
            .iter()
            .position(|g| std::sync::Arc::ptr_eq(g, old))
    });
    if let Some(old) = &old {
        old.stop_remotedaemon().await;
        d.groups
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|live| !std::sync::Arc::ptr_eq(live, old));
    }
    if let Err(error) = recorded {
        tracing::warn!(group = %g.id, error = %error, "join-in-place-needs-restart");
        return Err("join-incomplete".into());
    }
    if let Err(error) = crate::join_transition::finish(&g, &d.dirs.config_file()).await {
        tracing::warn!(group = %g.id, error = %error, "join-in-place-needs-restart");
        return Err("join-incomplete".into());
    }
    crate::trace_roster::moved(&d.tracer, &g, "Merge", None).await;
    crate::refound::remember(&g).await;
    d.serve_group(g.clone(), None).await;
    if let Some(position) = position {
        let mut groups = d.groups.write().unwrap_or_else(|p| p.into_inner());
        if let Some(index) = groups
            .iter()
            .position(|live| std::sync::Arc::ptr_eq(live, &g))
        {
            let g = groups.remove(index);
            let position = position.min(groups.len());
            groups.insert(position, g);
        }
    }
    drop(busy);
    drop(old_busy);
    tracing::info!(group = %g.id, old = %r.from, "joined-in-place");
    d.wake.notify_one();
    Ok(inv.name.clone())
}

/// Takes `main`'s tree into the index, keeping the folder: pigeon's own
/// files are written, an annexed file absent from the folder is hidden,
/// and what was hidden stays hidden.
pub(crate) async fn take_tree(g: &Group, main: &str) -> Result<(), String> {
    let Some(wt) = g.worktree.clone() else {
        return g
            .git
            .run(&["read-tree", main])
            .await
            .map(|_| ())
            .map_err(|e| e.to_string());
    };
    let mut hidden = crate::evict::hidden_paths(g).await;
    let fail = |e: crate::git::Error| e.to_string();
    // Adopt only the verified index. `-m` refuses dirty working files
    // whose incoming pointer changed, stranding replay at every restart.
    // Without `-u`, --reset leaves those user files for normal capture.
    g.git
        .run(&["read-tree", "--reset", main])
        .await
        .map_err(fail)?;
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
