//! `pigeon sync` (08 §8.5): every folder of the group and whether this
//! computer keeps it, with what decides it, and forgetting this computer's
//! choices of a folder so that it follows its default again. The loops of
//! `evict` free and keep; this module only reads and records choices.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pigeon_proto::Response;
use pigeon_proto::api::{Arg, SyncFolder};

use crate::api::{group_of, problem, relative, said, wake_soon};
use crate::daemon::Daemon;
use crate::evict::under;

/// `pigeon sync [PATH]`: the folders of the group, under `folder` if given,
/// parents before their children.
pub async fn list(d: &Daemon, folder: Option<&Path>) -> Vec<Response> {
    let found = match folder {
        Some(path) => group_of(d, path).and_then(|g| relative(&g, path).map(|rel| (g, rel))),
        None => d.default_group().map(|g| (g, String::new())),
    };
    let Some((g, scope)) = found else {
        return vec![problem("not-in-group", &[], None)];
    };
    let root = g
        .worktree
        .as_ref()
        .unwrap_or(&g.git_dir)
        .to_string_lossy()
        .into_owned();
    let files = match g.git.rev("refs/heads/main").await {
        Some(main) => g.git.keys(&main).await.unwrap_or_default(),
        None => BTreeMap::new(),
    };
    let chosen = crate::keep_targets::read(&g);
    let targets = crate::evict::targets(&g);
    let others = others(&g);
    let mut folders: BTreeSet<String> = files
        .keys()
        .flat_map(|path| {
            let parts: Vec<&str> = path.split('/').collect();
            (1..parts.len())
                .map(|n| parts[..n].join("/"))
                .collect::<Vec<_>>()
        })
        .collect();
    folders.extend(chosen.0.keys().filter(|p| !p.is_empty()).cloned());
    folders.extend(others.keys().cloned());
    let folders = folders
        .into_iter()
        .filter(|path| under(path, &scope))
        .map(|path| SyncFolder {
            kept: !targets.is_freed(&path),
            chosen: chosen.0.contains_key(&path),
            others: others.get(&path).cloned(),
            path,
        })
        .collect();
    vec![Response::Sync { root, folders }]
}

/// Other people's folders of a per-person folder, with their owner's name;
/// none on a backup machine, which keeps everything.
fn others(g: &crate::group::Group) -> BTreeMap<String, String> {
    if g.role != pigeon_core::filter::Role::Laptop {
        return BTreeMap::new();
    }
    let Some(doc) = g.roster() else {
        return BTreeMap::new();
    };
    let me = crate::git::signer_device().and_then(|device| {
        doc.roster
            .devices
            .get(&pigeon_core::roster::DeviceId(device))
            .map(|d| d.person.clone())
    });
    let Some(me) = me else {
        return BTreeMap::new();
    };
    pigeon_core::keep_targets::others_folders(&doc.roster, &me)
        .into_iter()
        .filter_map(|folder| {
            let id = folder.rsplit('/').next()?;
            let person = doc
                .roster
                .persons
                .get(&pigeon_core::roster::PersonId(id.to_owned()))?;
            Some((folder, person.name.clone()))
        })
        .collect()
}

/// `pigeon sync PATH --default`: forgets this computer's choices of the
/// folder and inside it; the loops then free or keep it as its default says.
pub async fn forget(d: &Daemon, path: &Path) -> Vec<Response> {
    let Some(g) = group_of(d, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(prefix) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    if let Err(error) = crate::evict::forget_target(&g, &prefix) {
        return vec![problem("internal-error", &[], Some(error))];
    }
    wake_soon(d);
    vec![said(
        "sync-default-result",
        &[("folder", Arg::Text(prefix))],
    )]
}
