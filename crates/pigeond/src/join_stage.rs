//! In-place welcomes are fetched into a private, non-listening repository.
//! Failed validation cannot publish new-group refs through the old group.
//! The staging repo contains no old local objects, and only its inviter
//! can be the destination of its metadata connection (09 §9.9).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    daemon::Daemon,
    git::Git,
    group::Group,
    invite::{self, Invitation},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Source {
    pub device: String,
    pub uuid: String,
    pub addresses: Vec<String>,
    pub peers: BTreeMap<String, Vec<String>>,
}

pub(crate) struct Verified {
    pub directory: tempfile::TempDir,
    pub main: String,
    pub archive_main: String,
    pub source: Source,
}

pub(crate) async fn prepare(d: &Daemon, inv: &Invitation, uuid: &str) -> Result<Verified, String> {
    std::fs::create_dir_all(&d.dirs.state).map_err(|e| e.to_string())?;
    let directory = tempfile::Builder::new()
        .prefix("join-stage-")
        .tempdir_in(&d.dirs.state)
        .map_err(|e| e.to_string())?;
    let git = Git::new(directory.path(), d.env.clone());
    let fail = |e: crate::git::Error| e.to_string();
    git.run(&["init", "--bare", "-q", "-b", "main"])
        .await
        .map_err(fail)?;
    for (key, value) in [
        ("user.name", "pigeon join"),
        ("user.email", "join@pigeon"),
        ("core.fsync", "all"),
        ("annex.uuid", uuid),
    ] {
        git.run(&["config", key, value]).await.map_err(fail)?;
    }
    let g = Arc::new(
        Group::open(&inv.group, directory.path(), &d.env)
            .await
            .ok_or("not-a-repository")?,
    );
    let busy = g.busy.lock().await;
    // No remotedaemon/listener: an inviter cannot PUT counted content into
    // an ephemeral staging repository. Only outgoing metadata is allowed.
    d.groups
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .push(g.clone());
    let result: Result<(String, String, Source), String> = async {
        let name = invite::device_name(d, inv);
        let role = if inv.role == "laptop" {
            pigeon_core::filter::Role::Laptop
        } else {
            pigeon_core::filter::Role::Custodian
        };
        let (actual, token) = crate::settings::init_annex(&git, role, &name, inv.k)
            .await
            .map_err(fail)?;
        if actual != uuid {
            return Err("staging annex UUID changed".into());
        }
        g.set_token(&token);
        let request = invite::request(d, inv, &name, uuid)?;
        let (commit, inviter_uuid, peers) = invite::dial(d, inv, &request).await?;
        g.lock().bootstrap_peer = Some((inv.inviter.clone(), inviter_uuid.clone()));
        crate::settings::ensure_remote(&git, &inv.inviter, &inviter_uuid)
            .await
            .map_err(fail)?;
        g.set_addresses(&inv.inviter, invite::inviter_addresses(inv));
        let main = invite::fetch_welcome(&g, &inv.inviter, &inv.group, &commit, &d.device).await?;
        let from = inv
            .refounded
            .as_ref()
            .ok_or("invite-unknown")?
            .from
            .as_str();
        let archive_main = archives(&g, &inv.group, from).await?;
        git.update_ref("refs/pigeon/staged/main", &main, None)
            .await
            .map_err(fail)?;
        pigeon_sys::fs::fsync_dir(directory.path()).map_err(|e| e.to_string())?;
        Ok((
            main,
            archive_main,
            Source {
                device: inv.inviter.clone(),
                uuid: inviter_uuid,
                addresses: inv.addresses.clone(),
                peers,
            },
        ))
    }
    .await;
    d.groups
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|live| !Arc::ptr_eq(live, &g));
    drop(busy);
    let (main, archive_main, source) = result?;
    Ok(Verified {
        directory,
        main,
        archive_main,
        source,
    })
}

/// Keep only archive links actually named by validated signed roots.
async fn archives(g: &Group, root: &str, expected_old: &str) -> Result<String, String> {
    let mut cursor = root.to_owned();
    let mut first = None;
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..32 {
        if !seen.insert(cursor.clone()) {
            return Err("join-history-unverified".into());
        }
        let raw = g.git.object(&cursor).await.map_err(|e| e.to_string())?;
        let Some((old, tip)) = crate::refound::refounded(&raw) else {
            return first.ok_or_else(|| "join-history-unverified".into());
        };
        if first.is_none() && old != expected_old {
            return Err("join-history-unverified".into());
        }
        if g.git.root(&tip).await.as_deref() != Some(old.as_str()) {
            return Err("join-history-unverified".into());
        }
        let judge = g.history_judge(&old);
        if crate::verify::judge(&judge, None, &tip)
            .await
            .map_err(|e| e.to_string())?
            .accepted
            .as_deref()
            != Some(tip.as_str())
        {
            return Err("join-history-unverified".into());
        }
        g.git
            .update_ref(&crate::refound::archive_ref(&old), &tip, None)
            .await
            .map_err(|e| e.to_string())?;
        if first.is_none() {
            first = Some(tip);
        }
        cursor = old;
    }
    Err("join-history-unverified".into())
}

pub(crate) fn persist(verified: Verified) -> (PathBuf, String, String, Source) {
    (
        verified.directory.keep(),
        verified.main,
        verified.archive_main,
        verified.source,
    )
}
