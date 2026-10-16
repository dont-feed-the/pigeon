//! How a person's suggestions were decided (ADR 0078, 04 §4.5), shown to
//! their author on the Suggestions page and by `pigeon suggestions`, for
//! the decisions of the last [`RECENT`] seconds on `main`:
//!
//! - an adoption, a merge with one of their kept proposals as a parent:
//!   every file accepted;
//! - a decision `rejected`, `.pigeon/decisions/<commit>.toml`, added by a
//!   commit whose `Pigeon-Accepted:` trailers name the files it accepted
//!   (ADR 0078), the others declined, with the decision's note. A trailer
//!   counts only if the commit's tree holds that file's change
//!   (`proposal::holds`): a decider's claim alone is never shown as an
//!   acceptance. A decision without trailers declines every file, as an
//!   older release writes it.
//!
//! A withdrawal is the author's own, so it is not listed. Who decided is
//! the person of the commit's signer, when it was decided its committer
//! time.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::authority::{self, Decided};
use pigeon_core::roster::{DeviceId, PersonId};
use pigeon_proto::api::Outcome;

use crate::group::Group;

/// How long a decision is shown to its suggestion's author: 30 days.
pub const RECENT: i64 = 30 * 86_400;

/// The decisions of the last [`RECENT`] seconds on the suggestions of
/// person `me`, newest first.
pub async fn outcomes(g: &Group, me: &PersonId) -> Vec<Outcome> {
    match read(g, me).await {
        Ok(out) => out,
        Err(e) => {
            tracing::warn!(group = %g.id, error = %e, "decided-unreadable");
            Vec::new()
        }
    }
}

async fn read(g: &Group, me: &PersonId) -> Result<Vec<Outcome>, crate::git::Error> {
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return Ok(Vec::new());
    };
    let Some(doc) = g.roster() else {
        return Ok(Vec::new());
    };
    let mine: BTreeSet<String> = crate::proposal::kept(&g.git)
        .await
        .into_iter()
        .filter(|(device, _)| {
            doc.roster
                .devices
                .get(&DeviceId(device.clone()))
                .is_some_and(|d| d.person == *me)
        })
        .map(|(_, c)| c)
        .collect();
    if mine.is_empty() {
        return Ok(Vec::new());
    }
    let since = format!("--max-age={}", crate::util::now() - RECENT);
    let mut out = Vec::new();
    let merges = g
        .git
        .run(&["rev-list", "--merges", "--parents", &since, &main])
        .await?;
    for line in merges.lines() {
        let mut ids = line.split(' ');
        let Some(merge) = ids.next() else { continue };
        for x in ids.skip(1).filter(|x| mine.contains(*x)) {
            let Some((at, tree)) = trees(g, x).await? else {
                continue;
            };
            let accepted = authority::delta(&at, &tree).into_iter().collect();
            out.push(outcome(g, x, merge, accepted, Vec::new(), None).await?);
        }
    }
    let decisions = g
        .git
        .run(&["rev-list", &since, &main, "--", ".pigeon/decisions/"])
        .await?;
    for commit in decisions.lines() {
        let added = g
            .git
            .run(&[
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                "--diff-filter=A",
                commit,
                "--",
                ".pigeon/decisions/",
            ])
            .await?;
        for path in added.lines() {
            let Some(x) = path
                .strip_prefix(".pigeon/decisions/")
                .and_then(|p| p.strip_suffix(".toml"))
                .filter(|x| mine.contains(*x))
            else {
                continue;
            };
            if let Some(o) = declined(g, x, commit, path).await? {
                out.push(o);
            }
        }
    }
    out.sort_by(|a, b| b.decided_at.cmp(&a.decided_at));
    Ok(out)
}

/// The tree of the base of the proposal `x` and its own.
async fn trees(
    g: &Group,
    x: &str,
) -> Result<Option<(authority::Tree, authority::Tree)>, crate::git::Error> {
    let Some(base) = crate::proposal::base(g, x).await? else {
        return Ok(None);
    };
    let at = g
        .lock()
        .engine
        .base(std::slice::from_ref(&base))
        .map(|v| v.tree.clone());
    let Some(at) = at else {
        return Ok(None);
    };
    Ok(Some((at, g.git.tree(x).await?)))
}

/// The decision `rejected` on `x` that `commit` adds at `path`, with the
/// files it accepts.
async fn declined(
    g: &Group,
    x: &str,
    commit: &str,
    path: &str,
) -> Result<Option<Outcome>, crate::git::Error> {
    let Some(bytes) = g.git.blob(&format!("{commit}:{path}")).await else {
        return Ok(None);
    };
    let Ok(doc) = crate::decision_file::parse(&String::from_utf8_lossy(&bytes)) else {
        return Ok(None);
    };
    if doc.verdict != Decided::Rejected || doc.proposal != x {
        return Ok(None);
    }
    let Some((at, tree)) = trees(g, x).await? else {
        return Ok(None);
    };
    let raw = g.git.object(commit).await?;
    let claimed: BTreeSet<String> = authority::accepted(&raw).into_iter().collect();
    let (before, after) = if claimed.is_empty() {
        (BTreeMap::new(), BTreeMap::new())
    } else {
        let parent = authority::parents(&raw)
            .into_iter()
            .next()
            .unwrap_or_default();
        (g.git.tree(&parent).await?, g.git.tree(commit).await?)
    };
    let (accepted, declined) = authority::delta(&at, &tree).into_iter().partition(|p| {
        claimed.contains(p) && pigeon_core::proposal::holds(&at, &tree, &before, &after, p)
    });
    Ok(Some(
        outcome(g, x, commit, accepted, declined, doc.note).await?,
    ))
}

async fn outcome(
    g: &Group,
    x: &str,
    commit: &str,
    accepted: Vec<String>,
    declined: Vec<String>,
    note: Option<String>,
) -> Result<Outcome, crate::git::Error> {
    let raw = g.git.object(commit).await?;
    let by = pigeon_core::sig::verify(&raw)
        .ok()
        .and_then(|signer| {
            let doc = g.roster()?;
            let d = doc.roster.devices.get(&signer)?;
            doc.roster.persons.get(&d.person).map(|p| p.name.clone())
        })
        .unwrap_or_default();
    let at = g.git.commit_time(commit).await.unwrap_or(0);
    Ok(Outcome {
        id: x.to_owned(),
        by,
        decided_at: crate::util::rfc3339(at),
        accepted,
        declined,
        note,
    })
}
