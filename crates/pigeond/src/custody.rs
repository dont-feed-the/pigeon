//! Accept and place (08 §8.2, §8.3): fetch each peer's `main` and git-annex
//! branch, fast-forward to a descendant of `main`, and move content to the
//! custodians.
//!
//! In the slice a laptop sends every version to every custodian with
//! `copy --to --key`, and a custodian gets every key of `main` it lacks
//! with `get --key`, then syncs the object to disk; with K = 2 and two
//! custodians that is `sizebalanced`'s choice too (13 §13.12). Accept only
//! fast-forwards: with one laptop, nothing else commits, and a divergence
//! is logged and left (slice only).

use std::collections::BTreeSet;
use std::time::Duration;

use pigeon_core::filter::Role;
use pigeon_core::roster::Role as DeviceRole;

use crate::git::Git;
use crate::group::Group;
use crate::settings::ensure_remote;
use crate::trace::{Act, Tracer};

/// How long one fetch may take: a P2P fetch can hang (E12).
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);
/// How long one transfer may take.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(600);

/// The peers of the roster this device fetches from and sends to: every
/// laptop and custodian but itself, with its annex UUID.
pub fn peers(g: &Group, me: &str) -> Vec<(String, String, DeviceRole)> {
    let Some(doc) = g.roster() else {
        return Vec::new();
    };
    doc.roster
        .devices
        .iter()
        .filter(|(id, d)| id.0 != me && d.role != DeviceRole::Recovery)
        .filter_map(|(id, d)| Some((id.0.clone(), d.annex_uuid.clone()?, d.role)))
        .collect()
}

/// One round of accept: answers whether `main` moved.
pub async fn accept(g: &Group, me: &str, tracer: &Tracer) -> bool {
    let mut fetched = Vec::new();
    for (id, uuid, _) in peers(g, me) {
        if let Err(e) = ensure_remote(&g.git, &id, &uuid).await {
            tracing::warn!(group = %g.id, peer = %id, error = %e, "remote-failed");
            continue;
        }
        let spec = format!("+refs/heads/*:refs/pigeon/peers/{id}/*");
        let args = [
            "fetch",
            "-q",
            "--no-tags",
            "--no-write-fetch-head",
            id.as_str(),
            spec.as_str(),
        ];
        match g.git.run_for(&args, FETCH_TIMEOUT).await {
            Ok(_) => {
                g.lock().heard.insert(id.clone(), crate::util::now());
                if let Some(ga) = g
                    .git
                    .rev(&format!("refs/pigeon/peers/{id}/git-annex"))
                    .await
                {
                    let name = format!("refs/remotes/{id}/git-annex");
                    let _ = g.git.run(&["update-ref", &name, &ga]).await;
                }
                fetched.push(id);
            }
            Err(e) => tracing::debug!(group = %g.id, peer = %id, error = %e, "fetch-failed"),
        }
    }
    if fetched.is_empty() {
        return false;
    }
    if let Err(e) = g.git.run(&["annex", "merge"]).await {
        tracing::warn!(group = %g.id, error = %e, "annex-merge-failed");
    }
    let mut moved = false;
    for id in fetched {
        let Some(theirs) = g.git.rev(&format!("refs/pigeon/peers/{id}/main")).await else {
            continue;
        };
        if g.git.root(&theirs).await.as_deref() != Some(g.id.as_str()) {
            tracing::warn!(group = %g.id, peer = %id, "accept-foreign-history");
            continue;
        }
        let ours = g.git.rev("refs/heads/main").await;
        let forward = match &ours {
            None => true,
            Some(o) if *o == theirs => false,
            Some(o) => g.git.contains(&theirs, o).await,
        };
        if !forward {
            if let Some(o) = &ours
                && !g.git.contains(o, &theirs).await
            {
                tracing::warn!(group = %g.id, peer = %id, "accept-diverged");
            }
            continue;
        }
        if g.role == Role::Laptop {
            tracing::info!(group = %g.id, peer = %id, "accept-install-not-in-this-build");
            continue;
        }
        match g
            .git
            .update_ref("refs/heads/main", &theirs, ours.as_deref())
            .await
        {
            Ok(()) => {
                tracer.event(Act::Merge, &[("commit", &theirs)]);
                tracing::info!(group = %g.id, peer = %id, commit = %theirs, "accepted");
                moved = true;
            }
            Err(e) => tracing::warn!(group = %g.id, error = %e, "accept-ref-failed"),
        }
    }
    g.reload_roster().await;
    moved
}

/// The annex UUIDs git-annex knows to hold `key`.
pub async fn holders(git: &Git, key: &str) -> BTreeSet<String> {
    let out = git
        .repo
        .run_status(
            &["annex", "whereis", "--json", "--key", key],
            None,
            pigeon_git::DEFAULT_TIMEOUT,
        )
        .await;
    let Ok((_, out)) = out else {
        return BTreeSet::new();
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return BTreeSet::new();
    };
    ["whereis", "untrusted"]
        .iter()
        .filter_map(|list| v.get(list)?.as_array())
        .flatten()
        .filter_map(|l| Some(l.get("uuid")?.as_str()?.to_owned()))
        .collect()
}

/// Where `key`'s content is in this repository, if it is present.
pub async fn present(g: &Group, key: &str) -> Option<std::path::PathBuf> {
    let out = g.git.run(&["annex", "contentlocation", key]).await.ok()?;
    let rel = out.trim();
    (!rel.is_empty()).then(|| g.git.repo.dir.join(rel))
}

/// One round of place: answers how many copies it made.
pub async fn place(g: &Group, me: &str) -> usize {
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return 0;
    };
    let Ok(keys) = g.git.keys(&main).await else {
        return 0;
    };
    let keys: BTreeSet<String> = keys.into_values().collect();
    let mut made = 0;
    match g.role {
        Role::Laptop => {
            let custodians: Vec<(String, String)> = peers(g, me)
                .into_iter()
                .filter(|(_, _, r)| *r == DeviceRole::Custodian)
                .map(|(id, u, _)| (id, u))
                .collect();
            let mut failed = BTreeSet::new();
            for key in &keys {
                let have = holders(&g.git, key).await;
                for (id, uuid) in &custodians {
                    if have.contains(uuid) || failed.contains(id) {
                        continue;
                    }
                    let args = [
                        "annex",
                        "copy",
                        "--json",
                        "--to",
                        id.as_str(),
                        "--key",
                        key.as_str(),
                    ];
                    match g.git.run_for(&args, TRANSFER_TIMEOUT).await {
                        Ok(_) => made += 1,
                        Err(e) => {
                            tracing::debug!(group = %g.id, peer = %id, error = %e, "copy-failed");
                            failed.insert(id.clone());
                        }
                    }
                }
            }
        }
        Role::Custodian => {
            for key in &keys {
                if present(g, key).await.is_some() {
                    continue;
                }
                let args = ["annex", "get", "--json", "--key", key.as_str()];
                match g.git.run_for(&args, TRANSFER_TIMEOUT).await {
                    Ok(_) => {
                        if let Some(object) = present(g, key).await {
                            let _ = crate::net::sync::sync_object(&object);
                        }
                        made += 1;
                    }
                    Err(e) => tracing::debug!(group = %g.id, key = %key, error = %e, "get-failed"),
                }
            }
        }
    }
    if made > 0 {
        tracing::info!(group = %g.id, copies = made, "placed");
    }
    made
}
