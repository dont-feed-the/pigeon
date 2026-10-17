//! Retained content (05 §5.1, 08 §8.3–8.4): placement and repair use every
//! key in accepted, proposal and archived history (ADR 0047), including
//! overwritten/deleted files.
//! A complete purge policy excludes released keys. Incomplete reference
//! readings pause content rather than refetching previously erased data.

use std::collections::BTreeSet;

use crate::git::Error;
use crate::group::Group;

/// A deliberate content barrier is distinct from unreadable Git history.
pub(crate) fn waiting(error: &Error) -> bool {
    matches!(error, Error::Parse { argv, .. } if argv == &["retained history waiting"])
}

/// One reading of accepted and proposal refs, shared by both status views.
pub(crate) struct Snapshot {
    /// The accepted head in this reading.
    pub main: Option<String>,
    /// Every head retaining content in this reading.
    pub tips: Vec<String>,
    /// The same admitted ref reading, preserving each retention namespace.
    pub named: std::collections::BTreeMap<String, String>,
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
            crate::refound::ARCHIVE,
        ])
        .await?;
    let mut tips = Vec::new();
    let mut main = None;
    let mut named = std::collections::BTreeMap::new();
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
            || name.starts_with(crate::refound::ARCHIVE)
        {
            tips.push(commit.to_owned());
            named.insert(name.to_owned(), commit.to_owned());
        }
    }
    Ok(Snapshot { main, tips, named })
}

/// Retention heads, read without silently ignoring a failed ref listing.
pub async fn tips(g: &Group) -> Result<Vec<String>, Error> {
    Ok(snapshot(g).await?.tips)
}

/// Every key this repository must retain, including historical references.
pub async fn keys(g: &Group) -> Result<BTreeSet<String>, Error> {
    let policy = g.git.purge_gate.policy();
    if policy.waiting {
        return Err(Error::Parse {
            argv: vec!["retained keys".into()],
            detail: "content waits for a complete purge reference reading".into(),
        });
    }
    let mut keys = g.git.retained_keys(&tips(g).await?).await?;
    keys.retain(|key| !policy.released.contains(key));
    Ok(keys)
}

/// States of retained keys outside the current tree, for the group
/// headline. A peer must have an accepted retaining head for that key.
/// Laptops can prove publication; only custodians prove protection.
pub async fn states(
    g: &Group,
    me: &str,
    current: &BTreeSet<String>,
    tips: &[String],
) -> Result<Vec<pigeon_core::state::State<String>>, Error> {
    use pigeon_core::roster::Role;
    use pigeon_core::state::{Custodian, Version, state};
    use pigeon_core::time::Time;
    let mut keys = BTreeSet::new();
    let mut by_tip = Vec::new();
    for tip in tips {
        let retained = g.git.retained_keys(std::slice::from_ref(tip)).await?;
        keys.extend(retained.iter().cloned());
        by_tip.push((tip, retained));
    }
    let policy = g.git.purge_gate.policy();
    if policy.waiting {
        return Err(Error::Parse {
            argv: vec!["retained history waiting".into()],
            detail: "historical content waits for complete purge metadata".into(),
        });
    }
    keys.retain(|k| !current.contains(k) && !policy.released.contains(k));
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let (mut heard, mut lost, mut checking) = {
        let k = g.lock();
        let checking: BTreeSet<String> = k
            .report_files
            .iter()
            .filter(|(_, r)| r.checking)
            .map(|(id, _)| id.clone())
            .collect();
        (k.heard.clone(), k.lost.clone(), checking)
    };
    if let Some(doc) = g.roster() {
        checking.extend(
            doc.roster
                .devices
                .keys()
                .filter(|id| crate::membership::pending(g, &id.0))
                .map(|id| id.0.clone()),
        );
    }
    lost.extend(crate::report::lost(g));
    if g.role == pigeon_core::filter::Role::Custodian
        && crate::health::own_report(g, me)
            .await
            .is_some_and(|r| r.checking)
    {
        checking.insert(me.to_owned());
    }
    let mut peers = Vec::new();
    for (id, uuid, role) in crate::custody::peers(g, me) {
        let prefix = format!("refs/pigeon/peers/{id}/");
        let refs = g
            .git
            .run(&["for-each-ref", "--format=%(objectname)", &prefix])
            .await?;
        let mut named = BTreeSet::new();
        for (tip, retained) in &by_tip {
            for root in refs.lines() {
                if g.git.contains(root, tip).await {
                    named.extend(retained.iter().cloned());
                    break;
                }
            }
        }
        peers.push((id, uuid, role, named));
    }
    if g.role == pigeon_core::filter::Role::Custodian
        && let Some(doc) = g.roster()
        && let Some(device) = doc
            .roster
            .devices
            .get(&pigeon_core::roster::DeviceId(me.to_owned()))
        && device.role == Role::Custodian
        && let Some(uuid) = &device.annex_uuid
        && g.git.config("annex.uuid").await.as_ref() == Some(uuid)
    {
        peers.push((me.to_owned(), uuid.clone(), Role::Custodian, keys.clone()));
        heard.insert(me.to_owned(), crate::util::now());
    }
    let now = Time(crate::util::now());
    let settings = crate::status::settings(g);
    let mut out = Vec::new();
    for key in keys {
        let held = crate::custody::holders(&g.git, &key).await;
        let local = crate::custody::present(g, &key).await.is_some();
        let version = Version {
            custodians: peers
                .iter()
                .filter(|(_, _, role, _)| *role == Role::Custodian)
                .map(|(id, uuid, _, named)| Custodian {
                    device: id.clone(),
                    heard: heard.get(id).copied().map(Time),
                    holds: if id == me { local } else { held.contains(uuid) },
                    has_commit: named.contains(&key),
                    checking: checking.contains(id),
                    repository_lost: lost.contains(id),
                })
                .collect(),
            other_device_has_commit: peers
                .iter()
                .any(|(id, _, _, named)| id != me && named.contains(&key)),
            this_device_holds: local,
            erased: false,
        };
        out.push(state(&version, &settings, now));
    }
    Ok(out)
}
