//! Historical scope checks for signed purge additions (04 §4.4, 08 §8.6).
//! A supplied SHA is not proof of accepted history. Archive scope must be
//! bound to the current genesis's signed chain and pass its own judge.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::authority::{self, PurgeCheck, Tree, Unread};

use crate::git::Error;
use crate::group::Group;

/// Only validated purge additions may be metadata-bearing proposals.
/// Ordinary decisions and roster edits never acquire proposal authority.
pub(crate) fn proposal_metadata(before: &Tree, after: &Tree) -> bool {
    authority::delta(before, after)
        .iter()
        .all(|p| !authority::is_pigeon_path(p) || p.starts_with(crate::purge_file::PREFIX))
}

/// Approval starts a fresh minimum window. Extend only the serialized
/// deadline, leaving requester, observed bases, paths and keys unchanged.
pub(crate) async fn extend_deadline(
    g: &Group,
    base: &Tree,
    proposed: &Tree,
    tree: &mut Tree,
) -> Result<(), Error> {
    let invalid = |detail: &str| Error::Parse {
        argv: vec!["purge approval".into()],
        detail: detail.into(),
    };
    let Some(roster) = g.roster() else {
        return Err(invalid("no accepted roster"));
    };
    let minimum = crate::util::now()
        .checked_add(i64::from(roster.roster.retention_days) * 86_400)
        .and_then(|t| t.checked_add(60))
        .ok_or_else(|| invalid("retention time overflow"))?;
    for path in authority::delta(base, proposed)
        .iter()
        .filter(|p| p.starts_with(crate::purge_file::PREFIX))
    {
        let Some(entry) = tree.get(path).cloned() else {
            continue;
        };
        let bytes = g.git.object(&entry.oid).await?;
        let text = std::str::from_utf8(&bytes).map_err(|_| invalid("purge encoding"))?;
        let mut doc = crate::purge_file::parse(text).map_err(|_| invalid("purge format"))?;
        if doc.end().is_some_and(|end| end >= minimum) {
            continue;
        }
        doc.retention_end = crate::report_file::datetime(minimum);
        let text = crate::purge_file::render(&doc).map_err(|_| invalid("purge serialization"))?;
        let oid = g
            .git
            .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
            .await?;
        tree.insert(
            path.clone(),
            authority::Entry {
                oid: oid.trim().into(),
                ..entry
            },
        );
    }
    Ok(())
}

/// Parse each addition. Core admission checks P4 under every parent roster
/// and restricts adoption/cancellation to admins.
pub(crate) async fn checks(
    g: &Group,
    after: &Tree,
    changed: &BTreeSet<String>,
    adopted: &[pigeon_core::commit::Adopted<'_>],
    parents: &[String],
) -> Result<BTreeMap<String, Result<PurgeCheck, Unread>>, Error> {
    let mut out = BTreeMap::new();
    if !changed
        .iter()
        .any(|p| p.starts_with(crate::purge_file::PREFIX) && after.contains_key(p))
    {
        return Ok(out);
    }
    let mut quorum_ready = false;
    for parent in parents {
        if !g.lock().engine.accepted(parent) {
            continue;
        }
        let roster = crate::membership::roster(g, parent).await?;
        if roster.rules < pigeon_core::purge::RULES {
            quorum_ready = false;
            break;
        }
        let state = crate::membership::read(g, parent).await?;
        if state.eligible.is_empty() || !state.pending.is_empty() {
            quorum_ready = false;
            break;
        }
        quorum_ready = true;
    }
    for path in changed
        .iter()
        .filter(|p| p.starts_with(crate::purge_file::PREFIX))
    {
        let Some(entry) = after.get(path) else {
            continue;
        };
        let bytes = g.git.object(&entry.oid).await?;
        let parsed = match std::str::from_utf8(&bytes) {
            Ok(text) => crate::purge_file::parse(text),
            Err(_) => Err(Unread::Malformed("purge encoding".into())),
        };
        let doc = match parsed {
            Ok(doc) => doc,
            Err(e) => {
                out.insert(path.clone(), Err(e));
                continue;
            }
        };
        let base_seen = accepted(g, &doc.base).await?;
        let mut proposals_seen = true;
        for tip in &doc.proposals {
            if !proposal(g, tip).await?
                || (!doc.history
                    && crate::proposal::base(g, tip).await?.as_deref() != Some(doc.base.as_str()))
            {
                proposals_seen = false;
                break;
            }
        }
        let mut adoption_blobs = BTreeSet::new();
        for proposal in adopted {
            if let Some(original) = proposal.tree.get(path) {
                let bytes = g.git.object(&original.oid).await?;
                if let Ok(text) = std::str::from_utf8(&bytes)
                    && let Ok(mut old) = crate::purge_file::parse(text)
                    && old.end() <= doc.end()
                {
                    old.retention_end = doc.retention_end;
                    if old == doc {
                        adoption_blobs.insert(original.oid.clone());
                    }
                }
            }
        }
        let value = doc
            .end()
            .map(|end| PurgeCheck {
                end,
                quorum_ready,
                base_seen,
                proposals_seen,
                adoption_blobs,
            })
            .ok_or_else(|| Unread::Malformed("purge UTC deadline".into()));
        out.insert(path.clone(), value);
    }
    Ok(out)
}

async fn archive(g: &Group, root: &str) -> Result<Option<Group>, Error> {
    let mut cursor = g.id.clone();
    let mut seen = BTreeSet::new();
    for _ in 0..31 {
        if !seen.insert(cursor.clone()) {
            return Ok(None);
        }
        let raw = g.git.object(&cursor).await?;
        if pigeon_core::sig::verify(&raw).is_err() {
            return Ok(None);
        }
        let Some((old, tip)) = crate::refound::refounded(&raw) else {
            return Ok(None);
        };
        if g.git.root(&tip).await.as_deref() != Some(&old) {
            return Ok(None);
        }
        let judge = g.history_judge(&old);
        let verdict = Box::pin(crate::verify::judge(&judge, None, &tip)).await?;
        if verdict.accepted.as_deref() != Some(tip.as_str()) {
            return Ok(None);
        }
        if old == root {
            return Ok(Some(judge));
        }
        cursor = old;
    }
    Ok(None)
}

async fn accepted(g: &Group, tip: &str) -> Result<bool, Error> {
    if g.lock().engine.accepted(tip) {
        return Ok(true);
    }
    let Some(root) = g.git.root(tip).await else {
        return Ok(false);
    };
    if root == g.id {
        let result = Box::pin(crate::verify::judge(g, None, tip)).await?;
        return Ok(result.accepted.as_deref() == Some(tip));
    }
    let Some(judge) = Box::pin(archive(g, &root)).await? else {
        return Ok(false);
    };
    // Merely being an ancestor is insufficient: an accepted adoption may
    // contain refused proposal parents. The base itself must be accepted.
    Ok(judge.lock().engine.accepted(tip))
}

async fn proposal(g: &Group, tip: &str) -> Result<bool, Error> {
    let Ok(raw) = g.git.object(tip).await else {
        return Ok(false);
    };
    let Ok(signer) = pigeon_core::sig::verify(&raw) else {
        return Ok(false);
    };
    let Some(root) = g.git.root(tip).await else {
        return Ok(false);
    };
    if root == g.id {
        return Box::pin(crate::proposal::valid(g, &signer.0, tip)).await;
    }
    let Some(judge) = Box::pin(archive(g, &root)).await? else {
        return Ok(false);
    };
    Box::pin(crate::proposal::valid(&judge, &signer.0, tip)).await
}
