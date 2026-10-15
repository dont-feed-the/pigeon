//! Retained content (05 §5.1, 08 §8.3–8.4): placement and repair use every
//! key in accepted and proposal history, including overwritten/deleted files.
//! Purge is not implemented, so no reachable reference is released here.

use std::collections::BTreeSet;

use crate::git::Error;
use crate::group::Group;

/// One reading of accepted and proposal refs, shared by both status views.
pub(crate) struct Snapshot {
    /// The accepted head in this reading.
    pub main: Option<String>,
    /// Every head retaining content in this reading.
    pub tips: Vec<String>,
}

/// Read refs once, without silently ignoring a failed listing.
pub(crate) async fn snapshot(g: &Group) -> Result<Snapshot, Error> {
    let refs = g
        .git
        .run(&[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/heads/main",
            crate::proposal::PREFIX,
        ])
        .await?;
    let mut tips = Vec::new();
    let mut main = None;
    for line in refs.lines() {
        let Some((name, commit)) = line.split_once(' ') else {
            return Err(Error::Parse {
                argv: vec!["for-each-ref".to_owned()],
                detail: "unreadable retained reference".to_owned(),
            });
        };
        if name == "refs/heads/main" {
            main = Some(commit.to_owned());
        }
        if name == "refs/heads/main"
            || name
                .strip_prefix(crate::proposal::PREFIX)
                .and_then(|r| r.split_once('/'))
                .is_some_and(|(_, suffix)| suffix == commit)
        {
            tips.push(commit.to_owned());
        }
    }
    Ok(Snapshot { main, tips })
}

/// Retention heads, read without silently ignoring a failed ref listing.
pub async fn tips(g: &Group) -> Result<Vec<String>, Error> {
    Ok(snapshot(g).await?.tips)
}

/// Every key this repository must retain, including historical references.
pub async fn keys(g: &Group) -> Result<BTreeSet<String>, Error> {
    g.git.retained_keys(&tips(g).await?).await
}

/// States of retained keys outside the current tree, for the group
/// headline. Count a custodian conservatively only after it has every
/// retained head, so old bytes cannot count without their signed history.
pub async fn states(
    g: &Group,
    me: &str,
    current: &BTreeSet<String>,
    tips: &[String],
) -> Result<Vec<pigeon_core::state::State<String>>, Error> {
    use pigeon_core::roster::Role;
    use pigeon_core::state::{Custodian, Version, state};
    use pigeon_core::time::Time;
    let mut keys = g.git.retained_keys(tips).await?;
    keys.retain(|k| !current.contains(k));
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let (heard, mut lost) = {
        let k = g.lock();
        (k.heard.clone(), k.lost.clone())
    };
    lost.extend(crate::report::lost(g));
    let mut peers = Vec::new();
    for (id, uuid, role) in crate::custody::peers(g, me) {
        if role != Role::Custodian {
            continue;
        }
        let prefix = format!("refs/pigeon/peers/{id}/");
        let refs = g
            .git
            .run(&["for-each-ref", "--format=%(objectname)", &prefix])
            .await?;
        let mut named = true;
        for tip in tips {
            let mut found = false;
            for root in refs.lines() {
                if g.git.contains(root, tip).await {
                    found = true;
                    break;
                }
            }
            named &= found;
        }
        peers.push((id, uuid, named));
    }
    let now = Time(crate::util::now());
    let settings = crate::status::settings(g);
    let mut out = Vec::new();
    for key in keys {
        let held = crate::custody::holders(&g.git, &key).await;
        let version = Version {
            custodians: peers
                .iter()
                .map(|(id, uuid, named)| Custodian {
                    device: id.clone(),
                    heard: heard.get(id).copied().map(Time),
                    holds: held.contains(uuid),
                    has_commit: *named,
                    checking: false,
                    repository_lost: lost.contains(id),
                })
                .collect(),
            other_device_has_commit: peers.iter().any(|(_, _, named)| *named),
            this_device_holds: crate::custody::present(g, &key).await.is_some(),
            erased: false,
        };
        out.push(state(&version, &settings, now));
    }
    Ok(out)
}
