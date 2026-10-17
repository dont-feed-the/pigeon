//! Proposals (04 §4.5, ADR 0009): signed commits the authority rule does
//! not accept, kept as `refs/pigeon/proposals/<signer-device-id>/<commit>`
//! and relayed by every device.
//!
//! Capture writes one for each set of deciders of a batch's changes this
//! person may not make (07 §7.3 step 7). Accept fetches a peer's into
//! `refs/pigeon/peers/<peer>/proposals/` and keeps one only if its name is
//! its commit, its signature is the named device's, and, judged as one
//! commit on the first accepted commit it builds on, it is refused for
//! folder rules alone (`commit-not-allowed`): a forged or malformed commit
//! is nobody's suggestion. Deciding them is the next step of M2.

use std::collections::BTreeSet;

use pigeon_core::authority::{Refusal, Tree, Verdict};

use crate::git::{Error, Git};
use crate::group::Group;

/// Where this device keeps proposals.
pub const PREFIX: &str = "refs/pigeon/proposals/";

/// The refspec that fetches a peer's proposals beside its `main`.
pub fn refspec(peer: &str) -> String {
    format!("+{PREFIX}*:refs/pigeon/peers/{peer}/proposals/*")
}

/// The ref of `device`'s proposal `commit`.
pub fn name(device: &str, commit: &str) -> String {
    format!("{PREFIX}{device}/{commit}")
}

/// The `(device, commit)` of each ref under `prefix` named
/// `<device>/<commit>`, with the commit it points to.
async fn listed(git: &Git, prefix: &str) -> Vec<(String, String, String)> {
    let format = "--format=%(refname) %(objectname)";
    let Ok(out) = git.run(&["for-each-ref", format, prefix]).await else {
        return Vec::new();
    };
    out.lines()
        .filter_map(|l| {
            let (name, target) = l.split_once(' ')?;
            let (device, commit) = name.strip_prefix(prefix)?.split_once('/')?;
            Some((device.to_owned(), commit.to_owned(), target.to_owned()))
        })
        .collect()
}

/// The proposals this device keeps, as `(device, commit)`.
pub async fn kept(git: &Git) -> Vec<(String, String)> {
    listed(git, PREFIX)
        .await
        .into_iter()
        .filter(|(_, c, t)| c == t)
        .map(|(d, c, _)| (d, c))
        .collect()
}

/// Writes the proposal of `tree` on `parent`, signed by this device `me`,
/// and keeps it.
pub async fn write(g: &Group, me: &str, parent: &str, tree: &Tree) -> Result<String, Error> {
    let oid = crate::merge::write_tree(&g.git, tree).await?;
    let commit = g
        .git
        .commit_tree(&oid, &[parent], "pigeon: suggest")
        .await?;
    g.git.update_ref(&name(me, &commit), &commit, None).await?;
    tracing::info!(group = %g.id, commit = %commit, paths = tree.len(), "suggested");
    Ok(commit)
}

/// The first accepted commit `commit` builds on, by first parents.
pub async fn base(g: &Group, commit: &str) -> Result<Option<String>, Error> {
    let list = g.git.run(&["rev-list", "--first-parent", commit]).await?;
    let k = g.lock();
    Ok(list
        .lines()
        .find(|c| k.engine.accepted(c))
        .map(str::to_owned))
}

/// Whether `commit`, named as `device`'s, is a proposal to keep.
pub(crate) async fn valid(g: &Group, device: &str, commit: &str) -> Result<bool, Error> {
    let raw = g.git.object(commit).await?;
    if !pigeon_core::sig::verify(&raw).is_ok_and(|d| d.0 == device) {
        return Ok(false);
    }
    crate::merge::judge_history(g, &[commit.to_owned()]).await?;
    if g.lock().engine.accepted(commit) {
        return Ok(false);
    }
    let Some(base) = base(g, commit).await? else {
        return Ok(false);
    };
    let after = g.git.tree(commit).await?;
    let before = g
        .lock()
        .engine
        .base(std::slice::from_ref(&base))
        .map(|v| v.tree.clone());
    if before.is_none_or(|before| !crate::purge_admission::proposal_metadata(&before, &after)) {
        return Ok(false);
    }
    let verdict = crate::merge::judge_proposal(g, &raw, &base, &after).await?;
    Ok(matches!(verdict, Verdict::Refused(Refusal::NotAllowed(_))))
}

/// Keeps the valid proposals fetched from `peer` that this device lacks:
/// how many.
pub async fn take(g: &Group, peer: &str) -> usize {
    let mine: BTreeSet<(String, String)> = kept(&g.git).await.into_iter().collect();
    let prefix = format!("refs/pigeon/peers/{peer}/proposals/");
    let mut taken = 0;
    for (device, commit, target) in listed(&g.git, &prefix).await {
        if commit != target || mine.contains(&(device.clone(), commit.clone())) {
            continue;
        }
        match valid(g, &device, &commit).await {
            Ok(true) => {
                if let Err(e) = g
                    .git
                    .update_ref(&name(&device, &commit), &commit, None)
                    .await
                {
                    tracing::warn!(group = %g.id, error = %e, "proposal-ref-failed");
                    continue;
                }
                tracing::info!(group = %g.id, peer = %peer, device = %device,
                    commit = %commit, "proposal-kept");
                taken += 1;
            }
            Ok(false) => {
                tracing::debug!(group = %g.id, peer = %peer, commit = %commit, "proposal-invalid");
            }
            Err(e) => tracing::warn!(group = %g.id, error = %e, "proposal-judge-failed"),
        }
    }
    taken
}

/// The keys of every kept proposal's tree: content to protect like any
/// version (04 §4.5).
pub async fn keys(g: &Group) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, commit) in kept(&g.git).await {
        if let Ok(k) = g.git.keys(&commit).await {
            out.extend(k.into_values());
        }
    }
    out
}
