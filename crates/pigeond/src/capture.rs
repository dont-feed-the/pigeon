//! Capture (07, 08 §8.1): every change of the folder becomes a version on
//! `main`, through `git status`, `git annex add` and a commit.
//!
//! Only regular files are shared; links, nested repositories and special
//! files are listed as not shared (07 §7.9). A file that a process may be
//! writing waits for the next round (07 §7.5's writer scan). A version is
//! added with `-c annex.thin=false`, so it stays an immutable copy in the
//! annex until it is protected (03 §3.4). pigeon's own paths are never
//! captured: the roster changes only through `pigeond`.

use std::path::Path;

use pigeon_git::status::Entry;
use pigeon_sys::fs::lstat;

use crate::git::{Error, is_pigeon_path};
use crate::group::Group;
use crate::trace::{Act, Tracer};

/// What one round of capture did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Captured {
    /// The new commit of `main`, if any.
    pub commit: Option<String>,
    /// Paths added or modified.
    pub added: usize,
    /// Paths deleted.
    pub removed: usize,
    /// Paths left for the next round: a process may be writing them.
    pub busy: usize,
}

fn kind(worktree: &Path, path: &str) -> Option<&'static str> {
    let p = worktree.join(path.trim_end_matches('/'));
    let st = lstat(&p).ok()??;
    if st.is_regular() {
        None
    } else if st.mode & 0o170_000 == 0o120_000 {
        Some("symlink")
    } else if st.mode & 0o170_000 == 0o040_000 {
        Some(if p.join(".git").exists() {
            "repository"
        } else {
            "folder"
        })
    } else {
        Some("special")
    }
}

/// One round of capture on a laptop.
pub async fn capture(g: &Group, tracer: &Tracer) -> Result<Captured, Error> {
    let mut done = Captured::default();
    let Some(worktree) = g.worktree.clone() else {
        return Ok(done);
    };
    let out = g
        .git
        .repo
        .run(
            &[
                "status",
                "--porcelain=v2",
                "-z",
                "-uall",
                "--ignored=traditional",
            ],
            None,
            pigeon_git::DEFAULT_TIMEOUT,
        )
        .await?;
    let entries = pigeon_git::status::parse(&out.stdout).map_err(|detail| Error::Parse {
        argv: vec!["status".to_owned()],
        detail,
    })?;
    let (mut add, mut remove, mut not_shared) = (Vec::new(), Vec::new(), Vec::new());
    for e in entries {
        let (path, deleted) = match e {
            Entry::Changed { xy, path, .. } => (path, xy.get(1..2) == Some("D")),
            Entry::Renamed { path, .. } | Entry::Untracked { path } | Entry::Ignored { path } => {
                (path, false)
            }
            Entry::Unmerged { .. } => continue,
        };
        if is_pigeon_path(&path) {
            continue;
        }
        if deleted {
            remove.push(path);
            continue;
        }
        match kind(&worktree, &path) {
            Some(k) => not_shared.push((path.trim_end_matches('/').to_owned(), k.to_owned())),
            None => {
                let Ok(Some(st)) = lstat(&worktree.join(&path)) else {
                    continue;
                };
                if pigeon_sys::writers::scan(st.id).is_clear() {
                    add.push(path);
                } else {
                    done.busy += 1;
                }
            }
        }
    }
    g.lock().not_shared = not_shared;
    for chunk in add.chunks(64) {
        let mut args = vec![
            "-c",
            "annex.thin=false",
            "annex",
            "add",
            "--no-check-gitignore",
            "--json",
            "--",
        ];
        args.extend(chunk.iter().map(String::as_str));
        g.git.run(&args).await?;
        for p in chunk {
            tracer.event(Act::Capture, &[("path", p)]);
        }
    }
    for chunk in remove.chunks(64) {
        let mut args = vec!["update-index", "--remove", "--"];
        args.extend(chunk.iter().map(String::as_str));
        g.git.run(&args).await?;
    }
    done.added = add.len();
    done.removed = remove.len();
    if done.added + done.removed == 0 {
        return Ok(done);
    }
    let tree = g.git.run(&["write-tree"]).await?.trim().to_owned();
    let head = g.git.rev("refs/heads/main").await;
    let head_tree = g.git.rev("refs/heads/main^{tree}").await;
    if head_tree.as_deref() == Some(tree.as_str()) {
        return Ok(done);
    }
    let parents: Vec<&str> = head.iter().map(String::as_str).collect();
    let commit = g
        .git
        .commit_tree(&tree, &parents, "pigeon: capture")
        .await?;
    pigeon_sys::lab::crash_point("capture-before-ref");
    g.git
        .update_ref("refs/heads/main", &commit, head.as_deref())
        .await?;
    tracing::info!(group = %g.id, commit = %commit, added = done.added, removed = done.removed, "captured");
    done.commit = Some(commit);
    Ok(done)
}
