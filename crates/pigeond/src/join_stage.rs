//! In-place welcomes are fetched into a private, non-listening repository.
//! Failed validation cannot publish new-group refs through the old group.
//! The staging repo contains no old local objects, and only its admitter
//! can be the destination of its metadata connection (09 §9.9).

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    daemon::Daemon,
    git::Git,
    group::Group,
    invite::{self, Joining},
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

pub(crate) async fn prepare(d: &Daemon, j: &Joining, uuid: &str) -> Result<Verified, String> {
    let inv = &j.inv;
    std::fs::create_dir_all(&d.dirs.state).map_err(|e| e.to_string())?;
    let directory = tempfile::Builder::new()
        .prefix("join-stage-")
        .permissions(std::fs::Permissions::from_mode(0o700))
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
    // No remotedaemon/listener: an admitter cannot PUT counted content into
    // an ephemeral staging repository. Only outgoing metadata is allowed.
    d.groups
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .push(g.clone());
    let result: Result<(String, String, Source), String> = async {
        let role = if j.role == pigeon_core::roster::Role::Custodian {
            pigeon_core::filter::Role::Custodian
        } else {
            pigeon_core::filter::Role::Laptop
        };
        let (actual, token) = crate::settings::init_annex(&git, role, &j.name, inv.k)
            .await
            .map_err(fail)?;
        if actual != uuid {
            return Err("staging annex UUID changed".into());
        }
        g.set_token(&token);
        let w = invite::dial(d, j, uuid).await?;
        let admitter = w.admitter.device.clone();
        g.lock().bootstrap_peer = Some((admitter.clone(), w.annex_uuid.clone()));
        crate::settings::ensure_remote(&git, &admitter, &w.annex_uuid)
            .await
            .map_err(fail)?;
        g.set_addresses(&admitter, invite::admitter_addresses(&w.admitter));
        let main = invite::fetch_welcome(&g, &admitter, &inv.group, &w.commit, &d.device).await?;
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
                device: admitter,
                uuid: w.annex_uuid,
                addresses: w.admitter.addresses,
                peers: w.peers,
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
    let chain = crate::archive_chain::retain(g, root)
        .await
        .map_err(|_| "join-history-unverified".to_owned())?;
    match chain.first() {
        Some((old, tip)) if old == expected_old => Ok(tip.clone()),
        _ => Err("join-history-unverified".into()),
    }
}

pub(crate) fn persist(verified: Verified) -> (PathBuf, String, String, Source) {
    (
        verified.directory.keep(),
        verified.main,
        verified.archive_main,
        verified.source,
    )
}
