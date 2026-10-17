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

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use pigeon_core::filter::Role;
use pigeon_core::queue::Work;
use pigeon_core::roster::Role as DeviceRole;

use crate::git::Git;
use crate::group::Group;
use crate::settings::ensure_remote;
use crate::trace::Tracer;

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
    let archive = crate::refound::refspec(id);
    let proofs = crate::purge_epoch::refspec(id);
    let args = [
        "fetch",
        "-q",
        "--no-tags",
        "--no-write-fetch-head",
        "--refmap=",
        id,
        spec.as_str(),
        reports.as_str(),
        proposals.as_str(),
        archive.as_str(),
        proofs.as_str(),
    ];
    match g.git.run_for(&args, FETCH_TIMEOUT).await {
        Err(e @ crate::git::Error::Timeout { .. }) => {
            tracing::info!(group = %g.id, peer = %id, error = %e, "fetch-retried");
            g.git.run_for(&args, FETCH_TIMEOUT).await.map(drop)
        }
        other => other.map(drop),
    }
}

/// How long [`any_reachable`] waits for one peer: someone is waiting.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// Whether at least one of `ids` answers a fetch now, tried at once and
/// once each; one that does is heard. The remotes are configured under
/// the repository lock, the fetches run without it.
pub async fn any_reachable(g: &Group, me: &str, ids: &[String]) -> bool {
    {
        let _busy = g.busy.lock().await;
        for (id, uuid, _) in peers(g, me).iter().filter(|(id, _, _)| ids.contains(id)) {
            if let Err(e) = ensure_remote(&g.git, id, uuid).await {
                tracing::warn!(group = %g.id, peer = %id, error = %e, "remote-failed");
            }
        }
    }
    let probes = ids.iter().map(|id| async move {
        let spec = crate::settings::peer_refspec(id);
        let args = [
            "fetch",
            "-q",
            "--no-tags",
            "--no-write-fetch-head",
            id,
            &spec,
        ];
        let ok = g.git.run_for(&args, PROBE_TIMEOUT).await.is_ok();
        if ok {
            g.lock().heard.insert(id.clone(), crate::util::now());
        }
        ok
    });
    crate::util::join_all(probes.collect())
        .await
        .into_iter()
        .any(|ok| ok)
}

/// Configures fetch remotes under the repository lock, before network waits.
pub async fn prepare_accept(g: &Group, me: &str) -> Vec<String> {
    let peers = peers(g, me);
    for (id, uuid, _) in &peers {
        if let Err(e) = ensure_remote(&g.git, id, uuid).await {
            tracing::warn!(group = %g.id, peer = %id, error = %e, "remote-failed");
        }
    }
    g.reload_remotedaemon(peers.iter().map(|(id, _, _)| id.clone()).collect())
        .await;
    peers.into_iter().map(|(id, _, _)| id).collect()
}

/// Fetches only objects and peer refs, without holding the file-operation lock.
/// It never moves `main`, edits the index, or changes the working tree.
pub async fn fetch_ready(g: &Group, peers: &[String]) -> Vec<String> {
    fetch_each(g, peers, |_| std::future::ready(())).await
}

/// Offers completed peers without waiting for the others. Already-ready
/// results are grouped without a delay to avoid repeated annex merges. The caller serializes
/// adoption with other repository operations. Fetches continue being polled
/// while adoption runs, and every fetch still finishes or times out.
pub async fn fetch_each<F, R>(g: &Group, peers: &[String], mut ready: F) -> Vec<String>
where
    F: FnMut(Vec<String>) -> R,
    R: std::future::Future<Output = ()>,
{
    let mut fetched = Vec::new();
    let mut unreachable = BTreeSet::new();
    // Every peer at once: one whose fetch hangs until its timeout must not
    // hold back what the others bring (16 §16.4).
    let fetches = peers
        .iter()
        .map(|id| async move { (id.clone(), fetch(g, id).await) });
    let mut history_queue = crate::telemetry::Queue::start(&g.id, "history", "fetch", peers.len());
    let mut fetches = crate::util::Pending::new(fetches.collect());
    // There is at most one result per peer. Buffering those results keeps
    // network I/O and its existing deadlines running during history checks.
    let (send, mut receive) = tokio::sync::mpsc::channel(peers.len().max(1));
    let network = async move {
        while let Some(result) = fetches.next().await {
            if send.send(result).await.is_err() {
                break;
            }
        }
        drop(send);
    };
    let histories = async {
        while let Some(first) = receive.recv().await {
            let mut completed = vec![first];
            while let Ok(result) = receive.try_recv() {
                completed.push(result);
            }
            let mut available = Vec::new();
            for (id, result) in completed {
                history_queue.complete(1);
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
                        fetched.push(id.clone());
                        available.push(id);
                    }
                    Err(e) => {
                        tracing::debug!(group = %g.id, peer = %id, error = %e, "fetch-failed");
                        unreachable.insert(id);
                    }
                }
            }
            if !available.is_empty() {
                ready(available).await;
            }
        }
        g.lock().unreachable = unreachable;
        fetched
    };
    tokio::join!(network, histories).1
}

/// Validates fetched history and advances `main` under the repository lock.
/// A peer whose fetch failed is unreachable for this round: place skips it.
pub async fn accept(g: &Group, fetched: Vec<String>, tracer: &Tracer) -> bool {
    if fetched.is_empty() {
        return false;
    }
    if let Err(e) = g.git.run(&["annex", "merge"]).await {
        tracing::warn!(group = %g.id, error = %e, "annex-merge-failed");
    }
    crate::refound::adopt(g).await;
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
                g.reload_roster().await;
                crate::trace_roster::moved(tracer, g, "Merge", ours.as_deref()).await;
                tracing::info!(group = %g.id, peer = %id, commit = %target, "accepted");
                crate::telemetry::accepted(g, ours.as_deref(), &target).await;
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
    holders_for(git, &BTreeSet::from([key.to_owned()]))
        .await
        .remove(key)
        .unwrap_or_default()
}

/// Read location-log claims in bounded native batches. Starting one annex
/// and external helper per key makes a small-file round needlessly slow.
/// These remain claims, not positive storage proofs or deletion authority.
pub async fn holders_for(git: &Git, keys: &BTreeSet<String>) -> BTreeMap<String, BTreeSet<String>> {
    let keys: Vec<_> = keys
        .iter()
        .filter(|key| !key.is_empty() && !key.contains(['\n', '\r', '\0']))
        .collect();
    let mut all = BTreeMap::new();
    for batch in keys.chunks(64) {
        let input = batch
            .iter()
            .map(|key| format!("{key}\n"))
            .collect::<String>();
        let output = git
            .repo
            .run_status(
                &["annex", "whereis", "--batch-keys", "--json"],
                Some(input.as_bytes()),
                pigeon_git::DEFAULT_TIMEOUT,
            )
            .await;
        if let Ok((Some(0 | 1), output)) = output
            && let Some(locations) = read_holders_batch(&output.stdout, batch)
        {
            all.extend(locations);
        }
    }
    all
}

fn read_holders_batch(
    bytes: &[u8],
    expected: &[&String],
) -> Option<BTreeMap<String, BTreeSet<String>>> {
    let mut all = BTreeMap::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let answer: pigeon_git::annex::Whereis = serde_json::from_slice(line).ok()?;
        let key = answer.answer.key?;
        if !expected.iter().any(|expected| **expected == key) || all.contains_key(&key) {
            return None;
        }
        all.insert(
            key,
            answer
                .whereis
                .into_iter()
                .chain(answer.untrusted)
                .filter(|location| !location.uuid.is_empty())
                .map(|location| location.uuid)
                .collect(),
        );
    }
    (all.len() == expected.len()).then_some(all)
}

/// Where `key`'s content is in this repository, if it is present.
pub async fn present(g: &Group, key: &str) -> Option<std::path::PathBuf> {
    present_one(&g.git, key).await
}

async fn present_one(git: &Git, key: &str) -> Option<std::path::PathBuf> {
    let out = git.run(&["annex", "contentlocation", key]).await.ok()?;
    let rel = out.trim();
    (!rel.is_empty()).then(|| git.repo.dir.join(rel))
}

/// Local content observations in bounded native batches. A blank answer
/// means absent; missing/extra lines fail closed. Special encoded key names
/// use the scalar query rather than guessing their filesystem spelling.
pub async fn present_for(
    git: &Git,
    keys: &BTreeSet<String>,
) -> BTreeMap<String, std::path::PathBuf> {
    let keys: Vec<_> = keys
        .iter()
        .filter(|key| !key.is_empty() && !key.contains(['\n', '\r', '\0']))
        .collect();
    let mut all = BTreeMap::new();
    for batch in keys.chunks(64) {
        let input = batch
            .iter()
            .map(|key| format!("{key}\n"))
            .collect::<String>();
        let Ok((Some(0 | 1), output)) = git
            .repo
            .run_status(
                &["annex", "contentlocation", "--batch"],
                Some(input.as_bytes()),
                pigeon_git::DEFAULT_TIMEOUT,
            )
            .await
        else {
            continue;
        };
        let Some(lines) = output.stdout.strip_suffix(b"\n") else {
            continue;
        };
        let lines: Vec<_> = lines.split(|b| *b == b'\n').collect();
        if lines.len() != batch.len() {
            continue;
        }
        for (key, line) in batch.iter().zip(lines) {
            if line.is_empty() {
                continue;
            }
            let Ok(path) = std::str::from_utf8(line) else {
                continue;
            };
            let path = std::path::Path::new(path);
            let object = if path.file_name().is_some_and(|name| name == key.as_str())
                && path
                    .parent()
                    .and_then(std::path::Path::file_name)
                    .is_some_and(|name| name == key.as_str())
            {
                Some(git.repo.dir.join(path))
            } else {
                present_one(git, key).await
            };
            if let Some(object) = object.filter(|p| p.is_file()) {
                all.insert((*key).clone(), object);
            }
        }
    }
    all
}

/// One round of place, in queue order (15 §15.2): answers how many copies
/// it made. A custodian gets what admission lets it take (08 §8.9); a
/// laptop sends each key to the custodians lacking it, whose own reserve
/// refuses what does not fit (E22).
pub async fn place(g: &Group, me: &str) -> usize {
    place_inner(g, me, None).await
}

/// Run queued placement without a caller-held worktree lock. Retaining
/// references are read under that lock; cold probes run outside it, and
/// current identity, membership and retention are rechecked before each key.
pub async fn place_worker(
    g: &Group,
    me: &str,
    d: &std::sync::Weak<crate::daemon::Daemon>,
) -> usize {
    place_inner(g, me, Some(d)).await
}

async fn place_inner(
    g: &Group,
    me: &str,
    d: Option<&std::sync::Weak<crate::daemon::Daemon>>,
) -> usize {
    let planning = if d.is_some() {
        Some(g.busy.lock().await)
    } else {
        None
    };
    if crate::membership::pending(g, me) || g.lock().admission_error.is_some() {
        return 0;
    }
    let keys = match crate::retention::keys(g).await {
        Ok(keys) => keys,
        Err(e) => {
            tracing::warn!(group = %g.id, error = %e, "retained-history-unreadable");
            return 0;
        }
    };
    // Only the retained reference snapshot needs the worktree lock. Cold
    // location/capacity probes can take a whole round per hundred keys;
    // every key is revalidated again before its actual transfer.
    drop(planning);
    if keys.is_empty() {
        return 0;
    }
    let mut made = 0;
    let mut left = 0;
    let mut validation = crate::placement::Validation::default();
    let mut transfers = crate::custody_transfer::Pool::default();
    match g.role {
        Role::Laptop => {
            let mut failed = g.lock().unreachable.clone();
            let items = crate::admission::items(g, me, &keys).await;
            let mut queue = crate::telemetry::Queue::start(&g.id, "protect", "send", items.len());
            for (work, lacking) in items {
                let Work::Protect { key, size, .. } = work else {
                    continue;
                };
                if let Some(d) = d
                    && !validation.allows(d, g, &key).await
                {
                    break;
                }
                if g.shared.is_some() && crate::restore::bring(g, &key).await.is_err() {
                    left += size;
                    continue;
                }
                for id in lacking {
                    if failed.contains(&id) {
                        left += size;
                        continue;
                    }
                    match transfers.copy(&g.git, &id, &key, TRANSFER_TIMEOUT).await {
                        Ok(_) => made += 1,
                        Err(e) => {
                            tracing::debug!(group = %g.id, peer = %id, error = %e, "copy-failed");
                            failed.insert(id);
                            left += size;
                        }
                    }
                }
                queue.complete(1);
            }
        }
        Role::Custodian => {
            let plan = crate::admission::plan(g, me, &keys).await;
            let mut queue =
                crate::telemetry::Queue::start(&g.id, "protect", "receive", plan.admitted.len());
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
                if let Some(d) = d
                    && !validation.allows(d, g, &key).await
                {
                    break;
                }
                if crate::shared::provider_hydrate(g, &key)
                    .await
                    .is_ok_and(|here| here)
                    && let Some(object) = present(g, &key).await
                    && crate::net::sync::sync_object(&object).is_ok()
                {
                    made += 1;
                    continue;
                }
                match transfers.get(&g.git, &key, TRANSFER_TIMEOUT).await {
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
                queue.complete(1);
            }
        }
    }
    transfers.stop().await;
    g.lock().queue_bytes = left;
    if made > 0 {
        tracing::info!(group = %g.id, copies = made, "placed");
    }
    made
}

#[cfg(test)]
mod location_tests {
    #[test]
    fn native_batch_claims_are_bound_to_all_requested_keys_without_duplicates() {
        let a = String::from("a");
        let b = String::from("b");
        let valid = b"{\"key\":\"a\",\"whereis\":[{\"uuid\":\"one\"}]}\n{\"key\":\"b\",\"untrusted\":[{\"uuid\":\"two\"}]}\n";
        let claims = super::read_holders_batch(valid, &[&a, &b]);
        assert!(
            claims
                .as_ref()
                .is_some_and(|m| m.len() == 2 && m[&a].contains("one") && m[&b].contains("two"))
        );
        for invalid in [
            b"{\"key\":\"a\"}\n".as_slice(),
            b"{\"key\":\"a\"}\n{\"key\":\"a\"}\n",
            b"{\"key\":\"a\"}\n{\"key\":\"other\"}\n",
            b"{\"key\":\"a\"}\nnot JSON\n",
            b"{\"key\":\"a\"}\n{\"whereis\":[{\"uuid\":\"two\"}]}\n",
        ] {
            assert!(super::read_holders_batch(invalid, &[&a, &b]).is_none());
        }
    }
}
