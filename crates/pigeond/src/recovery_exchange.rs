//! The sheet-signed recovery exchange (ADR 0047, 09 §9.9). Custodians
//! serve metadata to a recovery key, then pull its proposed roster commit.
//! Both sides validate history before adopting any ref. The custodian
//! checks the exact requested addition as well as ordinary authority.

use pigeon_core::authority::{Entry, ROSTER};
use pigeon_core::roster::{Device, DeviceId, Edit, Role, apply, may_sign};
use pigeon_proto::join::{JOIN_VERSION, Join, JoinAnswer};

use crate::daemon::Daemon;
use crate::group::Group;
use crate::net::recovery::Bundle;
use crate::recovery::Sheet;

fn refused(code: &str) -> JoinAnswer {
    JoinAnswer::Refused { code: code.into() }
}

async fn export(g: &Group) -> Result<Bundle, String> {
    let bundle = Bundle::new(&g.git_dir).map_err(|e| e.to_string())?;
    let path = bundle.path().to_str().ok_or("bundle path is not UTF-8")?;
    let mut refs = vec!["refs/heads/main".to_owned()];
    if g.git.rev("refs/heads/git-annex").await.is_some() {
        refs.push("refs/heads/git-annex".into());
    }
    let archives = g
        .git
        .run(&[
            "for-each-ref",
            "--format=%(refname)",
            crate::refound::ARCHIVE,
            crate::proposal::PREFIX,
            crate::purge_epoch::PROOFS,
        ])
        .await
        .map_err(|e| e.to_string())?;
    refs.extend(archives.lines().map(str::to_owned));
    let mut args = vec!["bundle", "create", path];
    args.extend(refs.iter().map(String::as_str));
    g.git.run(&args).await.map_err(|e| e.to_string())?;
    Ok(bundle)
}

async fn import(g: &Group, peer: &str, bundle: &Bundle) -> Result<String, String> {
    let path = bundle.path().to_str().ok_or("bundle path is not UTF-8")?;
    // Import only metadata refs into the peer's namespace. No incoming
    // bundle controls main, configuration, local archives or other refs.
    let list = g
        .git
        .run(&["bundle", "list-heads", path])
        .await
        .map_err(|e| e.to_string())?;
    let mut specs = Vec::new();
    for line in list.lines() {
        let Some((_, name)) = line.split_once(' ') else {
            continue;
        };
        let tail = match name {
            "refs/heads/main" => "main".to_owned(),
            "refs/heads/git-annex" => "git-annex".to_owned(),
            _ => {
                if let Some(tail) = name.strip_prefix(crate::proposal::PREFIX) {
                    format!("proposals/{tail}")
                } else if let Some(tail) = name.strip_prefix(crate::refound::ARCHIVE) {
                    format!("archive/{tail}")
                } else if let Some(tail) = name.strip_prefix(crate::purge_epoch::PROOFS) {
                    format!("purge-proof/{tail}")
                } else {
                    continue;
                }
            }
        };
        specs.push(format!("+{name}:refs/pigeon/peers/{peer}/{tail}"));
    }
    if !specs.iter().any(|s| s.ends_with(&format!("/{peer}/main"))) {
        return Err("join-history-unverified".into());
    }
    let mut args = vec![
        "fetch",
        "-q",
        "--no-tags",
        "--no-write-fetch-head",
        "--refmap=",
        path,
    ];
    args.extend(specs.iter().map(String::as_str));
    g.git.run(&args).await.map_err(|e| e.to_string())?;
    while crate::proposal::take(g, peer).await > 0 {}
    g.git
        .rev(&format!("refs/pigeon/peers/{peer}/main"))
        .await
        .ok_or("invite-unknown".into())
}

fn addition(
    doc: &crate::roster_file::RosterDoc,
    signer: &str,
    new: &str,
    join: &Join,
) -> Option<crate::roster_file::RosterDoc> {
    let signer = DeviceId(signer.into());
    let recovery = doc.roster.devices.get(&signer)?;
    if recovery.role != Role::Recovery
        || join.role != "laptop"
        || doc.roster.devices.contains_key(&DeviceId(new.into()))
        || join.annex_uuid.is_empty()
    {
        return None;
    }
    let edits = [Edit::AddDevice(
        DeviceId(new.into()),
        Device {
            person: recovery.person.clone(),
            name: join.device_name.clone(),
            role: Role::Laptop,
            annex_uuid: Some(join.annex_uuid.clone()),
            maxsize: None,
        },
    )];
    if !may_sign(&doc.roster, &signer, &edits) {
        return None;
    }
    let applied = apply(&doc.roster, &edits);
    if applied.skipped.is_some() || applied.roster.validate().is_err() {
        return None;
    }
    Some(crate::roster_file::RosterDoc {
        roster: applied.roster,
    })
}

/// Only a current recovery key can fetch this group's metadata, and only
/// a custodian serves it. Recovery devices cannot open annex streams.
pub async fn bundle(d: &Daemon, group: &str, device: &str) -> Option<Bundle> {
    let g = d.group(group)?;
    let _busy = g.busy.lock().await;
    let doc = g.roster()?;
    if g.role != pigeon_core::filter::Role::Custodian
        || doc.roster.devices.get(&DeviceId(device.into()))?.role != Role::Recovery
    {
        return None;
    }
    match export(&g).await {
        Ok(bundle) => Some(bundle),
        Err(error) => {
            tracing::warn!(group = %g.id, %error, "recovery-metadata-export-failed");
            None
        }
    }
}

/// The custodian never writes a roster contribution on the sheet's behalf.
pub async fn welcome(d: &Daemon, device: &str, new: &str, join: &Join) -> JoinAnswer {
    if join.recovery_version != Some(1) || crate::net::parse_device_id(new).is_none() {
        return refused("invite-not-allowed");
    }
    for g in d.groups() {
        let _busy = g.busy.lock().await;
        let Some(doc) = g.roster() else { continue };
        if !doc.roster.devices.contains_key(&DeviceId(device.into())) {
            continue;
        }
        if g.role != pigeon_core::filter::Role::Custodian {
            return refused("invite-not-allowed");
        }
        let Some(expected) = addition(&doc, device, new, join) else {
            return refused("invite-not-allowed");
        };
        let Some(parent) = g.git.rev("refs/heads/main").await else {
            return refused("not-in-group");
        };
        let commit = if let Some(commit) = &join.recovery_commit {
            match pull(d, &g, device, new, join, &parent, commit, &expected).await {
                Ok(()) => commit.clone(),
                Err(e) => {
                    tracing::warn!(group = %g.id, error = %e, "recovery-refused");
                    return refused("join-history-unverified");
                }
            }
        } else {
            parent
        };
        let annex_uuid = g.lock().annex_uuid.clone().unwrap_or_default();
        return JoinAnswer::Welcome {
            commit,
            annex_uuid,
            peers: g.known_addresses(new),
        };
    }
    refused("invite-unknown")
}

#[allow(
    clippy::too_many_arguments,
    reason = "the authenticated recovery exchange"
)]
async fn pull(
    d: &Daemon,
    g: &Group,
    signer: &str,
    new: &str,
    join: &Join,
    parent: &str,
    commit: &str,
    expected: &crate::roster_file::RosterDoc,
) -> Result<(), String> {
    let addresses = join
        .recovery_addresses
        .iter()
        .filter_map(|a| a.parse().ok())
        .collect();
    let bundle = crate::net::recovery::fetch(&d.node, signer, addresses, &g.id, &g.git_dir).await?;
    let tip = import(g, signer, &bundle).await?;
    let raw = g.git.object(&tip).await.map_err(|e| e.to_string())?;
    let verified = pigeon_core::sig::verify(&raw).map_err(|e| format!("signature {e:?}"))?;
    let parents = g
        .git
        .run(&["rev-list", "--parents", "-n", "1", &tip])
        .await
        .map_err(|e| e.to_string())?;
    let parsed = g
        .git
        .blob(&format!("{tip}:{ROSTER}"))
        .await
        .and_then(|b| crate::roster_file::parse(&String::from_utf8_lossy(&b)).ok());
    if tip != commit
        || verified.0 != signer
        || parents.trim() != format!("{tip} {parent}")
        || parsed.as_ref() != Some(expected)
    {
        return Err("recovery commit does not match the requested addition".into());
    }
    crate::verify::bootstrap(g, &g.id, &tip, &tip, new).await?;
    g.git
        .update_ref("refs/heads/main", &tip, Some(parent))
        .await
        .map_err(|e| e.to_string())?;
    g.reload_roster().await;
    crate::settings::ensure_remote(&g.git, new, &join.annex_uuid)
        .await
        .map_err(|e| e.to_string())?;
    g.set_addresses(
        new,
        join.addresses
            .iter()
            .filter_map(|a| a.parse().ok())
            .collect(),
    );
    crate::trace_roster::moved(&d.tracer, g, "Merge", Some(parent)).await;
    d.wake.notify_one();
    Ok(())
}

#[allow(clippy::too_many_arguments, reason = "the local recovery inputs")]
pub async fn recover(
    d: &Daemon,
    g: &Group,
    sheet: &Sheet,
    secret: &[u8; 32],
    node: &crate::net::Node,
    name: &str,
    uuid: &str,
    from: Option<&str>,
) -> Result<String, String> {
    let mut join = Join {
        v: JOIN_VERSION,
        traceparent: None,
        invitation_hash: String::new(),
        proof: String::new(),
        device_name: name.into(),
        role: "laptop".into(),
        annex_uuid: uuid.into(),
        capacity: None,
        addresses: d.addresses().iter().map(ToString::to_string).collect(),
        new_device: Some(d.device.clone()),
        recovery_commit: None,
        recovery_addresses: node
            .advertised_addresses()
            .iter()
            .map(ToString::to_string)
            .collect(),
        recovery_version: Some(1),
    };
    let targets = sheet
        .custodians
        .iter()
        .filter(|c| from.is_none_or(|f| c.name == f || c.device == f));
    let mut last = "peer-unreachable";
    for c in targets {
        let addresses = || c.addresses.iter().filter_map(|a| a.parse().ok()).collect();
        let (parent, annex_uuid, peers) = match node.join(&c.device, addresses(), &join).await {
            Ok(JoinAnswer::Welcome {
                commit,
                annex_uuid,
                peers,
            }) => (commit, annex_uuid, peers),
            Ok(JoinAnswer::Refused { .. }) => {
                last = "recovery-not-allowed";
                continue;
            }
            Err(_) => continue,
        };
        let bundle =
            crate::net::recovery::fetch(node, &c.device, addresses(), &sheet.group, &g.git_dir)
                .await?;
        let tip = import(g, &c.device, &bundle).await?;
        crate::verify::bootstrap(g, &sheet.group, &tip, &parent, &node.device_id()).await?;
        let base = g
            .git
            .blob(&format!("{tip}:{ROSTER}"))
            .await
            .ok_or("join-history-unverified")?;
        let doc = crate::roster_file::parse(&String::from_utf8_lossy(&base))
            .map_err(|_| "join-history-unverified")?;
        if doc
            .roster
            .devices
            .get(&DeviceId(node.device_id()))
            .map(|x| x.person.0.as_str())
            != Some(sheet.person.as_str())
        {
            return Err("invite-not-allowed".into());
        }
        let added =
            addition(&doc, &node.device_id(), &d.device, &join).ok_or("invite-not-allowed")?;
        let text = crate::roster_file::render(&added)?;
        let blob = g
            .git
            .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut tree = g.git.tree(&tip).await.map_err(|e| e.to_string())?;
        tree.insert(
            ROSTER.into(),
            Entry {
                mode: "100644".into(),
                kind: "blob".into(),
                oid: blob.trim().into(),
            },
        );
        let tree = crate::merge::write_tree(&g.git, &tree)
            .await
            .map_err(|e| e.to_string())?;
        let unsigned = g
            .git
            .run(&[
                "commit-tree",
                &tree,
                "-p",
                &tip,
                "-m",
                "pigeon: recover with a sheet",
            ])
            .await
            .map_err(|e| e.to_string())?;
        let signed = g
            .git
            .sign(unsigned.trim(), secret)
            .await
            .map_err(|e| e.to_string())?;
        crate::verify::bootstrap(g, &sheet.group, &signed, &signed, &d.device).await?;
        g.git
            .update_ref("refs/heads/main", &signed, None)
            .await
            .map_err(|e| e.to_string())?;
        let offered = export(g).await?;
        let offer = tokio::spawn(crate::net::recovery::offer(
            node.clone(),
            c.device.clone(),
            sheet.group.clone(),
            offered,
        ));
        join.recovery_commit = Some(signed.clone());
        let answer = node.join(&c.device, addresses(), &join).await;
        offer.abort();
        let _ = offer.await;
        if !matches!(answer, Ok(JoinAnswer::Welcome { commit, .. }) if commit == signed) {
            return Err("join-history-unverified".into());
        }
        crate::settings::ensure_remote(&g.git, &c.device, &annex_uuid)
            .await
            .map_err(|e| e.to_string())?;
        for c in &sheet.custodians {
            g.set_addresses(
                &c.device,
                c.addresses.iter().filter_map(|a| a.parse().ok()).collect(),
            );
        }
        g.learn_peers(&peers);
        if let Some(annex) = g
            .git
            .rev(&format!("refs/pigeon/peers/{}/git-annex", c.device))
            .await
        {
            let _ = g
                .git
                .run(&[
                    "update-ref",
                    &format!("refs/remotes/{}/git-annex", c.device),
                    &annex,
                ])
                .await;
            let _ = g.git.run(&["annex", "merge"]).await;
        }
        crate::refound::adopt(g).await;
        return Ok(signed);
    }
    Err(last.into())
}
