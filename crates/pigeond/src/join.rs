//! A laptop's join (09 §9.6, steps 1 and 5): the new laptop makes its
//! repository in an empty folder, sends `Join` to an admitter, and once
//! welcomed fetches `main` from it, checks its genesis and that the welcome
//! commit adds it, and takes `main`'s tree with every annexed file hidden,
//! so that keep brings the files back (08 §8.5) and nothing reads as
//! deleted. The first build keeps every folder: choosing them at the join
//! is not in it (13 §13.12).

use std::path::PathBuf;

use crate::daemon::Daemon;
use crate::group::Group;
use crate::invite::{Joining, admitter_addresses, dial, fetch_welcome};
use crate::settings::{ensure_remote, init_annex};

/// Joins the group of `j` as a laptop in `folder` (by default the group's
/// name in the home directory): answers the group's name.
pub async fn laptop(d: &Daemon, j: &Joining, folder: Option<String>) -> Result<String, String> {
    let inv = &j.inv;
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    let path = folder.map_or_else(|| home.join(&inv.name), PathBuf::from);
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let path = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
    if crate::join_in_place::applies(d, j, &path).await {
        return crate::join_in_place::join(d, j, &path).await;
    }
    if std::fs::read_dir(&path)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("folder-not-empty".into());
    }
    let git = crate::git::Git::new(&path, d.env.clone());
    let fail = |e: crate::git::Error| e.to_string();
    git.run(&["init", "-q", "-b", "main"]).await.map_err(fail)?;
    let name = &j.name;
    git.run(&["config", "user.name", name])
        .await
        .map_err(fail)?;
    let email = format!("{}@pigeon", d.device);
    git.run(&["config", "user.email", &email])
        .await
        .map_err(fail)?;
    let pending = Group::open("pending", &path, &d.env)
        .await
        .ok_or("not-a-repository")?;
    crate::settings::check(&pending).await;
    let (uuid, token) = init_annex(&git, pigeon_core::filter::Role::Laptop, name, inv.k)
        .await
        .map_err(fail)?;
    let k = inv.k.to_string();
    git.run(&["annex", "numcopies", &k]).await.map_err(fail)?;
    git.run(&["annex", "mincopies", &k]).await.map_err(fail)?;
    drop(pending);
    let w = dial(d, j, &uuid).await?;
    ensure_remote(&git, &w.admitter.device, &w.annex_uuid)
        .await
        .map_err(fail)?;
    let g = Group::open(&inv.group, &path, &d.env)
        .await
        .ok_or("not-a-repository")?;
    g.set_token(&token);
    g.set_addresses(&w.admitter.device, admitter_addresses(&w.admitter));
    g.learn_peers(&w.peers);
    let g = d.add_group(g, &path).await?;
    // No round may capture before every annexed file is hidden: a file of
    // the index missing from the folder would read as a deletion.
    let busy = g.busy.lock().await;
    let theirs = fetch_welcome(&g, &w.admitter.device, &inv.group, &w.commit, &d.device).await?;
    g.git
        .update_ref("refs/heads/main", &theirs, None)
        .await
        .map_err(|e| e.to_string())?;
    g.reload_roster().await;
    crate::trace_roster::moved(&d.tracer, &g, "Merge", None).await;
    crate::recovery::hide_all(&g, &theirs).await?;
    drop(busy);
    tracing::info!(group = %g.id, commit = %w.commit, "joined-as-laptop");
    d.wake.notify_one();
    Ok(inv.name.clone())
}
