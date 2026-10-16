//! Active purge identities from the accepted catalogue (08 §8.6). A
//! decision is identified by its actual accepted introduction and path;
//! cancelling and re-adding identical bytes therefore starts a new wait.
//! Retention comes from its original parents, never today's setting.

use std::collections::BTreeMap;

use pigeon_core::catalogue::Content;
use sha2::{Digest, Sha256};

use crate::git::Error;
use crate::group::Group;
use crate::purge_file::PurgeDoc;

/// One currently active, verified decision.
#[derive(Clone, Debug)]
pub struct Active {
    /// Stable opaque identity, distinct from its filename and blob.
    pub id: String,
    /// The metadata path admins remove to cancel it.
    pub path: String,
    /// The accepted introduction, including an approval of a request.
    pub introduced: String,
    /// Its exact replicated scope and deadline.
    pub doc: PurgeDoc,
    /// The maximum original parent-roster waiting period.
    pub retention_seconds: u64,
}

fn invalid(detail: &str) -> Error {
    Error::Parse {
        argv: vec!["active purge decisions".into()],
        detail: detail.into(),
    }
}

async fn retention(g: &Group, introduced: &str) -> Result<u64, Error> {
    let parents = g
        .git
        .run(&["rev-list", "--parents", "--max-count=1", introduced])
        .await?;
    let mut maximum = None;
    for parent in parents.split_whitespace().skip(1) {
        if !g.lock().engine.accepted(parent) {
            continue;
        }
        let blob = g
            .git
            .blob(&format!("{parent}:.pigeon/roster.toml"))
            .await
            .ok_or_else(|| invalid("missing original parent roster"))?;
        let doc = crate::roster_file::parse(&String::from_utf8_lossy(&blob))
            .map_err(|_| invalid("unknown original parent roster"))?;
        let seconds = u64::from(doc.roster.retention_days) * 86_400;
        maximum = Some(maximum.map_or(seconds, |old: u64| old.max(seconds)));
    }
    maximum.ok_or_else(|| invalid("a purge has no accepted introduction parent"))
}

/// Exact active decisions of `main`, after its complete history is judged.
/// Any unrecognized metadata or missing origin fails closed.
pub async fn active(g: &Group, main: &str) -> Result<BTreeMap<String, Active>, Error> {
    crate::merge::judge_history(g, &[main.to_owned()]).await?;
    if !g.lock().engine.accepted(main) {
        return Err(invalid("unaccepted main"));
    }
    let view = g.lock().engine.catalogue(&[main.to_owned()]);
    let tree = g.git.tree(main).await?;
    let mut decisions = BTreeMap::new();
    for (path, entry) in tree
        .iter()
        .filter(|(p, _)| p.starts_with(crate::purge_file::PREFIX))
    {
        let introduced = view
            .shown
            .values()
            .flatten()
            .find_map(|shown| {
                (shown.path == *path
                    && matches!(&shown.version.content, Content::File(e) if e == entry))
                .then(|| shown.origin.clone())
            })
            .ok_or_else(|| invalid("purge origin absent from accepted catalogue"))?;
        let bytes = g.git.object(&entry.oid).await?;
        let text = std::str::from_utf8(&bytes).map_err(|_| invalid("purge encoding"))?;
        let doc = crate::purge_file::parse(text).map_err(|_| invalid("unknown purge decision"))?;
        let retention_seconds = retention(g, &introduced).await?;
        let mut hash = Sha256::new();
        hash.update(introduced.as_bytes());
        hash.update([0]);
        hash.update(path.as_bytes());
        let id = crate::util::hex(&hash.finalize());
        decisions.insert(
            id.clone(),
            Active {
                id,
                path: path.clone(),
                introduced,
                doc,
                retention_seconds,
            },
        );
    }
    Ok(decisions)
}
