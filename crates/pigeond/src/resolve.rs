//! Divergences (07 §7.6, ux.md §8.8): `pigeon conflicts` lists this
//! laptop's divergence table, and `pigeon resolve PATH` ends one.
//!
//! *Take the group's version* installs `main`'s version over the person's
//! file with install's `put` or `remove`, which move the file aside and
//! check that it still holds the key the table recorded, after a check of
//! its stat; a pending proposal of theirs is withdrawn by the decision
//! `withdrawn` on `main` (04 §4.5), and their version stays in history.
//! *Keep mine as a copy* first copies that key's content to DEST, a path
//! they may add, where capture takes it. *Suggest again* makes a proposal
//! of their version on the current `main`, waiting. A file saved since the
//! table recorded it is never replaced (`changed-since`).

use std::path::Path;

use pigeon_core::authority::{self, Entry};
use pigeon_core::roster::DeviceId;
use pigeon_proto::api::{Choice, Divergence as Row};
use pigeon_proto::{Arg, Response};

use crate::conflicts_file::{Divergence, Kind, State};
use crate::daemon::Daemon;
use crate::group::Group;
use crate::install::Put;

fn problem(code: &str, path: &str) -> Vec<Response> {
    vec![crate::api::problem(
        code,
        &[("path", Arg::Text(path.to_owned()))],
        None,
    )]
}

fn internal(e: impl ToString) -> Vec<Response> {
    vec![crate::api::problem(
        "internal-error",
        &[],
        Some(e.to_string()),
    )]
}

/// `pigeon conflicts`.
pub async fn list(d: &Daemon) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![crate::api::problem("not-in-group", &[], None)];
    };
    let Some(table) = crate::conflicts_file::read(&g) else {
        return internal("conflicts.json is unreadable");
    };
    let conflicts = table
        .paths
        .iter()
        .map(|(p, v)| Row {
            path: p.clone(),
            kind: match v.kind {
                Kind::Suggestion => "suggestion",
                Kind::Concurrent => "concurrent",
                Kind::Recovered => "recovered",
            }
            .to_owned(),
            state: match v.state {
                State::Waiting => "waiting",
                State::Conflict => "conflict",
            }
            .to_owned(),
            since: v.since.clone(),
        })
        .collect();
    vec![Response::Conflicts { conflicts }]
}

fn stat_now(wt: &Path, rel: &str) -> Option<[i128; 4]> {
    crate::evict::stat_of(&wt.join(rel))
        .map(|s| [s.dev.into(), s.ino.into(), s.size.into(), s.mtime_ns])
}

/// The pointer blob of `key`, written to the object store, as an entry
/// with the file's executable bit.
async fn pointer(g: &Group, key: &str, file: &Path) -> Result<Entry, String> {
    let text = format!("/annex/objects/{key}\n");
    let oid = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let exec = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(file).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    };
    Ok(Entry {
        mode: if exec { "100755" } else { "100644" }.to_owned(),
        kind: "blob".to_owned(),
        oid: oid.trim().to_owned(),
    })
}

/// `pigeon resolve PATH` with `choice`.
pub async fn resolve(d: &Daemon, path: &Path, choice: &Choice) -> Vec<Response> {
    let shown = path.display().to_string();
    let Some(g) = d.group_at(path).or_else(|| d.group_at(path.parent()?)) else {
        return vec![crate::api::problem("not-in-group", &[], None)];
    };
    let (Some(wt), Some(rel)) = (g.worktree.clone(), crate::api::relative(&g, path)) else {
        return vec![crate::api::problem("not-in-group", &[], None)];
    };
    let busy = g.busy.lock().await;
    let Some(mut table) = crate::conflicts_file::read(&g) else {
        return internal("conflicts.json is unreadable");
    };
    let Some(div) = table.paths.get(&rel).cloned() else {
        return problem("not-diverging", &shown);
    };
    if stat_now(&wt, &rel) != div.mine_stat {
        return problem("changed-since", &shown);
    }
    let said = match choice {
        Choice::TakeGroup => take(d, &g, &rel, &div).await.map(|()| {
            crate::api::said("resolved-take-group", &[("path", Arg::Text(shown.clone()))])
        }),
        Choice::KeepCopy { dest } => match copy(d, &g, &rel, &div, Path::new(dest)).await {
            Err(r) => return r,
            Ok(dest) => take(d, &g, &rel, &div).await.map(|()| {
                crate::api::said(
                    "resolved-keep-copy",
                    &[
                        ("path", Arg::Text(shown.clone())),
                        ("dest", Arg::Text(dest)),
                    ],
                )
            }),
        },
        Choice::SuggestAgain => again(d, &g, &rel, &div, &mut table).await.map(|()| {
            crate::api::said(
                "resolved-suggest-again",
                &[("path", Arg::Text(shown.clone()))],
            )
        }),
    };
    let said = match said {
        Ok(s) => s,
        Err(Some(code)) => return problem(code, &shown),
        Err(None) => return internal("resolve failed; see the daemon's log"),
    };
    if !matches!(choice, Choice::SuggestAgain) {
        table.paths.remove(&rel);
    }
    if let Err(e) = crate::conflicts_file::write(&g, &table) {
        return internal(e);
    }
    tracing::info!(group = %g.id, path = %rel, choice = ?choice, "resolved");
    drop(busy);
    crate::api::wake_soon(d);
    vec![said]
}

/// Withdraws `div`'s proposal if it is pending: `main` neither contains it
/// nor holds a decision on it.
async fn withdraw(g: &Group, div: &Divergence) -> Result<(), Option<&'static str>> {
    let (Kind::Suggestion, Some(c)) = (div.kind, &div.proposal) else {
        return Ok(());
    };
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return Ok(());
    };
    let tree = g.git.tree(&main).await.map_err(|_| None)?;
    if tree.contains_key(&authority::decision_path(c)) || g.git.contains(&main, c).await {
        return Ok(());
    }
    match crate::decide::decision(g, c, authority::Decided::Withdrawn, None).await {
        Ok(commit) => {
            tracing::info!(group = %g.id, proposal = %c, commit = %commit, "suggestion-withdrawn");
            Ok(())
        }
        Err(e) => {
            tracing::warn!(group = %g.id, proposal = %c, error = %e, "withdraw-failed");
            Err(None)
        }
    }
}

/// *Take the group's version*: `main`'s version replaces the person's file,
/// which must still hold the key the table recorded.
async fn take(
    d: &Daemon,
    g: &Group,
    rel: &str,
    div: &Divergence,
) -> Result<(), Option<&'static str>> {
    let main = g.git.rev("refs/heads/main").await.ok_or(None)?;
    let want = g.git.tree(&main).await.map_err(|_| None)?;
    let entry = want.get(rel);
    let key = match entry {
        Some(e) => {
            let blob = g.git.object(&e.oid).await.map_err(|_| None)?;
            let key = crate::git::pointer_key(&blob).ok_or(None)?;
            crate::restore::bring(g, &key)
                .await
                .map_err(|_| Some("receiving"))?;
            Some(key)
        }
        None => None,
    };
    let done = match (&key, &div.mine) {
        (Some(k), mine) => crate::install::put(g, rel, k, mine.as_deref(), entry, &d.tracer).await,
        (None, Some(mine)) => crate::install::remove(g, rel, mine, &d.tracer).await,
        (None, None) => crate::install::index_entry(g, rel, None)
            .await
            .map(|()| Put::Done),
    };
    match done {
        Ok(Put::Done) => withdraw(g, div).await,
        Ok(_) => Err(Some("changed-since")),
        Err(e) => {
            tracing::warn!(group = %g.id, path = %rel, error = %e, "resolve-install-failed");
            Err(None)
        }
    }
}

/// *Keep mine as a copy*: the captured key's content, never the live file,
/// goes to `dest`, a new path this person may add; capture then takes it.
async fn copy(
    d: &Daemon,
    g: &Group,
    rel: &str,
    div: &Divergence,
    dest: &Path,
) -> Result<String, Vec<Response>> {
    let shown = dest.display().to_string();
    let Some(mine) = &div.mine else {
        return Err(problem("not-diverging", rel));
    };
    let Some(to) = crate::api::relative(g, dest).filter(|t| t != rel) else {
        return Err(vec![crate::api::problem("not-allowed", &[], None)]);
    };
    let wt = g
        .worktree
        .clone()
        .ok_or_else(|| problem("not-in-group", rel))?;
    if std::fs::symlink_metadata(wt.join(&to)).is_ok() {
        return Err(problem("dest-exists", &shown));
    }
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| internal("no main"))?;
    let tree = g.git.tree(&main).await.map_err(internal)?;
    let entry = pointer(g, mine, &wt.join(rel)).await.map_err(internal)?;
    let doc = g.roster().ok_or_else(|| internal("no roster"))?;
    let changes = std::collections::BTreeMap::from([(to.clone(), Some(entry))]);
    let split = authority::split(&doc.roster, &DeviceId(d.device.clone()), &tree, &changes);
    if tree.contains_key(&to) || !split.proposed.is_empty() {
        return Err(vec![crate::api::problem("not-allowed", &[], None)]);
    }
    let object = crate::custody::present(g, mine)
        .await
        .ok_or_else(|| problem("receiving", rel))?;
    let target = wt.join(&to);
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(internal)?;
    }
    let stage = target.with_file_name(format!(".pigeon-copy-{}", std::process::id()));
    std::fs::copy(&object, &stage).map_err(internal)?;
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(0o644));
    }
    match pigeon_sys::fs::rename_noreplace(&stage, &target) {
        Ok(pigeon_sys::fs::Renamed::Done) => Ok(shown),
        _ => {
            let _ = std::fs::remove_file(&stage);
            Err(problem("dest-exists", &shown))
        }
    }
}

/// *Suggest again*: a new proposal of the person's version on the current
/// `main`, waiting; a pending one of theirs is withdrawn first.
async fn again(
    d: &Daemon,
    g: &Group,
    rel: &str,
    div: &Divergence,
    table: &mut crate::conflicts_file::Table,
) -> Result<(), Option<&'static str>> {
    let wt = g.worktree.clone().ok_or(None)?;
    let main = g.git.rev("refs/heads/main").await.ok_or(None)?;
    let tree = g.git.tree(&main).await.map_err(|_| None)?;
    let entry = match &div.mine {
        Some(k) => Some(pointer(g, k, &wt.join(rel)).await.map_err(|_| None)?),
        None => None,
    };
    let changes = std::collections::BTreeMap::from([(rel.to_owned(), entry)]);
    withdraw(g, div).await?;
    let me = d.device.clone();
    let commit = crate::proposal::write(g, &me, &main, &crate::suggest::applied(&tree, &changes))
        .await
        .map_err(|_| None)?;
    crate::suggest::record(g, &wt, &tree, table, &commit, &[rel.to_owned()], &changes)
        .await
        .map_err(|_| None)?;
    if let Some(v) = table.paths.get_mut(rel) {
        v.told = false;
    }
    Ok(())
}
