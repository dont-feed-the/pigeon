//! Accept and place (08 §8.2, §8.3): fetch each peer's `main`, git-annex
//! branch and proposals (`proposal`), fast-forward to a descendant of
//! `main`, and move content to the custodians.
//!
//! In the slice a laptop sends every version to every custodian with
//! `copy --to --key`, and a custodian gets each key of `main` and of the
//! kept proposals it lacks and
//! admits (`admission`) with `get --key`, then syncs the object to disk,
//! both in queue order; with K = 2 and two custodians that is
//! `sizebalanced`'s choice too (13 §13.12). Every device moves `main` only
//! as far as the authority rule accepts (`verify`, 04 §4.4), and merges a
//! peer's accepted `main` that diverged from its own (05 §5.2); on a
//! laptop, install then writes the new versions into the folder
//! (`install`, 07 §7.5).

use std::collections::BTreeSet;
use std::time::Duration;

use pigeon_core::filter::Role;
use pigeon_core::queue::Work;
use pigeon_core::roster::Role as DeviceRole;

use crate::git::Git;
use crate::group::Group;
use crate::settings::ensure_remote;
use crate::trace::{Act, Tracer};

/// How long one fetch may take: a P2P fetch can hang (E12, 16 §16.4), and
/// one that does is tried once more at once.
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
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

/// Fetches `id`'s branches and the status reports it has (06 §6.3); a
/// fetch that hangs is killed and tried once more.
async fn fetch(g: &Group, id: &str) -> Result<(), crate::git::Error> {
    let spec = crate::settings::peer_refspec(id);
    let reports = crate::report::refspec(id);
    let proposals = crate::proposal::refspec(id);
    let args = [
        "fetch",
        "-q",
        "--no-tags",
        "--no-write-fetch-head",
        id,
        spec.as_str(),
        reports.as_str(),
        proposals.as_str(),
    ];
    match g.git.run_for(&args, FETCH_TIMEOUT).await {
        Err(e @ crate::git::Error::Timeout { .. }) => {
            tracing::info!(group = %g.id, peer = %id, error = %e, "fetch-retried");
            g.git.run_for(&args, FETCH_TIMEOUT).await.map(drop)
        }
        other => other.map(drop),
    }
}

/// One round of accept: answers whether `main` moved. A peer whose fetch
/// fails is unreachable for this round: place sends it nothing.
pub async fn accept(g: &Group, me: &str, tracer: &Tracer) -> bool {
    let mut fetched = Vec::new();
    let mut unreachable = BTreeSet::new();
    let peers = peers(g, me);
    for (id, uuid, _) in &peers {
        if let Err(e) = ensure_remote(&g.git, id, uuid).await {
            tracing::warn!(group = %g.id, peer = %id, error = %e, "remote-failed");
        }
    }
    g.reload_remotedaemon(peers.iter().map(|(id, _, _)| id.clone()).collect())
        .await;
    // Every peer at once: one whose fetch hangs until its timeout must not
    // hold back what the others bring (16 §16.4).
    let fetches = peers
        .iter()
        .map(|(id, _, _)| async move { (id.clone(), fetch(g, id).await) });
    for (id, result) in crate::util::join_all(fetches.collect()).await {
        match result {
            Ok(()) => {
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
            Err(e) => {
                tracing::debug!(group = %g.id, peer = %id, error = %e, "fetch-failed");
                unreachable.insert(id);
            }
        }
    }
    g.lock().unreachable = unreachable;
    if fetched.is_empty() {
        return false;
    }
    if let Err(e) = g.git.run(&["annex", "merge"]).await {
        tracing::warn!(group = %g.id, error = %e, "annex-merge-failed");
    }
    let mut moved = false;
    for id in fetched {
        crate::proposal::take(g, &id).await;
        let Some(theirs) = g.git.rev(&format!("refs/pigeon/peers/{id}/main")).await else {
            continue;
        };
        if g.git.root(&theirs).await.as_deref() != Some(g.id.as_str()) {
            tracing::warn!(group = %g.id, peer = %id, "accept-foreign-history");
            continue;
        }
        let ours = g.git.rev("refs/heads/main").await;
        if let Some(o) = &ours
            && g.git.contains(o, &theirs).await
        {
            continue;
        }
        let Some(target) = judged(g, &id, ours.as_deref(), &theirs).await else {
            continue;
        };
        let target = match &ours {
            Some(o) if g.git.contains(o, &target).await => continue,
            Some(o) if !g.git.contains(&target, o).await => {
                match merged(g, &id, o, &target).await {
                    Some(m) => m,
                    None => continue,
                }
            }
            _ => target,
        };
        match g
            .git
            .update_ref("refs/heads/main", &target, ours.as_deref())
            .await
        {
            Ok(()) => {
                tracer.event(Act::Merge, &[("commit", &target)]);
                tracing::info!(group = %g.id, peer = %id, commit = %target, "accepted");
                moved = true;
            }
            Err(e) => tracing::warn!(group = %g.id, error = %e, "accept-ref-failed"),
        }
    }
    g.reload_roster().await;
    moved
}

/// The merge of our `main` with a peer's accepted `target` that diverged
/// from it (05 §5.2), if `target` brings a contribution we lack.
async fn merged(g: &Group, peer: &str, ours: &str, target: &str) -> Option<String> {
    match crate::merge::brings(g, ours, target).await {
        Ok(true) => {}
        Ok(false) => return None,
        Err(e) => {
            tracing::warn!(group = %g.id, peer = %peer, error = %e, "merge-failed");
            return None;
        }
    }
    match crate::merge::merge(g, ours, target).await {
        Ok(Some(m)) => {
            tracing::info!(group = %g.id, peer = %peer, commit = %m, "merged");
            Some(m)
        }
        Ok(None) => {
            tracing::warn!(group = %g.id, peer = %peer, "merge-not-accepted");
            None
        }
        Err(e) => {
            tracing::warn!(group = %g.id, peer = %peer, error = %e, "merge-failed");
            None
        }
    }
}

/// The last commit of `theirs` that the authority rule accepts after
/// `ours` (04 §4.4); the first it refuses is kept, by peer, for `doctor`.
async fn judged(g: &Group, peer: &str, ours: Option<&str>, theirs: &str) -> Option<String> {
    let judged = match crate::verify::judge(g, ours, theirs).await {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(group = %g.id, peer = %peer, error = %e, "accept-judge-failed");
            return None;
        }
    };
    match &judged.stopped {
        Some((commit, verdict)) => {
            let code = crate::verify::code(verdict);
            let entry = (commit.clone(), code);
            let known = g.lock().refused.insert(peer.to_owned(), entry.clone());
            if known.as_ref() != Some(&entry) {
                tracing::warn!(group = %g.id, peer = %peer, commit = %commit, code,
                    verdict = ?verdict, "accept-refused");
            }
        }
        None => {
            g.lock().refused.remove(peer);
        }
    }
    judged.accepted
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

/// One round of place, in queue order (15 §15.2): answers how many copies
/// it made. A custodian gets what admission lets it take (08 §8.9); a
/// laptop sends each key to the custodians lacking it, whose own reserve
/// refuses what does not fit (E22).
pub async fn place(g: &Group, me: &str) -> usize {
    let keys = match crate::retention::keys(g).await {
        Ok(keys) => keys,
        Err(e) => {
            tracing::warn!(group = %g.id, error = %e, "retained-history-unreadable");
            return 0;
        }
    };
    if keys.is_empty() {
        return 0;
    }
    let mut made = 0;
    let mut left = 0;
    match g.role {
        Role::Laptop => {
            let mut failed = g.lock().unreachable.clone();
            for (work, lacking) in crate::admission::items(g, me, &keys).await {
                let Work::Protect { key, size, .. } = work else {
                    continue;
                };
                for id in lacking {
                    if failed.contains(&id) {
                        left += size;
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
                            failed.insert(id);
                            left += size;
                        }
                    }
                }
            }
        }
        Role::Custodian => {
            let plan = crate::admission::plan(g, me, &keys).await;
            left = plan
                .deferred
                .iter()
                .map(|d| match &d.work {
                    Work::Protect { size, .. } => *size,
                    _ => 0,
                })
                .sum();
            for work in plan.admitted {
                let Work::Protect { key, size, .. } = work else {
                    continue;
                };
                let args = ["annex", "get", "--json", "--key", key.as_str()];
                match g.git.run_for(&args, TRANSFER_TIMEOUT).await {
                    Ok(_) => {
                        if let Some(object) = present(g, &key).await {
                            let _ = crate::net::sync::sync_object(&object);
                        }
                        made += 1;
                    }
                    Err(e) => {
                        tracing::debug!(group = %g.id, key = %key, error = %e, "get-failed");
                        left += size;
                    }
                }
            }
        }
    }
    g.lock().queue_bytes = left;
    if made > 0 {
        tracing::info!(group = %g.id, copies = made, "placed");
    }
    made
}
