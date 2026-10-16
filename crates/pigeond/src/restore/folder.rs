//! Restore one accepted folder snapshot through the install journal
//! (10 §10.1, ADR 0034). No deletion is requested. Preflight downloads
//! and total staging space precede writes; races produce partial results.

use super::{instant, key_size, text};
use crate::{api::problem, daemon::Daemon, group::Group, install::Put};
use pigeon_core::restore::{Snapshot, under};
use pigeon_proto::{Arg, Response};
use std::collections::BTreeSet;

async fn commits(g: &Group, rel: &str) -> Result<Vec<String>, String> {
    let mut commits = Vec::new();
    let mut seen = BTreeSet::new();
    for tip in crate::history::tips(g).await {
        let list = g
            .git
            .run(&["rev-list", "--topo-order", &tip, "--", rel])
            .await
            .map_err(|e| e.to_string())?;
        commits.extend(
            list.lines()
                .filter(|c| seen.insert((*c).to_owned()))
                .map(str::to_owned),
        );
    }
    Ok(commits)
}

pub(super) async fn is_folder(g: &Group, rel: &str) -> bool {
    if g.worktree
        .as_ref()
        .is_some_and(|root| root.join(rel).is_dir())
    {
        return true;
    }
    // A saved file's exact path takes precedence over older descendants.
    for commit in commits(g, rel).await.unwrap_or_default() {
        let Ok(keys) = g.git.keys(&commit).await else {
            return false;
        };
        if keys.contains_key(rel) {
            return false;
        }
        if keys.keys().any(|p| under(p, rel)) {
            return true;
        }
    }
    false
}

async fn snapshot(
    g: &Group,
    rel: &str,
    before: Option<i64>,
) -> Result<Option<Snapshot<String, (String, String)>>, String> {
    for commit in commits(g, rel).await? {
        let at = g
            .git
            .commit_time(&commit)
            .await
            .ok_or("unreadable snapshot time")?;
        if before.is_some_and(|time| at > time) {
            continue;
        }
        let keys = g.git.keys(&commit).await.map_err(|e| e.to_string())?;
        let tree = g.git.tree(&commit).await.map_err(|e| e.to_string())?;
        let files = keys
            .into_iter()
            .map(|(path, key)| {
                tree.get(&path)
                    .map(|entry| (path, (key, entry.mode.clone())))
                    .ok_or_else(|| "unreadable saved mode".to_owned())
            })
            .collect::<Result<_, _>>()?;
        let candidate = Snapshot { commit, at, files };
        if pigeon_core::restore::select(std::slice::from_ref(&candidate), rel, before).is_some() {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

pub(super) async fn restore(
    d: &Daemon,
    g: &Group,
    rel: &str,
    version: Option<u64>,
    at: Option<&str>,
) -> Vec<Response> {
    if version.is_some() {
        return vec![problem("restore-folder-version", &[], None)];
    }
    let before = match at {
        None => None,
        Some(at) => match instant(at) {
            Some(time) => Some(time),
            None => {
                return vec![problem(
                    "restore-at",
                    &[("path", text(rel)), ("at", text(at))],
                    None,
                )];
            }
        },
    };
    let selected = match snapshot(g, rel, before).await {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            if let Some(at) = at {
                return vec![problem(
                    "restore-at",
                    &[("path", text(rel)), ("at", text(at))],
                    None,
                )];
            }
            return vec![problem(
                "no-such-version",
                &[("path", text(rel)), ("version", Arg::Number(1))],
                None,
            )];
        }
        Err(error) => return vec![problem("internal-error", &[], Some(error))],
    };
    let files = pigeon_core::restore::files(&selected, rel);
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return vec![problem("internal-error", &[], None)];
    };
    let current = match g.git.keys(&main).await {
        Ok(current) => current,
        Err(error) => return vec![problem("internal-error", &[], Some(error.to_string()))],
    };
    let mut needed = crate::settings::reserve_bytes(g.role, &g.git_dir);
    let mut downloads = BTreeSet::new();
    for (path, (key, _)) in &files {
        if !super::path::parents_clear(g, path) {
            return vec![problem("restore-in-use", &[("path", text(path))], None)];
        }
        let Some(size) = key_size(key) else {
            return vec![problem("internal-error", &[], None)];
        };
        needed = needed.saturating_add(size);
        if downloads.insert(key) && crate::custody::present(g, key).await.is_none() {
            needed = needed.saturating_add(size);
        }
    }
    if !pigeon_sys::fs::space(&g.git_dir).is_ok_and(|space| space.available >= needed) {
        return vec![problem(
            "disk-full",
            &[("machine", text(&crate::status::this_name(g, &d.device)))],
            None,
        )];
    }
    for key in downloads {
        if let Err(code) = super::bring(g, key).await {
            return vec![problem(code, &[], None)];
        }
    }
    let mut restored = 0i64;
    let mut errors = Vec::new();
    for (path, (key, mode)) in &files {
        if !super::path::parents_clear(g, path) {
            errors.push(problem("restore-in-use", &[("path", text(path))], None));
            continue;
        }
        match crate::install::put_restore(
            g,
            path,
            key,
            current.get(path).map(String::as_str),
            mode,
            &d.tracer,
        )
        .await
        {
            Ok(Put::Done(_)) => restored += 1,
            Ok(Put::Raced | Put::InUse) => {
                errors.push(problem("restore-in-use", &[("path", text(path))], None))
            }
            Err(error) => errors.push(problem("internal-error", &[], Some(error))),
        }
    }
    if let Err(error) = crate::capture::capture(g, &d.device, &d.tracer).await {
        errors.push(problem("internal-error", &[], Some(error.to_string())));
    }
    d.wake.notify_one();
    // The protocol has progress followed by exactly one final response.
    // A success before errors would make the CLI stop before the errors.
    if !errors.is_empty() {
        let paths: Vec<_> = errors
            .iter()
            .filter_map(|error| match error {
                Response::Problem(p) => match p.message.args.get("path") {
                    Some(Arg::Text(path)) => Some(path.as_str()),
                    _ => None,
                },
                _ => None,
            })
            .take(20)
            .collect();
        return vec![problem(
            "restore-folder-incomplete",
            &[
                ("path", text(rel)),
                ("files", Arg::Number(restored)),
                (
                    "skipped",
                    Arg::Number((files.len() as i64).saturating_sub(restored)),
                ),
                ("paths", text(&paths.join(", "))),
            ],
            serde_json::to_string(&errors).ok(),
        )];
    }
    vec![crate::api::said(
        "restore-folder-result",
        &[("path", text(rel)), ("files", Arg::Number(restored))],
    )]
}
