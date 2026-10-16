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
    g: &std::sync::Arc<Group>,
    rel: &str,
    version: Option<u64>,
    at: Option<&str>,
    busy: tokio::sync::MutexGuard<'_, ()>,
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
    let mut absent = false;
    for (path, (key, _)) in &files {
        if !super::path::parents_clear(g, path) {
            return vec![problem("restore-in-use", &[("path", text(path))], None)];
        }
        let Some(size) = key_size(key) else {
            return vec![problem("internal-error", &[], None)];
        };
        needed = needed.saturating_add(size);
        if downloads.insert(key) && crate::custody::present(g, key).await.is_none() {
            absent = true;
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
    let intent = if absent {
        let Some(worktree) = &g.worktree else {
            return vec![problem("not-in-group", &[], None)];
        };
        // A reference is an explicit request even if fetching later fails.
        // Refuse obvious private/writer obstructions before publishing it.
        for path in files.keys() {
            let file = worktree.join(path);
            let clear = match pigeon_sys::fs::lstat(&file) {
                Ok(None) => true,
                Ok(Some(stat)) => {
                    stat.is_regular()
                        && pigeon_sys::writers::scan(stat.id).is_clear()
                        && crate::evict::is_old(&file, current.get(path).map(String::as_str))
                }
                Err(_) => false,
            };
            if !clear {
                return vec![problem("restore-in-use", &[("path", text(path))], None)];
            }
        }
        if crate::capture_file::pending(g) || crate::evict::unsettled(g) {
            return vec![problem("receiving", &[], None)];
        }
        let tree = match g.git.tree(&selected.commit).await {
            Ok(tree) => tree,
            Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
        };
        let changes = files
            .keys()
            .map(|p| (p.clone(), tree.get(p).cloned()))
            .collect();
        match super::reference::retain(g, &d.device, changes).await {
            Ok(intent) => intent,
            Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
        }
    } else {
        super::reference::Intent::default()
    };
    drop(busy);
    crate::purge::prepare(g).await;
    let _busy = g.busy.lock().await;
    if !d.current_group(g).await {
        return vec![problem("not-in-group", &[], None)];
    }
    crate::purge::round(g, &d.device, false).await;
    for key in downloads {
        if let Err(code) = super::bring(g, key).await {
            if code == "receiving" && absent {
                d.wake.notify_one();
                return vec![problem("restore-waiting", &[("path", text(rel))], None)];
            }
            return vec![problem(code, &[], None)];
        }
    }
    let index = match g.git.index().await {
        Ok(index) => index,
        Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
    };
    let mut current = std::collections::BTreeMap::new();
    for path in files.keys() {
        if let Some(entry) = index.get(path) {
            let bytes = match g.git.object(&entry.oid).await {
                Ok(bytes) => bytes,
                Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
            };
            if let Some(key) = crate::git::pointer_key(&bytes) {
                current.insert(path.clone(), key);
            }
        }
    }
    let mut restored = 0i64;
    let mut installed = BTreeSet::new();
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
            Ok(Put::Done(_)) => {
                restored += 1;
                installed.insert(path.clone());
            }
            Ok(Put::Raced | Put::InUse) => {
                errors.push(problem("restore-in-use", &[("path", text(path))], None))
            }
            Err(error) => errors.push(problem("internal-error", &[], Some(error))),
        }
    }
    if let Err(error) = intent.installed(g, &d.device, &installed).await {
        errors.push(problem("internal-error", &[], Some(error.to_string())));
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
