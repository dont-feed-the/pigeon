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

use pigeon_core::authority::Tree;
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

/// `git status` of the folder (07 §7.3 step 2).
async fn status(g: &Group) -> Result<Vec<Entry>, Error> {
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
    pigeon_git::status::parse(&out.stdout).map_err(|detail| Error::Parse {
        argv: vec!["status".to_owned()],
        detail,
    })
}

/// The status letters of an entry, index then folder (`?` untracked), and
/// its path.
fn letters(e: Entry) -> Option<(char, char, String)> {
    let split = |xy: &str| {
        let mut c = xy.chars();
        (c.next().unwrap_or('.'), c.next().unwrap_or('.'))
    };
    match e {
        Entry::Changed { xy, path, .. } | Entry::Renamed { xy, path, .. } => {
            let (x, y) = split(&xy);
            Some((x, y, path))
        }
        Entry::Untracked { path } | Entry::Ignored { path } => Some(('?', '?', path)),
        Entry::Unmerged { .. } => None,
    }
}

/// The paths whose file differs from the index, untracked ones included:
/// the person's, which install never writes (07 §7.5).
pub async fn dirty(g: &Group) -> Result<std::collections::BTreeSet<String>, Error> {
    Ok(status(g)
        .await?
        .into_iter()
        .filter_map(letters)
        .filter(|(_, y, _)| *y != '.')
        .map(|(_, _, p)| p.trim_end_matches('/').to_owned())
        .collect())
}

/// One round of capture on a laptop. The index may lag `main` where
/// install has not written a version yet: such a path is left out, and the
/// commit is `main`'s tree with the person's changes. A path the person
/// changed there diverges, unless it now holds `main`'s version.
pub async fn capture(g: &Group, tracer: &Tracer) -> Result<Captured, Error> {
    let mut done = Captured::default();
    let Some(worktree) = g.worktree.clone() else {
        return Ok(done);
    };
    let head = g.git.rev("refs/heads/main").await;
    let mut main: Option<Tree> = None;
    let (mut add, mut remove, mut not_shared) = (Vec::new(), Vec::new(), Vec::new());
    let (mut behind, mut pending, mut gone) = (false, Vec::new(), Vec::new());
    for (x, y, path) in status(g).await?.into_iter().filter_map(letters) {
        if is_pigeon_path(&path) {
            continue;
        }
        if y == '.' {
            behind |= x != '.';
            continue;
        }
        let lagging = match x {
            '.' => false,
            '?' => {
                if main.is_none()
                    && let Some(h) = &head
                {
                    main = Some(g.git.tree(h).await?);
                }
                main.as_ref()
                    .is_some_and(|t| t.contains_key(path.trim_end_matches('/')))
            }
            _ => true,
        };
        behind |= lagging;
        if y == 'D' {
            if lagging {
                gone.push(path);
            } else {
                remove.push(path);
            }
            continue;
        }
        match kind(&worktree, &path) {
            Some(k) => not_shared.push((path.trim_end_matches('/').to_owned(), k.to_owned())),
            None => {
                let Ok(Some(st)) = lstat(&worktree.join(&path)) else {
                    continue;
                };
                if !pigeon_sys::writers::scan(st.id).is_clear() {
                    done.busy += 1;
                } else if lagging {
                    pending.push(path);
                } else {
                    add.push(path);
                }
            }
        }
    }
    g.lock().not_shared = not_shared;
    if behind
        && main.is_none()
        && let Some(h) = &head
    {
        main = Some(g.git.tree(h).await?);
    }
    let before = if pending.is_empty() && gone.is_empty() {
        Tree::new()
    } else {
        g.git.index().await?
    };
    let all: Vec<String> = add.iter().chain(&pending).cloned().collect();
    for chunk in all.chunks(64) {
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
    let diverging = settle(g, main.as_ref(), &before, &pending, &gone).await?;
    note_diverging(g, diverging);
    done.added = add.len();
    done.removed = remove.len();
    if done.added + done.removed == 0 {
        return Ok(done);
    }
    let tree = match &main {
        Some(main) if behind => {
            let index = g.git.index().await?;
            let mut tree = main.clone();
            for p in &add {
                match index.get(p) {
                    Some(e) => tree.insert(p.clone(), e.clone()),
                    None => tree.remove(p),
                };
            }
            for p in &remove {
                tree.remove(p);
            }
            crate::merge::write_tree(&g.git, &tree).await?
        }
        _ => g.git.run(&["write-tree"]).await?.trim().to_owned(),
    };
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

/// Settles the paths changed where the index lags `main`: one that now
/// holds `main`'s version, or is gone as `main` has it gone, takes it in
/// the index; any other diverges and keeps its index entry, its content
/// added to the annex. Answers the diverging paths.
async fn settle(
    g: &Group,
    main: Option<&Tree>,
    before: &Tree,
    pending: &[String],
    gone: &[String],
) -> Result<Vec<String>, Error> {
    if pending.is_empty() && gone.is_empty() {
        return Ok(Vec::new());
    }
    let empty = Tree::new();
    let main = main.unwrap_or(&empty);
    let index = g.git.index().await?;
    let mut diverging = Vec::new();
    for p in pending {
        if index.contains_key(p) && index.get(p) == main.get(p) {
            continue;
        }
        let args: Vec<String> = match before.get(p) {
            Some(e) => ["update-index", "--cacheinfo"]
                .map(str::to_owned)
                .into_iter()
                .chain([format!("{},{},{p}", e.mode, e.oid)])
                .collect(),
            None => ["update-index", "--force-remove", "--", p.as_str()]
                .map(str::to_owned)
                .into_iter()
                .collect(),
        };
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        g.git.run(&args).await?;
        diverging.push(p.clone());
    }
    for p in gone {
        if main.contains_key(p) {
            diverging.push(p.clone());
        } else {
            g.git
                .run(&["update-index", "--force-remove", "--", p])
                .await?;
        }
    }
    Ok(diverging)
}

fn note_diverging(g: &Group, paths: Vec<String>) {
    let paths: std::collections::BTreeSet<String> = paths.into_iter().collect();
    let mut k = g.lock();
    if k.diverging != paths {
        tracing::warn!(group = %g.id, paths = ?paths, "diverging");
        k.diverging = paths;
    }
}
