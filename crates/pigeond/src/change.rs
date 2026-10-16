//! Commits that `pigeond` writes itself: the genesis and roster changes
//! (04 §4.2). Genesis commits use the initial index. Later roster changes
//! build only on the accepted tree, verify authority before a CAS, then
//! stage only their metadata; private index and working files stay local.

use std::path::Path;

use crate::git::{Error, Git};
use crate::group::Group;
use crate::roster_file::{self, RosterDoc};
use crate::trace::Tracer;

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
    // The checked driver shares the bounded index-lock retry with capture's
    // update-index calls. commit-tree and the ref CAS are still never retried.
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
    let parent = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or("no accepted head")?;
    let mut tree = g.git.tree(&parent).await.map_err(|e| e.to_string())?;
    let blob = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    tree.insert(
        ".pigeon/roster.toml".into(),
        pigeon_core::authority::Entry {
            mode: "100644".into(),
            kind: "blob".into(),
            oid: blob.trim().into(),
        },
    );
    let tree = crate::merge::write_tree(&g.git, &tree)
        .await
        .map_err(|e| e.to_string())?;
    let commit = g
        .git
        .commit_tree(&tree, &[&parent], message)
        .await
        .map_err(|e| e.to_string())?;
    let judged = crate::verify::judge(g, None, &commit)
        .await
        .map_err(|e| e.to_string())?;
    if judged.accepted.as_deref() != Some(&commit) {
        return Err("roster change did not pass authority verification".into());
    }
    g.git
        .update_ref("refs/heads/main", &commit, Some(&parent))
        .await
        .map_err(|e| e.to_string())?;
    // Membership is already durable. A transient index.lock must not
    // refuse that welcome or leave the in-memory roster at its old value.
    g.lock().roster_index_pending = true;
    g.reload_roster().await;
    crate::trace_roster::moved(tracer, g, "Commit", Some(&parent)).await;
    if let Err(error) = repair_roster_index(g).await {
        tracing::info!(group = %g.id, %error, "roster-index-waits");
    }
    Ok(commit)
}

/// Retry only pigeon's roster entry after an accepted change. The caller
/// holds `busy`; unrelated staged files and working saves stay untouched.
/// Opening a laptop also requests this repair, covering a crash after CAS.
pub async fn repair_roster_index(g: &Group) -> Result<(), Error> {
    if !g.lock().roster_index_pending {
        return Ok(());
    }
    let Some(bytes) = g.git.blob("refs/heads/main:.pigeon/roster.toml").await else {
        return Err(Error::Parse {
            argv: vec!["repair roster index".into()],
            detail: "no accepted roster".into(),
        });
    };
    stage_file(&g.git, g.worktree.as_deref(), ".pigeon/roster.toml", &bytes).await?;
    g.lock().roster_index_pending = false;
    Ok(())
}
