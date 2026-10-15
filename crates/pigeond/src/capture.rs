//! Capture (07, 08 §8.1): every change of the folder becomes a version on
//! `main`, through `git status`, `git annex add` and a commit.
//!
//! Only regular files are shared; links, nested repositories and special
//! files are listed as not shared (07 §7.9). A file that a process may be
//! writing waits for the next round (07 §7.5's writer scan). A version is
//! added with `-c annex.thin=false`, so it stays an immutable copy in the
//! annex until it is protected (03 §3.4). pigeon's own paths are never
//! captured: the roster changes only through `pigeond`. A save within ten
//! seconds of an install of its path, as an application holding the old
//! content makes, goes beside the installed version, under its conflict
//! name, as if the two were concurrent (ADR 0071). A file deleted and one
//! added in one round are a rename when their contents match (`renames`,
//! ADR 0072).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pigeon_core::authority::Tree;
use pigeon_core::names;
use pigeon_git::status::Entry;
use pigeon_sys::fs::lstat;

use crate::capture_file::{Intent, Staged};
use crate::git::{Error, is_pigeon_path};
use crate::group::Group;
use crate::trace::{Act, Tracer};

/// How soon after an install a save of its path counts as concurrent with
/// the installed version (ADR 0071): ten seconds.
pub const STALE_SAVE_NS: i64 = 10_000_000_000;

/// How much earlier than install's record a save may be: install renames
/// the file in, then records.
const STALE_SLACK_NS: i64 = 2_000_000_000;

/// Whether `path`, modified at `mtime_ns`, was saved within
/// [`STALE_SAVE_NS`] of an install of it.
fn soon_after_install(g: &Group, path: &str, mtime_ns: i64) -> bool {
    g.lock()
        .installed
        .get(path)
        .is_some_and(|(at, _)| (at - STALE_SLACK_NS..=at + STALE_SAVE_NS).contains(&mtime_ns))
}

/// Where each save of `soon` goes (ADR 0071): its first free conflict name
/// (05 §5.3) in `main`, with this device's person and the time now, when
/// it changes the path and the person may add that name; the others keep
/// their path. Each record of an install that a save used is dropped.
fn beside(
    g: &Group,
    me: &str,
    main: &Tree,
    index: &Tree,
    soon: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(doc) = g.roster() else {
        return out;
    };
    let Some(person) = doc
        .roster
        .devices
        .get(&pigeon_core::roster::DeviceId(me.to_owned()))
        .map(|d| d.person.clone())
    else {
        return out;
    };
    let who = doc
        .roster
        .persons
        .get(&person)
        .map_or_else(|| person.0.clone(), |p| p.name.clone());
    let utc = crate::util::now();
    let when = names::Stamp {
        utc,
        offset: pigeon_sys::clock::local_offset_minutes(utc),
    };
    let taken: BTreeSet<String> = main.keys().map(|p| names::fold(p)).collect();
    let none = pigeon_core::authority::Writers::new();
    for p in soon {
        if index.get(p).is_none() || index.get(p) == main.get(p) {
            continue;
        }
        g.lock().installed.remove(p);
        let mut n = 1;
        let mut name = names::conflict_name(p, &who, when, n);
        while taken.contains(&names::fold(&name)) || out.values().any(|q| *q == name) {
            n += 1;
            name = names::conflict_name(p, &who, when, n);
        }
        if pigeon_core::authority::allowed(&doc.roster, &none, &person, &name, true) {
            tracing::info!(group = %g.id, path = %p, beside = %name, "saved-soon-after-install");
            out.insert(p.clone(), name);
        }
    }
    out
}

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
    /// Paths suggested in proposals (07 §7.3 step 7).
    pub suggested: usize,
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

/// One round of capture on a laptop `me`. The index may lag `main` where
/// install has not written a version yet: such a path is left out, and the
/// commit is `main`'s tree with the person's changes. A path the person
/// changed there diverges, unless it now holds `main`'s version. Changes
/// the person may not make become proposals (`suggest`); a suggested path
/// saved since is left alone.
pub async fn capture(g: &Group, me: &str, tracer: &Tracer) -> Result<Captured, Error> {
    let mut done = Captured::default();
    let Some(worktree) = g.worktree.clone() else {
        return Ok(done);
    };
    if crate::evict::unsettled(g) {
        // A half-done install, free or keep is no change of the person's;
        // the next round settles it first (07 §7.5).
        return Ok(done);
    }
    recover(g).await?;
    let now = crate::util::now_ns();
    g.lock()
        .installed
        .retain(|_, (at, _)| now - *at <= 6 * STALE_SAVE_NS);
    let table = crate::conflicts_file::read(g).unwrap_or_default();
    let mut held = crate::held_file::read(g);
    let waiting: BTreeSet<String> = held.held().difference(&held.released).cloned().collect();
    let head = g.git.rev("refs/heads/main").await;
    let mut main: Option<Tree> = None;
    let (mut add, mut remove, mut not_shared) = (Vec::new(), Vec::new(), Vec::new());
    let (mut behind, mut pending, mut gone) = (false, Vec::new(), Vec::new());
    let mut soon = BTreeSet::new();
    for (x, y, path) in status(g).await?.into_iter().filter_map(letters) {
        if is_pigeon_path(&path) {
            continue;
        }
        if y == '.' {
            behind |= x != '.';
            continue;
        }
        if crate::suggest::unchanged(&worktree, &table, &path, y == 'D') {
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
            } else if !waiting.contains(&path) {
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
                    if x == '.' && soon_after_install(g, &path, st.mtime_ns) {
                        soon.insert(path.clone());
                    }
                    add.push(path);
                }
            }
        }
    }
    g.lock().not_shared = not_shared;
    // A held path whose file is back is held no more (07 §7.4).
    let back: BTreeSet<String> = waiting
        .iter()
        .filter(|p| lstat(&worktree.join(p.as_str())).is_ok_and(|s| s.is_some()))
        .cloned()
        .collect();
    if !back.is_empty() {
        held.forget(&back);
        save_held(g, &held);
    }
    if behind
        && main.is_none()
        && let Some(h) = &head
    {
        main = Some(g.git.tree(h).await?);
    }
    let all: Vec<String> = add.iter().chain(&pending).cloned().collect();
    if all.is_empty() && remove.is_empty() && gone.is_empty() {
        return Ok(done);
    }
    let before = g.git.index().await?;
    let intent = Intent {
        format: crate::capture_file::FORMAT,
        head: head.clone(),
        before: all
            .iter()
            .chain(&remove)
            .chain(&gone)
            .map(|p| {
                let e = before.get(p).map(|e| Staged {
                    mode: e.mode.clone(),
                    oid: e.oid.clone(),
                });
                (p.clone(), e)
            })
            .collect(),
    };
    crate::capture_file::write(g, &intent).map_err(|source| Error::Pipe {
        argv: vec!["capture.json".to_owned()],
        source,
    })?;
    let batch = Batch {
        held,
        me,
        table,
        head,
        main,
        before,
        soon,
        add,
        remove,
        pending,
        gone,
        all,
    };
    let done = commit_batch(g, tracer, batch, done).await?;
    crate::capture_file::clear(g);
    Ok(done)
}

/// Puts back the index entries of a batch that a crash interrupted
/// (`capture.json`), so that this scan redoes it from the files.
pub async fn recover(g: &Group) -> Result<(), Error> {
    let Some(intent) = crate::capture_file::read(g) else {
        if crate::capture_file::pending(g) {
            return Err(Error::Parse {
                argv: vec!["capture.json".to_owned()],
                detail: "unreadable or unsupported capture intent".to_owned(),
            });
        }
        return Ok(());
    };
    for (p, e) in &intent.before {
        match e {
            Some(e) => {
                let info = format!("{},{},{p}", e.mode, e.oid);
                g.git.run(&["update-index", "--cacheinfo", &info]).await?;
            }
            None => {
                g.git
                    .run(&["update-index", "--force-remove", "--", p])
                    .await?;
            }
        }
    }
    tracing::info!(group = %g.id, paths = intent.before.len(), "capture-redone");
    crate::capture_file::clear(g);
    Ok(())
}

/// What one batch stages and commits.
struct Batch<'a> {
    /// The held deletions (07 §7.4).
    held: crate::held_file::Table,
    me: &'a str,
    table: crate::conflicts_file::Table,
    head: Option<String>,
    main: Option<Tree>,
    before: Tree,
    /// The paths saved soon after an install of them (ADR 0071).
    soon: BTreeSet<String>,
    add: Vec<String>,
    remove: Vec<String>,
    pending: Vec<String>,
    gone: Vec<String>,
    all: Vec<String>,
}

/// Stages a batch and commits it on `main` (07 §7.3 steps 3–7).
async fn commit_batch(
    g: &Group,
    tracer: &Tracer,
    b: Batch<'_>,
    mut done: Captured,
) -> Result<Captured, Error> {
    let Batch {
        mut held,
        me,
        mut table,
        head,
        mut main,
        before,
        soon,
        add,
        remove,
        pending,
        gone,
        all,
    } = b;
    let remove = hold_mass_deletions(g, &mut held, &before, &add, remove).await?;
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
    let settled = settle(g, head.as_deref(), main.as_ref(), &before, &pending, &gone).await?;
    note_diverging(g, settled.diverging);
    done.added = add.len();
    done.removed = remove.len();
    let head = match settled.moved {
        Some(m) => {
            main = Some(g.git.tree(&m).await?);
            done.commit = Some(m.clone());
            Some(m)
        }
        None => head,
    };
    if done.added + done.removed == 0 {
        return Ok(done);
    }
    let (Some(h), Some(worktree)) = (&head, &g.worktree) else {
        let tree = g.git.run(&["write-tree"]).await?.trim().to_owned();
        return commit_main(g, tree, head, done, &[]).await;
    };
    let main = match main {
        Some(t) => t,
        None => g.git.tree(h).await?,
    };
    let index = g.git.index().await?;
    let beside = beside(g, me, &main, &index, &soon);
    let changes: crate::suggest::Changes = add
        .iter()
        .map(|p| {
            let at = beside.get(p).unwrap_or(p);
            (at.clone(), index.get(p).cloned())
        })
        .chain(remove.iter().map(|p| (p.clone(), None)))
        .collect();
    let (allowed, proposals) =
        crate::suggest::plan(g, me, h, &main, &table, changes.clone()).await?;
    let known = table.clone();
    for p in proposals {
        let commit = crate::proposal::write(g, me, &p.parent, &p.tree).await?;
        crate::suggest::record(g, worktree, &main, &mut table, &commit, &p.paths, &changes).await?;
        done.suggested += p.paths.len();
    }
    table.paths.retain(|p, d| {
        !(allowed.contains_key(p) && d.kind == crate::conflicts_file::Kind::Suggestion)
    });
    if table != known
        && let Err(e) = crate::conflicts_file::write(g, &table)
    {
        tracing::warn!(group = %g.id, error = %e, "conflicts-write-failed");
    }
    done.added = allowed.values().filter(|e| e.is_some()).count();
    done.removed = allowed.len() - done.added;
    if allowed.is_empty() {
        return Ok(done);
    }
    let renames = crate::renames::detect(g, &main, &allowed).await;
    let tree = crate::merge::write_tree(&g.git, &crate::suggest::applied(&main, &allowed)).await?;
    commit_main(g, tree, head, done, &renames).await
}

/// Commits `tree` on `main` at `head`, unless it is `main`'s tree, with a
/// trailer for each of `renames` (ADR 0072).
async fn commit_main(
    g: &Group,
    tree: String,
    head: Option<String>,
    mut done: Captured,
    renames: &[(String, String)],
) -> Result<Captured, Error> {
    let head_tree = g.git.rev("refs/heads/main^{tree}").await;
    if head_tree.as_deref() == Some(tree.as_str()) {
        return Ok(done);
    }
    let parents: Vec<&str> = head.iter().map(String::as_str).collect();
    let commit = g
        .git
        .commit_tree(&tree, &parents, &crate::renames::message(renames))
        .await?;
    pigeon_sys::lab::crash_point("capture-before-ref");
    g.git
        .update_ref("refs/heads/main", &commit, head.as_deref())
        .await?;
    tracing::info!(group = %g.id, commit = %commit, added = done.added, removed = done.removed, renamed = renames.len(), "captured");
    done.commit = Some(commit);
    Ok(done)
}

/// What [`settle`] did.
struct Settled {
    /// The paths that diverge still.
    diverging: Vec<String>,
    /// `main` after the merges of the person's saves, if it moved.
    moved: Option<String>,
}

/// Settles the paths changed where the index lags `main`: one that now
/// holds `main`'s version, or is gone as `main` has it gone, takes it in
/// the index. Any other save becomes a contribution concurrent with
/// `main`'s version, merged into `main` (`merge::concurrent`), so the
/// catalogue keeps both (05 §5.2); if that merge is not accepted, the path
/// diverges and keeps its index entry, its content added to the annex.
async fn settle(
    g: &Group,
    head: Option<&str>,
    main: Option<&Tree>,
    before: &Tree,
    pending: &[String],
    gone: &[String],
) -> Result<Settled, Error> {
    let mut done = Settled {
        diverging: Vec::new(),
        moved: None,
    };
    if pending.is_empty() && gone.is_empty() {
        return Ok(done);
    }
    let empty = Tree::new();
    let main = main.unwrap_or(&empty);
    let index = g.git.index().await?;
    let mut tip = head.map(str::to_owned);
    for p in pending.iter().chain(gone) {
        let deleted = gone.contains(p);
        let mine = if deleted { None } else { index.get(p) };
        if mine.is_some() && mine == main.get(p) || deleted && !main.contains_key(p) {
            if deleted {
                g.git
                    .run(&["update-index", "--force-remove", "--", p])
                    .await?;
            }
            continue;
        }
        if let Some(t) = tip.clone() {
            match crate::merge::concurrent(g, &t, p, before.get(p), mine).await {
                Ok(Some(m)) => {
                    g.git.update_ref("refs/heads/main", &m, Some(&t)).await?;
                    tracing::info!(group = %g.id, path = %p, commit = %m, "concurrent-save");
                    if deleted {
                        g.git
                            .run(&["update-index", "--force-remove", "--", p])
                            .await?;
                    }
                    tip = Some(m.clone());
                    done.moved = Some(m);
                    continue;
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(group = %g.id, path = %p, error = %e, "concurrent-failed"),
            }
        }
        if !deleted {
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
        }
        done.diverging.push(p.clone());
    }
    Ok(done)
}

/// Holds the deletions of a batch that 07 §7.3 step 3 holds, once its
/// additions are in the index: a deletion whose content a new path holds
/// is a rename (ADR 0072) and goes through. Answers the deletions that do.
async fn hold_mass_deletions(
    g: &Group,
    held: &mut crate::held_file::Table,
    before: &Tree,
    add: &[String],
    remove: Vec<String>,
) -> Result<Vec<String>, Error> {
    use pigeon_core::held::{MASS_DELETION, split, top};
    if remove.is_empty() {
        return Ok(remove);
    }
    let index = g.git.index().await?;
    let added: BTreeSet<&str> = add
        .iter()
        .filter_map(|p| index.get(p))
        .map(|e| e.oid.as_str())
        .collect();
    let (mut through, rest): (Vec<String>, Vec<String>) = remove.into_iter().partition(|p| {
        before
            .get(p)
            .is_some_and(|e| added.contains(e.oid.as_str()))
    });
    let worktree = g.worktree.clone().unwrap_or_default();
    let gone: BTreeSet<String> = rest
        .iter()
        .filter_map(|p| top(p))
        .filter(|t| !worktree.join(t).exists())
        .map(str::to_owned)
        .collect();
    let rest: BTreeSet<String> = rest.into_iter().collect();
    let s = split(MASS_DELETION, &rest, &held.held(), &held.released, &gone);
    through.extend(s.delete.iter().cloned());
    let mut changed = !s.delete.is_disjoint(&held.released);
    held.released.retain(|p| !s.delete.contains(p));
    if let Some(why) = &s.why {
        tracing::warn!(group = %g.id, files = s.hold.len(), why = ?why, "deletions-held");
        held.hold(why, s.hold, crate::util::now());
        changed = true;
    }
    if changed {
        save_held(g, held);
    }
    Ok(through)
}

fn save_held(g: &Group, held: &crate::held_file::Table) {
    if let Err(e) = crate::held_file::write(g, held) {
        tracing::warn!(group = %g.id, error = %e, "held-write-failed");
    }
}

fn note_diverging(g: &Group, paths: Vec<String>) {
    let paths: std::collections::BTreeSet<String> = paths.into_iter().collect();
    let mut k = g.lock();
    if k.diverging != paths {
        tracing::warn!(group = %g.id, paths = ?paths, "diverging");
        k.diverging = paths;
    }
}
