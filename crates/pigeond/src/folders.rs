//! The answer to `Folders` (ux.md §6.2): the group's top-level folders,
//! each with its files, those not safe yet, its least safe state, and
//! whether its space is freed on this computer (08 §8.5). Files at the top
//! of the group's folder belong to no folder and are not listed. And the
//! answer to `Files`: the files under one folder, with their states, sizes,
//! whether each is freed and, for those on this computer, when they last
//! changed; and to `Deleted`: the files deleted from `main`.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::headline::Level;
use pigeon_proto::Response;
use pigeon_proto::api::{DeletedFile, FileEntry, Folder};

use crate::group::Group;

/// The top-level folder of `path`, if it is in one.
fn top(path: &str) -> Option<&str> {
    path.split_once('/').map(|(t, _)| t)
}

/// The size of `o`'s current version: its key's, or for a save not yet
/// captured, the file's on disk.
fn size(g: &Group, o: &crate::status::Observed) -> Option<u64> {
    crate::restore::key_size(&o.key).or_else(|| {
        let at = g.worktree.as_ref()?.join(&o.path);
        std::fs::symlink_metadata(at)
            .ok()
            .filter(std::fs::Metadata::is_file)
            .map(|m| m.len())
    })
}

/// When the copy of `path` on this computer last changed, if it is a
/// regular file there.
fn modified(g: &Group, path: &str) -> Option<String> {
    let at = g.worktree.as_ref()?.join(path);
    let time = std::fs::symlink_metadata(at)
        .ok()
        .filter(std::fs::Metadata::is_file)?
        .modified()
        .ok()?;
    let secs = time.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(crate::util::rfc3339(i64::try_from(secs).ok()?))
}

/// Files pages include uncaptured local saves, even while a round is busy.
/// Their display entries never claim an accepted key or publication.
async fn listing(
    g: &Group,
    me: &str,
    prefix: &str,
) -> Result<(Vec<crate::status::Observed>, BTreeSet<String>), crate::git::Error> {
    let mut observed: BTreeMap<_, _> = crate::status::observe_checked(g, me, prefix)
        .await?
        .into_iter()
        .map(|o| (o.path.clone(), o))
        .collect();
    let pending = crate::capture::pending(g, prefix).await?;
    for path in &pending {
        observed.insert(
            path.clone(),
            crate::status::Observed {
                path: path.clone(),
                key: String::new(),
                local: true,
                state: pigeon_core::state::State::Local(pigeon_core::state::Reason::Sending),
                level: Level::Working,
                since: crate::util::now(),
                silent: None,
            },
        );
    }
    Ok((observed.into_values().collect(), pending))
}

/// The group's folders, as `me` sees them.
pub async fn folders(g: &Group, me: &str) -> Response {
    let (observed, pending) = match listing(g, me, "").await {
        Ok(listing) => listing,
        Err(error) => return crate::api::problem("internal-error", &[], Some(error.to_string())),
    };
    let targets = crate::evict::targets(g);
    let mut by: BTreeMap<&str, Vec<&crate::status::Observed>> = BTreeMap::new();
    for o in &observed {
        if let Some(t) = top(&o.path) {
            by.entry(t).or_default().push(o);
        }
    }
    let n = |x: usize| u64::try_from(x).unwrap_or(u64::MAX);
    let folders = by
        .into_iter()
        .map(|(path, files)| {
            let freed = targets.is_freed(path) && files.iter().all(|o| targets.is_freed(&o.path));
            let arriving = if freed {
                0
            } else {
                files
                    .iter()
                    .filter(|o| {
                        !pending.contains(&o.path)
                            && !targets.is_freed(&o.path)
                            && (!o.local
                                || !g.worktree.as_ref().is_some_and(|wt| {
                                    std::fs::symlink_metadata(wt.join(&o.path))
                                        .is_ok_and(|m| m.is_file())
                                }))
                    })
                    .count()
            };
            Folder {
                path: path.to_owned(),
                files: n(files.len()),
                not_safe: n(files.iter().filter(|o| o.level != Level::Safe).count()),
                state: crate::status::worst(files.iter().map(|o| &o.state)),
                freed,
                arriving: n(arriving),
                size: Some(files.iter().filter_map(|o| size(g, o)).sum()),
            }
        })
        .collect();
    Response::Folders {
        root: root(g),
        folders,
    }
}

/// The files under `prefix`, a folder of the group, as `me` sees them.
pub async fn files(g: &Group, me: &str, prefix: &str) -> Response {
    let (observed, _) = match listing(g, me, prefix).await {
        Ok(listing) => listing,
        Err(error) => return crate::api::problem("internal-error", &[], Some(error.to_string())),
    };
    // A file is not on this computer once freeing hid it (08 §8.5).
    let targets = crate::evict::targets(g);
    let hidden = crate::evict::hidden_paths(g).await;
    let files = observed
        .into_iter()
        .map(|o| FileEntry {
            state: crate::status::worst(std::iter::once(&o.state)),
            size: size(g, &o),
            modified: modified(g, &o.path),
            freed: hidden.contains(&o.path) && targets.is_freed(&o.path),
            path: o.path,
        })
        .collect();
    Response::Files {
        root: root(g),
        files,
    }
}

/// The files deleted from `main` under `prefix` (all with ""), newest
/// deletion first: those the current and verified archive histories delete
/// among their last 1000 commits, and the current tree does not hold
/// again.
pub async fn deleted(g: &Group, prefix: &str) -> Response {
    let mut commits = Vec::new();
    for tip in crate::history::tips(g).await {
        let limit = format!("--max-count={}", 1000 - commits.len());
        let list = g
            .git
            .run(&["rev-list", &limit, &tip])
            .await
            .unwrap_or_default();
        commits.extend(list.lines().map(str::to_owned));
        if commits.len() >= 1000 {
            break;
        }
    }
    let commits = format!("{}\n", commits.join("\n"));
    let out = g
        .git
        .run_in(
            &[
                "diff-tree",
                "--stdin",
                "-r",
                "--no-renames",
                "--diff-filter=D",
                "--name-only",
                "--format=%x00%ct",
            ],
            commits.as_bytes(),
        )
        .await
        .unwrap_or_default();
    let now: std::collections::BTreeSet<String> = g
        .git
        .run(&["ls-tree", "-r", "--name-only", "refs/heads/main"])
        .await
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    for commit in out.split('\0').filter(|c| !c.trim().is_empty()) {
        let mut lines = commit.lines();
        let at = lines
            .next()
            .and_then(|t| t.trim().parse::<i64>().ok())
            .unwrap_or(0);
        for path in lines.map(str::trim).filter(|p| !p.is_empty()) {
            let hidden = path.starts_with(".pigeon/") || path == ".gitattributes";
            let wanted = crate::evict::under(path, prefix);
            if !hidden && wanted && !now.contains(path) && seen.insert(path.to_owned()) {
                files.push(DeletedFile {
                    path: path.to_owned(),
                    deleted_at: crate::util::rfc3339(at),
                });
            }
        }
    }
    Response::Deleted {
        root: root(g),
        files,
    }
}

fn root(g: &Group) -> String {
    g.worktree
        .as_ref()
        .map(|w| w.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::top;

    #[test]
    fn a_path_belongs_to_its_first_folder() {
        assert_eq!(top("Photos/2019/a.jpg"), Some("Photos"));
        assert_eq!(top("Photos/a.jpg"), Some("Photos"));
        assert_eq!(top("budget.xlsx"), None);
    }
}
