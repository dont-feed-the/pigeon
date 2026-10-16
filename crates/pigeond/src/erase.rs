//! Explicit erasure API (08 §8.6). Read-only previews never prepare the
//! protocol or capture files. Every destructive action binds freshly read
//! history to the person's review and rechecks under the repository lock.

use crate::{daemon::Daemon, erase_commit::refuse, git::Error, group::Group};
use pigeon_proto::{
    Response,
    erase::{Answer, Operation, Selection},
};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};

fn group(d: &Daemon, selection: Option<&Selection>) -> Option<Arc<Group>> {
    match selection {
        Some(Selection::Path(path)) => d.group_at(Path::new(path)),
        _ => d.default_group(),
    }
}

async fn head(g: &Group) -> Result<String, Error> {
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| refuse("not-in-group", "no main"))?;
    if !g.lock().engine.accepted(&main) {
        return Err(refuse("erase-checking", "main is not verified"));
    }
    Ok(main)
}

/// Local socket entry point, shared by the command line and browser.
pub async fn handle(d: &Daemon, operation: Operation) -> Vec<Response> {
    match perform(d, operation).await {
        Ok(answer) => vec![Response::Erasure {
            answer: Box::new(answer),
        }],
        Err(error) => {
            let code = match &error {
                Error::Parse { argv, .. } => argv
                    .first()
                    .filter(|s| {
                        matches!(
                            s.as_str(),
                            "erase-stale"
                                | "erase-pending"
                                | "erase-not-ready"
                                | "erase-checking"
                                | "erase-unknown"
                                | "not-allowed"
                                | "not-in-group"
                        )
                    })
                    .map(String::as_str)
                    .unwrap_or("erase-checking"),
                _ => "erase-checking",
            };
            vec![crate::api::problem(code, &[], Some(error.to_string()))]
        }
    }
}

async fn perform(d: &Daemon, operation: Operation) -> Result<Answer, Error> {
    let selection = match &operation {
        Operation::Preview { selection }
        | Operation::Prepare { selection }
        | Operation::Confirm { selection, .. } => Some(selection),
        _ => None,
    };
    let g =
        group(d, selection).ok_or_else(|| refuse("not-in-group", "no group for this selection"))?;
    let main = head(&g).await?;
    match operation {
        Operation::Preview { selection } => Ok(Answer::Preview(
            crate::erase_preview::plan(&g, &d.device, &selection)
                .await?
                .preview,
        )),
        Operation::List => Ok(Answer::Decisions {
            can_cancel: crate::erase_state::readiness(&g, &main).await?.can_prepare,
            decisions: crate::erase_state::decisions(&g, &main).await?,
        }),
        Operation::Prepare { .. } => {
            let _busy = g.busy.lock().await;
            crate::admission_publish::start(&g).await?;
            d.admissions.round(&g, &d.device).await;
            crate::report::publish(&g, &d.device).await;
            d.wake.notify_one();
            Ok(Answer::Preparing(
                crate::erase_state::readiness(&g, &head(&g).await?).await?,
            ))
        }
        Operation::Confirm { selection, token } => {
            let plan = crate::erase_preview::plan(&g, &d.device, &selection).await?;
            if plan.preview.token != token {
                return Err(refuse("erase-stale", "preview no longer matches"));
            }
            let _busy = g.busy.lock().await;
            if !crate::erase_preview::unchanged(&g, &plan).await? {
                return Err(refuse("erase-stale", "history changed"));
            }
            d.admissions.round(&g, &d.device).await;
            let answer = crate::erase_commit::submit(&g, &d.device, plan, &d.tracer).await?;
            d.wake.notify_one();
            Ok(answer)
        }
        Operation::PreviewCancel { decision } => {
            let (decision, token) = cancellation(&g, &d.device, &decision).await?;
            Ok(Answer::CancelPreview { decision, token })
        }
        Operation::Cancel { decision, token } => {
            let _busy = g.busy.lock().await;
            let (decision, fresh) = cancellation(&g, &d.device, &decision).await?;
            if token != fresh {
                return Err(refuse("erase-stale", "cancellation preview changed"));
            }
            let main = head(&g).await?;
            let active = crate::purge_state::active(&g, &main).await?;
            let active = active
                .get(&decision.id)
                .ok_or_else(|| refuse("erase-unknown", "no active decision"))?;
            let mut tree = g.git.tree(&main).await?;
            tree.remove(&active.path);
            let tree = crate::merge::write_tree(&g.git, &tree).await?;
            let commit = g
                .git
                .commit_tree(&tree, &[&main], "pigeon: cancel pending erasure")
                .await?;
            crate::merge::judge_history(&g, std::slice::from_ref(&commit)).await?;
            if !g.lock().engine.accepted(&commit) {
                return Err(refuse("not-allowed", "cancellation not accepted"));
            }
            g.git
                .update_ref("refs/heads/main", &commit, Some(&main))
                .await?;
            d.wake.notify_one();
            Ok(Answer::Cancelled { id: decision.id })
        }
    }
}

async fn cancellation(
    g: &Group,
    me: &str,
    prefix: &str,
) -> Result<(pigeon_proto::erase::Decision, String), Error> {
    let main = head(g).await?;
    let roster = crate::membership::roster(g, &main).await?;
    if !roster
        .devices
        .get(&pigeon_core::roster::DeviceId(me.into()))
        .filter(|d| d.role != pigeon_core::roster::Role::Recovery)
        .and_then(|d| roster.persons.get(&d.person))
        .is_some_and(|p| p.admin)
    {
        return Err(refuse("not-allowed", "only an admin can cancel erasing"));
    }
    let mut found: Vec<_> = crate::erase_state::decisions(g, &main)
        .await?
        .into_iter()
        .filter(|decision| {
            prefix.len() >= 4 && decision.id.starts_with(prefix) && !decision.completed
        })
        .collect();
    if found.len() != 1 {
        return Err(refuse(
            "erase-unknown",
            "pending decision absent or ambiguous",
        ));
    }
    let decision = found.remove(0);
    let refs = crate::retention::snapshot(g).await?;
    let bytes = serde_json::to_vec(&(&g.id, &refs.named, &decision.id))
        .map_err(|e| refuse("erase-checking", &e.to_string()))?;
    let token = crate::util::hex(&Sha256::digest(bytes));
    Ok((decision, token))
}
