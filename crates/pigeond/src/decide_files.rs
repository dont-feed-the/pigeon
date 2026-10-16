//! Deciding a suggestion file by file (ADR 0075, 04 §4.5):
//! `pigeon accept ID PATH…` accepts the files named and declines the
//! others, `pigeon decline ID PATH…` the reverse, in one decision, which
//! closes the proposal. Each path must be one of the proposal's; a rename
//! is decided with its old name (`suggestion_files::together`).
//!
//! When every file is accepted it is the adoption of `decide`, and when
//! none is, the decision `rejected`. Otherwise one signed commit on `main`,
//! not a merge, holds the change of the accepted files as the adoption
//! would (`proposal::partial`) and adds the decision `rejected` with the
//! note; its message names each accepted file in a `Pigeon-Accepted:`
//! trailer, with `Pigeon-Force:` when nobody may make an accepted change
//! and an admin forces it (04 §4.6). Like an adoption, it records no
//! rename: a proposal carries none. The authority rule judges it as any commit: the
//! decider may make every accepted change, and adds the decision as one of
//! the proposal's deciders. An older release reads the decision as a whole
//! decline, and its install keeps the files `main` now holds as the
//! suggester saved them (07 §7.6), so nothing is misread on disk.

use std::collections::BTreeSet;
use std::path::Path;

use pigeon_core::authority::{self, Decided};
use pigeon_core::names::Stamp;
use pigeon_proto::{Arg, Response};

use crate::daemon::Daemon;
use crate::decide::Pending;
use crate::group::Group;

fn problem(code: &str, args: &[(&str, Arg)]) -> Vec<Response> {
    vec![crate::api::problem(code, args, None)]
}

fn internal(e: impl ToString) -> Vec<Response> {
    vec![crate::api::problem(
        "internal-error",
        &[],
        Some(e.to_string()),
    )]
}

/// `pigeon accept ID PATH…` (`accept`) or `pigeon decline ID PATH…`.
pub async fn decide(
    d: &Daemon,
    id: &str,
    paths: &[String],
    accept: bool,
    note: Option<&str>,
    reason: Option<&str>,
) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return problem("not-in-group", &[]);
    };
    let p = match crate::decide::decided(d, &g, id).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    let named = match chosen(&g, &p, paths).await {
        Ok(n) => n,
        Err(r) => return r,
    };
    let (accepted, declined): (BTreeSet<String>, BTreeSet<String>) = p
        .paths
        .iter()
        .cloned()
        .partition(|q| named.contains(q) == accept);
    let done = if declined.is_empty() {
        crate::decide::adopt_now(d, &g, &p, reason).await
    } else if accepted.is_empty() {
        crate::decide::decline_now(d, &g, &p, note).await
    } else {
        part_now(d, &g, &p, &accepted, note, reason).await
    };
    if let Err(r) = done {
        return r;
    }
    let author = crate::decide::author(&g, &p);
    let list = |s: &BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>().join(", ");
    let count = |s: &BTreeSet<String>| Arg::Number(i64::try_from(s.len()).unwrap_or(i64::MAX));
    let said = if declined.is_empty() {
        crate::api::said(
            "decided-all-accepted",
            &[
                ("author", Arg::Text(author)),
                ("count", count(&accepted)),
                ("paths", Arg::Text(list(&accepted))),
            ],
        )
    } else if accepted.is_empty() {
        crate::api::said(
            "decided-all-declined",
            &[
                ("author", Arg::Text(author)),
                ("count", count(&declined)),
                ("paths", Arg::Text(list(&declined))),
            ],
        )
    } else {
        crate::api::said(
            "decided-some",
            &[
                ("author", Arg::Text(author)),
                ("accepted", count(&accepted)),
                ("accepted-paths", Arg::Text(list(&accepted))),
                ("declined", count(&declined)),
                ("declined-paths", Arg::Text(list(&declined))),
            ],
        )
    };
    vec![said]
}

/// The paths of `p` that `paths` name, each with the paths decided with
/// it; a path that is not one of them is a problem, and so is none.
async fn chosen(
    g: &Group,
    p: &Pending,
    paths: &[String],
) -> Result<BTreeSet<String>, Vec<Response>> {
    if paths.is_empty() {
        return Err(problem("no-files-chosen", &[]));
    }
    let rows = crate::suggestion_files::files(g, &p.at, &p.tree).await;
    let together = crate::suggestion_files::together(&rows);
    let mut out = BTreeSet::new();
    for given in paths {
        let path = Path::new(given);
        let rel = if path.is_absolute() {
            // A suggested new file in a new folder has no parent on disk
            // here: its path within the folder is then read as written.
            crate::api::relative(g, path).or_else(|| {
                let wt = g.worktree.as_ref()?;
                let rel = path.strip_prefix(wt).ok()?.to_str()?;
                Some(rel.to_owned())
            })
        } else {
            Some(given.trim_start_matches("./").to_owned())
        };
        match rel.filter(|r| p.paths.contains(r)) {
            Some(r) => out.extend(together.get(&r).cloned().unwrap_or_else(|| vec![r])),
            None => {
                return Err(problem(
                    "suggestion-file-unknown",
                    &[("path", Arg::Text(given.clone()))],
                ));
            }
        }
    }
    Ok(out)
}

/// Commits the decision on some files of `p` (ADR 0075): `accepted` taken,
/// the others declined with `note`.
async fn part_now(
    d: &Daemon,
    g: &Group,
    p: &Pending,
    accepted: &BTreeSet<String>,
    note: Option<&str>,
    reason: Option<&str>,
) -> Result<String, Vec<Response>> {
    let me = crate::decide::me(d, g).ok_or_else(|| problem("not-allowed", &[]))?;
    let busy = g.busy.lock().await;
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| problem("not-in-group", &[]))?;
    let (ours, tree) = partial_tree(g, &main, p, accepted)
        .await
        .map_err(internal)?
        .ok_or_else(|| problem("not-allowed", &[]))?;
    let doc = g.roster().ok_or_else(|| problem("not-in-group", &[]))?;
    let writers = crate::merge::writers(g, &main).await.map_err(internal)?;
    let changes: Vec<(String, bool)> = authority::delta(&ours, &tree)
        .into_iter()
        .map(|q| {
            let added = !ours.contains_key(&q);
            (q, added)
        })
        .collect();
    let changes: Vec<(&str, bool)> = changes.iter().map(|(q, a)| (q.as_str(), *a)).collect();
    let (deciders, needs) = authority::deciders(&doc.roster, &writers, &changes);
    let force = match (needs || !deciders.contains(&me), reason) {
        (true, None) => return Err(problem("needs-override", &[])),
        (true, Some(r)) => Some(r),
        (false, _) => None,
    };
    let commit = write(g, &main, p, accepted, tree, note, force)
        .await
        .map_err(internal)?;
    tracing::info!(group = %g.id, proposal = %p.id, commit = %commit,
        accepted = accepted.len(), "suggestion-decided-in-part");
    drop(busy);
    crate::api::wake_soon(d);
    Ok(commit)
}

/// `main`'s catalogue, B(c) of the decision, and the tree on it that holds
/// the change of `accepted`, files of `p`, as the adoption makes it
/// (`proposal::partial`); `None` when `main` is not judged accepted.
async fn partial_tree(
    g: &Group,
    main: &str,
    p: &Pending,
    accepted: &BTreeSet<String>,
) -> Result<Option<(authority::Tree, authority::Tree)>, crate::git::Error> {
    crate::merge::judge_history(g, &[main.to_owned(), p.id.clone()]).await?;
    let ours = g
        .lock()
        .engine
        .base(&[main.to_owned()])
        .map(|v| v.tree.clone());
    let Some(ours) = ours else {
        return Ok(None);
    };
    let raw = g.git.object(&p.id).await?;
    let (utc, offset) = authority::author_time(&raw).unwrap_or((0, 0));
    let who = crate::decide::author(g, p);
    let who = if who.is_empty() { "?".to_owned() } else { who };
    let tree = pigeon_core::proposal::partial(
        &p.at,
        &p.tree,
        &ours,
        &who,
        Stamp { utc, offset },
        accepted,
    );
    Ok(Some((ours, tree)))
}

/// Commits on `main` the decision on some files of `p`, signed with this
/// device's key: `accepted` taken, the others declined with `note`, with
/// Force when `force` gives why; the caller holds `busy`. The commit must
/// be accepted.
pub async fn part(
    g: &Group,
    p: &Pending,
    accepted: &BTreeSet<String>,
    note: Option<&str>,
    force: Option<&str>,
) -> Result<String, String> {
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| "no main".to_owned())?;
    let (_, tree) = partial_tree(g, &main, p, accepted)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "main is not accepted".to_owned())?;
    write(g, &main, p, accepted, tree, note, force).await
}

/// Writes, judges and moves `main` to the decision on some files of `p`
/// whose tree, without the decision, is `tree`; the caller holds `busy`.
/// The commit must be accepted.
async fn write(
    g: &Group,
    main: &str,
    p: &Pending,
    accepted: &BTreeSet<String>,
    mut tree: authority::Tree,
    note: Option<&str>,
    force: Option<&str>,
) -> Result<String, String> {
    let (path, entry) = crate::decide::decision_entry(g, &p.id, Decided::Rejected, note).await?;
    tree.insert(path, entry);
    let oid = crate::merge::write_tree(&g.git, &tree)
        .await
        .map_err(|e| e.to_string())?;
    let mut trailers: Vec<String> = accepted
        .iter()
        .map(|q| authority::accepted_trailer(q))
        .collect();
    if let Some(why) = force {
        trailers.push(format!("{} {why}", authority::FORCE));
    }
    let message = format!(
        "pigeon: accept part of a suggestion\n\n{}",
        trailers.join("\n")
    );
    let commit = g
        .git
        .commit_tree(&oid, &[main], &message)
        .await
        .map_err(|e| e.to_string())?;
    crate::merge::judge_history(g, std::slice::from_ref(&commit))
        .await
        .map_err(|e| e.to_string())?;
    if !g.lock().engine.accepted(&commit) {
        let v = g.lock().verdicts.get(&commit).cloned();
        return Err(format!("not accepted: {v:?}"));
    }
    g.git
        .update_ref("refs/heads/main", &commit, Some(main))
        .await
        .map_err(|e| e.to_string())?;
    Ok(commit)
}
