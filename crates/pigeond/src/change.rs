//! Commits that `pigeond` writes itself on a laptop: the genesis and roster
//! changes (04 §4.2). Each file is written to the folder and to the index,
//! then the index is committed on `main` with the expected old value.

use std::path::Path;

use crate::git::{Error, Git};
use crate::group::Group;
use crate::roster_file::{self, RosterDoc};
use crate::trace::{Act, Tracer};

/// Writes `bytes` at `path` in the index, and in `worktree` if there is one.
pub async fn stage_file(
    git: &Git,
    worktree: Option<&Path>,
    path: &str,
    bytes: &[u8],
) -> Result<(), Error> {
    if let Some(w) = worktree {
        let file = w.join(path);
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = pigeon_sys::fs::write_atomic(&file, bytes);
    }
    let blob = git
        .run_in(&["hash-object", "-w", "--stdin"], bytes)
        .await?
        .trim()
        .to_owned();
    let info = format!("100644,{blob},{path}");
    git.run(&["update-index", "--add", "--cacheinfo", &info])
        .await
        .map(|_| ())
}

/// Commits the index on `main`, whose head is `parent`.
pub async fn commit_index(git: &Git, parent: Option<&str>, message: &str) -> Result<String, Error> {
    let tree = git.run(&["write-tree"]).await?.trim().to_owned();
    let parents: Vec<&str> = parent.into_iter().collect();
    let commit = git.commit_tree(&tree, &parents, message).await?;
    git.update_ref("refs/heads/main", &commit, parent).await?;
    let _ = git.run(&["update-index", "-q", "--refresh"]).await;
    Ok(commit)
}

/// Commits `doc` as the roster of `g`'s `main`.
pub async fn commit_roster(
    g: &Group,
    doc: &RosterDoc,
    message: &str,
    tracer: &Tracer,
) -> Result<String, String> {
    doc.roster.validate().map_err(|e| format!("{e:?}"))?;
    let text = roster_file::render(doc)?;
    let parent = g.git.rev("refs/heads/main").await;
    stage_file(
        &g.git,
        g.worktree.as_deref(),
        ".pigeon/roster.toml",
        text.as_bytes(),
    )
    .await
    .map_err(|e| e.to_string())?;
    let commit = commit_index(&g.git, parent.as_deref(), message)
        .await
        .map_err(|e| e.to_string())?;
    tracer.event(Act::Commit, &[("commit", &commit)]);
    g.reload_roster().await;
    Ok(commit)
}
