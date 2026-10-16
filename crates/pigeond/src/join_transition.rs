//! Durable completion of a verified in-place join (09 §9.9). Failed
//! validation leaves the old repository untouched. Once a public intent
//! is durable, a restart finishes locally without another invitation.

use crate::{
    config::{self, GroupEntry},
    group::Group,
    join_stage::Source,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Plan {
    pub format: u32,
    pub old_group: String,
    pub old_main: String,
    pub group: String,
    pub main: String,
    pub archive_main: String,
    pub stage: Option<PathBuf>,
    pub source: Option<Source>,
}

fn file(g: &Group) -> PathBuf {
    g.root().join("in-place.json")
}
pub(crate) fn pending(g: &Group) -> bool {
    file(g).exists()
}

pub(crate) fn record(g: &Group, plan: &Plan) -> Result<(), String> {
    let bytes = serde_json::to_vec(plan).map_err(|e| e.to_string())?;
    crate::util::write_private(&file(g), &bytes).map_err(|e| e.to_string())?;
    pigeon_sys::fs::fsync_dir(&g.root()).map_err(|e| e.to_string())
}

async fn keep(g: &Group, name: &str, tip: &str) -> Result<(), String> {
    match g.git.rev(name).await {
        Some(existing) if existing == tip => Ok(()),
        Some(_) => Err("in-place archive reference changed".into()),
        None => g
            .git
            .update_ref(name, tip, None)
            .await
            .map_err(|e| e.to_string()),
    }
}

async fn preserve(g: &Group, old: &str, tip: &str) -> Result<(), String> {
    keep(
        g,
        &format!("{}{old}/local/{tip}", crate::refound::ARCHIVE),
        tip,
    )
    .await
}

async fn archive(g: &Group, old: &str, tip: &str) -> Result<(), String> {
    let name = crate::refound::archive_ref(old);
    let previous = g.git.rev(&name).await;
    if previous.as_deref() == Some(tip) {
        return Ok(());
    }
    if let Some(previous) = &previous {
        preserve(g, old, previous).await?;
    }
    g.git
        .update_ref(&name, tip, previous.as_deref())
        .await
        .map_err(|e| e.to_string())
}

async fn import(g: &Group, plan: &Plan) -> Result<(), String> {
    let Some(stage) = &plan.stage else {
        return Ok(());
    };
    let prefix = format!("refs/pigeon/incoming/{}/", plan.group);
    let spec = format!("+refs/*:{prefix}*");
    g.git
        .run(&[
            "fetch",
            "-q",
            "--no-tags",
            "--no-write-fetch-head",
            &stage.to_string_lossy(),
            &spec,
        ])
        .await
        .map_err(|e| e.to_string())?;
    // Rebuild canonical archive refs from the signed chain, including
    // proposal proofs imported from the private stage's refs. Never copy
    // arbitrary incoming archive names into the serving namespace.
    crate::archive_chain::retain(g, &plan.group).await?;
    if let Some(source) = &plan.source {
        crate::settings::ensure_remote(&g.git, &source.device, &source.uuid)
            .await
            .map_err(|e| e.to_string())?;
        let addresses = source
            .addresses
            .iter()
            .filter_map(|a| a.parse().ok())
            .collect();
        g.set_addresses(&source.device, addresses);
        g.learn_peers(&source.peers);
        if let Some(ga) = g.git.rev(&format!("{prefix}heads/git-annex")).await {
            g.git
                .run(&[
                    "update-ref",
                    &format!("refs/remotes/{}/git-annex", source.device),
                    &ga,
                ])
                .await
                .map_err(|e| e.to_string())?;
            g.git
                .run(&["annex", "merge"])
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

async fn proposals(g: &Group, old: &str) -> Result<(), String> {
    let refs = g
        .git
        .run(&[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            crate::proposal::PREFIX,
        ])
        .await
        .map_err(|e| e.to_string())?;
    for line in refs.lines() {
        let Some((name, tip)) = line.split_once(' ') else {
            return Err("bad proposal ref".into());
        };
        if g.git.root(tip).await.as_deref() != Some(old) {
            continue;
        }
        let suffix = name
            .strip_prefix(crate::proposal::PREFIX)
            .ok_or("bad proposal prefix")?;
        keep(
            g,
            &format!("{}{old}/proposals/{suffix}/{tip}", crate::refound::ARCHIVE),
            tip,
        )
        .await?;
        g.git
            .run(&["update-ref", "-d", name, tip])
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn clean(g: &Group, plan: &Plan) {
    if let Err(e) = std::fs::remove_file(file(g)) {
        tracing::warn!(group = %plan.group, error = %e, "join-intent-cleanup-waits");
        return;
    }
    let _ = pigeon_sys::fs::fsync_dir(&g.root());
    if let Some(stage) = &plan.stage
        && stage
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("join-stage-"))
    {
        let _ = std::fs::remove_dir_all(stage);
    }
}

pub(crate) async fn finish(g: &Group, configuration: &Path) -> Result<bool, String> {
    let bytes = match std::fs::read(file(g)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.to_string()),
    };
    let plan: Plan = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if plan.format != 1 || (g.id != plan.group && g.id != plan.old_group) {
        return Err("unknown in-place join intent".into());
    }
    let current = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or("in-place join has no main")?;
    let configured = config::load(configuration)
        .map_err(|e| e.to_string())?
        .groups
        .iter()
        .any(|e| e.id == plan.group);
    // Configuration is written last. A leftover intent must not reset a
    // later index, capture or install after an already-completed join.
    if configured
        && g.git.root(&current).await.as_deref() == Some(plan.group.as_str())
        && g.git.contains(&current, &plan.main).await
    {
        clean(g, &plan);
        return Ok(true);
    }
    import(g, &plan).await?;
    if g.git.root(&plan.main).await.as_deref() != Some(plan.group.as_str())
        || g.git.root(&plan.archive_main).await.as_deref() != Some(plan.old_group.as_str())
        || g.git.root(&plan.old_main).await.as_deref() != Some(plan.old_group.as_str())
    {
        return Err("in-place join history is missing".into());
    }
    preserve(g, &plan.old_group, &plan.old_main).await?;
    archive(g, &plan.old_group, &plan.archive_main).await?;
    proposals(g, &plan.old_group).await?;
    pigeon_sys::lab::crash_point("join-in-place.before-main");
    let root = g.git.root(&current).await;
    if root.as_deref() == Some(plan.old_group.as_str()) {
        preserve(g, &plan.old_group, &current).await?;
        g.git
            .update_ref("refs/heads/main", &plan.main, Some(&current))
            .await
            .map_err(|e| e.to_string())?;
    } else if root.as_deref() != Some(plan.group.as_str())
        || !g.git.contains(&current, &plan.main).await
    {
        return Err("in-place main belongs to another history".into());
    }
    pigeon_sys::lab::crash_point("join-in-place.after-main");
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or("in-place join has no main")?;
    crate::join_in_place::take_tree(g, &main).await?;
    g.reload_roster().await;
    pigeon_sys::lab::crash_point("join-in-place.after-index");
    let path = g.worktree.as_deref().unwrap_or(&g.git_dir);
    config::replace_group(
        configuration,
        &plan.old_group,
        &GroupEntry {
            id: plan.group.clone(),
            path: path.to_path_buf(),
        },
    )
    .map_err(|e| e.to_string())?;
    pigeon_sys::lab::crash_point("join-in-place.after-config");
    clean(g, &plan);
    Ok(true)
}

pub(crate) async fn recover(
    entry: &GroupEntry,
    configuration: &Path,
    env: &pigeon_git::GitEnv,
) -> Result<(), String> {
    let Some((_, git_dir, _)) = crate::group::locate(&entry.path) else {
        return Ok(());
    };
    if !git_dir.join("pigeon/in-place.json").exists() {
        return Ok(());
    }
    let g = Group::open(&entry.id, &entry.path, env)
        .await
        .ok_or("in-place repository missing")?;
    finish(&g, configuration).await?;
    Ok(())
}
