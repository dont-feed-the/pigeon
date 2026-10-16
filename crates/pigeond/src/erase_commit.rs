//! Confirm an exact erasure scope using an isolated tree and a main CAS
//! (08 §8.6). A durable install marker bridges the CAS/restart gap, so a
//! clean old file cannot be captured again before installing its deletion.

use pigeon_core::authority::{Entry, Refusal, Verdict};
use pigeon_proto::erase::Answer;
use serde::{Deserialize, Serialize};

use crate::{git::Error, group::Group, membership::invalid, trace::Tracer};

fn pending(g: &Group) -> std::path::PathBuf {
    g.root().join("erase-install.toml")
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Install {
    format: u32,
    before: String,
    after: String,
}

/// Recovery runs before every capture, including captures from local API
/// requests. Unknown or torn markers are preserved and stop capture.
pub async fn recover(g: &Group, me: &str, tracer: &Tracer) -> Result<(), Error> {
    let path = pending(g);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(invalid(&e.to_string())),
    };
    let install: Install = toml_edit::de::from_str(&text).map_err(|e| invalid(&e.to_string()))?;
    if install.format != 1
        || !crate::purge_file::commit_id(&install.before)
        || !crate::purge_file::commit_id(&install.after)
    {
        return Err(invalid("unrecognized erasure installation marker"));
    }
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| invalid("no main"))?;
    if main != install.before {
        if !g.git.contains(&main, &install.after).await {
            return Err(invalid(
                "erasure installation waits for its committed history",
            ));
        }
        if crate::evict::unsettled(g) || crate::capture_file::pending(g) {
            return Err(invalid("erasure installation waits for journal recovery"));
        }
        crate::merge::judge_history(g, std::slice::from_ref(&main)).await?;
        if !g.lock().engine.accepted(&main) {
            return Err(invalid("erasure installation waits for verified main"));
        }
        let before = g.git.tree(&install.before).await?;
        let after = g.git.tree(&install.after).await?;
        let paths = pigeon_core::authority::delta(&before, &after);
        crate::install::incoming_erasure(g, me, tracer, &install.after, &paths).await?;
    }
    std::fs::remove_file(&path).map_err(|e| invalid(&e.to_string()))?;
    pigeon_sys::fs::fsync_dir(&g.root()).map_err(|e| invalid(&e.to_string()))
}

/// A stable code with diagnostic detail kept out of the rendered message.
pub(crate) fn refuse(code: &str, detail: &str) -> Error {
    Error::Parse {
        argv: vec![code.into()],
        detail: detail.into(),
    }
}

/// Caller holds `busy`, has refreshed admission fences, and has compared
/// the preview token. This function rechecks the refs immediately before
/// constructing any metadata; private index/worktree bytes are untouched.
pub async fn submit(
    g: &Group,
    me: &str,
    mut plan: crate::erase_preview::Plan,
    tracer: &Tracer,
) -> Result<Answer, Error> {
    if !crate::erase_preview::unchanged(g, &plan).await? {
        return Err(refuse("erase-stale", "retained history changed"));
    }
    if crate::capture_file::pending(g) || crate::evict::unsettled(g) || pending(g).exists() {
        return Err(refuse(
            "erase-pending",
            "finish local installation recovery first",
        ));
    }
    let main = plan
        .reading
        .main
        .clone()
        .ok_or_else(|| invalid("no main"))?;
    if !crate::erase_state::readiness(g, &main).await?.ready {
        return Err(refuse(
            "erase-not-ready",
            "capability or custody barrier is incomplete",
        ));
    }
    let mut tree = g.git.tree(&main).await?;
    tree.retain(|path, _| !crate::erase_preview::removes(&plan, path));
    let end = crate::util::now()
        .checked_add(i64::from(plan.preview.retention_days) * 86_400)
        .and_then(|t| t.checked_add(60))
        .ok_or_else(|| invalid("retention time overflow"))?;
    let nonce = crate::util::random::<16>().map_err(|e| invalid(&e.to_string()))?;
    for (n, doc) in plan.documents.iter_mut().enumerate() {
        doc.retention_end = crate::report_file::datetime(end);
        let text = crate::purge_file::render(doc).map_err(|e| invalid(&e.to_string()))?;
        let oid = g
            .git
            .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
            .await?;
        tree.insert(
            format!(
                "{}{}-{n}.toml",
                crate::purge_file::PREFIX,
                crate::util::hex(&nonce)
            ),
            Entry {
                mode: "100644".into(),
                kind: "blob".into(),
                oid: oid.trim().into(),
            },
        );
    }
    let tree_id = crate::merge::write_tree(&g.git, &tree).await?;
    let commit = g
        .git
        .commit_tree(
            &tree_id,
            &[&main],
            "pigeon: request erasure of reviewed versions",
        )
        .await?;
    crate::merge::judge_history(g, std::slice::from_ref(&commit)).await?;
    let accepted = g.lock().engine.accepted(&commit);
    if !accepted {
        let raw = g.git.object(&commit).await?;
        let verdict = crate::merge::judge_proposal(g, &raw, &main, &tree).await?;
        if !matches!(verdict, Verdict::Refused(Refusal::NotAllowed(_))) {
            return Err(refuse(
                "erase-not-ready",
                "erasure request did not pass independent validation",
            ));
        }
        g.git
            .update_ref(&crate::proposal::name(me, &commit), &commit, None)
            .await?;
        return Ok(Answer::Submitted {
            id: commit,
            needs_approval: true,
        });
    }
    let marker = Install {
        format: 1,
        before: main.clone(),
        after: commit.clone(),
    };
    let text = toml_edit::ser::to_string(&marker).map_err(|e| invalid(&e.to_string()))?;
    pigeon_sys::fs::write_atomic(&pending(g), text.as_bytes())
        .map_err(|e| invalid(&e.to_string()))?;
    pigeon_sys::lab::crash_point("erase.before-main");
    g.git
        .update_ref("refs/heads/main", &commit, Some(&main))
        .await?;
    pigeon_sys::lab::crash_point("erase.after-main");
    if let Err(error) = recover(g, me, tracer).await {
        // The request is already durable. Recovery keeps the marker and
        // blocks capture; do not invite a duplicate destructive request.
        tracing::warn!(group = %g.id, %error, "erase-install-waits");
    }
    Ok(Answer::Submitted {
        id: commit,
        needs_approval: false,
    })
}
