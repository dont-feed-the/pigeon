//! Invitations and joining (09 §9.6), for backup machines in the slice.
//!
//! `invite --backup-machine` writes a one-time invitation; `join
//! --backup-machine` makes the custodian's bare repository, sends `Join`
//! on `pigeon/join/1`, and once welcomed fetches `main` from the inviter
//! and checks its genesis. The inviter keeps each invitation's secret in
//! `invitations.json`, mode 0600, since it checks the HMAC keyed by it
//! (13 §13.12 corrects 09 §9.6, which kept only its hash).

use std::path::PathBuf;
use std::time::Duration;

use data_encoding::BASE32_NOPAD;
use pigeon_core::roster::{Device, DeviceId, Edit, PersonId, Role as DeviceRole, apply, may_sign};
use pigeon_proto::join::{JOIN_VERSION, Join, JoinAnswer};
use serde::{Deserialize, Serialize};

use crate::daemon::Daemon;
use crate::group::Group;
use crate::settings::{ensure_remote, init_annex};
use crate::util::{hex, hmac_sha256, now, random, sha256_hex, unhex};

/// How long an invitation works.
const LIFETIME: i64 = 24 * 3600;

/// What an invitation code carries (09 §9.6), as JSON in base32.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invitation {
    /// Its version, 1.
    pub v: u32,
    /// The group ID.
    pub group: String,
    /// The group's name.
    pub name: String,
    /// K, for the new custodian's `wanted`.
    pub k: u32,
    /// The inviter's device ID.
    pub inviter: String,
    /// The inviter's name in the roster, for messages.
    #[serde(default)]
    pub from: String,
    /// The inviter's direct addresses.
    pub addresses: Vec<String>,
    /// 256 random bits, hex.
    pub secret: String,
    /// The person the new device belongs to.
    pub person: String,
    /// `custodian` or `laptop`.
    pub role: String,
    /// The new device's name in the roster, if the inviter chose one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
    /// When it stops working, in Unix seconds.
    pub expires: i64,
}

/// Writes an invitation as `pgn1-…`.
pub fn encode(inv: &Invitation) -> String {
    let json = serde_json::to_vec(inv).unwrap_or_default();
    format!("pgn1-{}", BASE32_NOPAD.encode(&json).to_ascii_lowercase())
}

/// Reads a `pgn1-…` invitation.
pub fn decode(code: &str) -> Option<Invitation> {
    let body = code.trim().strip_prefix("pgn1-")?;
    let bytes = BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// The proof of 09 §9.6: HMAC-SHA256(secret, "pigeon-join-1" ‖ new ‖ inviter).
pub fn proof(secret: &[u8], new_device: &str, inviter: &str) -> String {
    let message = format!("pigeon-join-1{new_device}{inviter}");
    hex(&hmac_sha256(secret, message.as_bytes()))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Records {
    format: u32,
    invitations: Vec<Record>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Record {
    id: String,
    secret: String,
    secret_sha256: String,
    purpose: serde_json::Value,
    expires: String,
    used_by: Option<String>,
}

fn load(g: &Group) -> Records {
    std::fs::read_to_string(g.files.invitations())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Records {
            format: 1,
            invitations: Vec::new(),
        })
}

fn save(g: &Group, r: &Records) -> std::io::Result<()> {
    let text = serde_json::to_vec_pretty(r).map_err(std::io::Error::other)?;
    crate::util::write_private(&g.files.invitations(), &text)
}

/// Makes an invitation for a backup machine of `person` (the inviter's
/// own person if empty) in `g`, named `name` in the roster if not empty.
pub fn invite(d: &Daemon, g: &Group, person: &str, name: &str) -> Result<String, &'static str> {
    let doc = g.roster().ok_or("not-in-group")?;
    let me = doc
        .roster
        .devices
        .get(&DeviceId(d.device.clone()))
        .ok_or("not-in-group")?;
    let person = if person.is_empty() {
        me.person.0.clone()
    } else {
        person.to_ascii_lowercase()
    };
    if !doc.roster.persons.contains_key(&PersonId(person.clone())) {
        return Err("invite-not-allowed");
    }
    let name = name.trim();
    if !name.is_empty() && doc.roster.devices.values().any(|x| x.name == name) {
        return Err("invite-not-allowed");
    }
    let secret = random::<32>().map_err(|_| "not-allowed")?;
    let expires = now() + LIFETIME;
    let mut records = load(g);
    records.invitations.push(Record {
        id: format!("inv-{}", hex(secret.get(..2).unwrap_or_default())),
        secret: hex(&secret),
        secret_sha256: sha256_hex(&secret),
        purpose: serde_json::json!({"new-device": {"person": person, "role": "custodian", "name": name}}),
        expires: crate::util::rfc3339(expires),
        used_by: None,
    });
    save(g, &records).map_err(|_| "not-allowed")?;
    Ok(encode(&Invitation {
        v: 1,
        group: g.id.clone(),
        name: doc.roster.group.clone(),
        k: doc.roster.k,
        inviter: d.device.clone(),
        from: me.name.clone(),
        addresses: d.addresses().iter().map(ToString::to_string).collect(),
        secret: hex(&secret),
        person,
        role: "custodian".to_owned(),
        device: name.to_owned(),
        expires,
    }))
}

fn equal(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// The inviter's side of a join by `device`.
pub async fn welcome(d: &Daemon, device: &str, join: Join) -> JoinAnswer {
    let refused = |code: &str| JoinAnswer::Refused {
        code: code.to_owned(),
    };
    for g in d.groups() {
        let _busy = g.busy.lock().await;
        let mut records = load(&g);
        let Some(rec) = records
            .invitations
            .iter_mut()
            .find(|r| equal(&r.secret_sha256, &join.invitation_hash))
        else {
            continue;
        };
        let secret = unhex(&rec.secret).unwrap_or_default();
        if !equal(&proof(&secret, device, &d.device), &join.proof) {
            return refused("invite-unknown");
        }
        if rec.used_by.is_some() {
            return refused("invite-used");
        }
        if crate::util::parse_rfc3339(&rec.expires).is_none_or(|t| t < now()) {
            return refused("invite-expired");
        }
        let person = rec.purpose["new-device"]["person"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        rec.used_by = Some(device.to_owned());
        if save(&g, &records).is_err() {
            return refused("not-allowed");
        }
        let Some(mut doc) = g.roster() else {
            return refused("not-in-group");
        };
        let id = DeviceId(device.to_owned());
        let added = Device {
            person: PersonId(person),
            name: join.device_name.clone(),
            role: DeviceRole::Custodian,
            annex_uuid: Some(join.annex_uuid.clone()),
            maxsize: join.capacity,
        };
        let change = [Edit::AddDevice(id.clone(), added)];
        if !may_sign(&doc.roster, &DeviceId(d.device.clone()), &change) {
            return refused("invite-not-allowed");
        }
        let applied = apply(&doc.roster, &change);
        if applied.skipped.is_some() {
            return refused("invite-not-allowed");
        }
        doc.roster = applied.roster;
        let message = format!("pigeon: add {}", join.device_name);
        let commit = match crate::change::commit_roster(&g, &doc, &message, &d.tracer).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(group = %g.id, error = %e, "join-commit-failed");
                return refused("not-allowed");
            }
        };
        let _ = ensure_remote(&g.git, device, &join.annex_uuid).await;
        g.set_addresses(
            device,
            join.addresses
                .iter()
                .filter_map(|a| a.parse().ok())
                .collect(),
        );
        tracing::info!(group = %g.id, peer = %device, commit = %commit, "joined");
        d.wake.notify_one();
        let annex_uuid = g.lock().annex_uuid.clone().unwrap_or_default();
        return JoinAnswer::Welcome { commit, annex_uuid };
    }
    refused("invite-unknown")
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "backup".to_owned())
}

/// The new custodian's side: answers a problem code, or the group's name.
pub async fn join(d: &Daemon, code: &str, folder: Option<String>) -> Result<String, String> {
    let inv = decode(code).ok_or("invite-unknown")?;
    if inv.expires < now() {
        return Err("invite-expired".into());
    }
    if inv.role != "custodian" {
        return Err("not-in-this-build".into());
    }
    if d.group(&inv.group).is_some() {
        return Err("invite-used".into());
    }
    let path = folder.map_or_else(
        || d.dirs.config.join(format!("{}.git", inv.group)),
        PathBuf::from,
    );
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let path = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
    let git = crate::git::Git::new(&path, d.env.clone());
    let fail = |e: crate::git::Error| e.to_string();
    git.run(&["init", "-q", "--bare", "-b", "main"])
        .await
        .map_err(fail)?;
    let short = d.device.get(..6).unwrap_or_default();
    let name = if inv.device.is_empty() {
        format!("{}-{short}", hostname())
    } else {
        inv.device.clone()
    };
    git.run(&["config", "user.name", &name])
        .await
        .map_err(fail)?;
    let email = format!("{}@pigeon", d.device);
    git.run(&["config", "user.email", &email])
        .await
        .map_err(fail)?;
    let pending = Group::open("pending", &path, &d.env)
        .await
        .ok_or("not-a-repository")?;
    crate::settings::check(&pending).await;
    let (uuid, token) = init_annex(&git, pigeon_core::filter::Role::Custodian, &name, inv.k)
        .await
        .map_err(fail)?;
    let secret = unhex(&inv.secret).ok_or("invite-unknown")?;
    let request = Join {
        v: JOIN_VERSION,
        invitation_hash: sha256_hex(&secret),
        proof: proof(&secret, &d.device, &inv.inviter),
        device_name: name,
        role: "custodian".to_owned(),
        annex_uuid: uuid,
        capacity: None,
        addresses: d.addresses().iter().map(ToString::to_string).collect(),
    };
    let addresses: Vec<std::net::SocketAddr> = inv
        .addresses
        .iter()
        .filter_map(|a| a.parse().ok())
        .collect();
    let mut answer = None;
    for _ in 0..30 {
        match d.node.join(&inv.inviter, addresses.clone(), &request).await {
            Ok(a) => {
                answer = Some(a);
                break;
            }
            Err(e) => tracing::info!(error = %e, "inviter-offline"),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let (commit, inviter_uuid) = match answer.ok_or("inviter-offline")? {
        JoinAnswer::Welcome { commit, annex_uuid } => (commit, annex_uuid),
        JoinAnswer::Refused { code } => return Err(code),
    };
    ensure_remote(&git, &inv.inviter, &inviter_uuid)
        .await
        .map_err(fail)?;
    let g = Group::open(&inv.group, &path, &d.env)
        .await
        .ok_or("not-a-repository")?;
    g.set_token(&token);
    g.set_addresses(&inv.inviter, addresses);
    let g = d.add_group(g, &path).await;
    first_fetch(&g, &inv, &commit, &d.device).await?;
    g.reload_roster().await;
    d.wake.notify_one();
    Ok(inv.name)
}

/// Fetches `main` from the inviter, checks that its root is the group's
/// genesis and that it holds the welcome commit, and takes it.
async fn first_fetch(g: &Group, inv: &Invitation, commit: &str, me: &str) -> Result<(), String> {
    let spec = format!("+refs/heads/*:refs/pigeon/peers/{}/*", inv.inviter);
    let args = [
        "fetch",
        "-q",
        "--no-tags",
        "--no-write-fetch-head",
        inv.inviter.as_str(),
        spec.as_str(),
    ];
    // The inviter may still be busy with the welcome's round: retry for a
    // minute, as a P2P fetch can also hang (16 §16.4).
    let mut fetched = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while tokio::time::Instant::now() < deadline {
        match g.git.run_for(&args, Duration::from_secs(30)).await {
            Ok(_) => {
                fetched = true;
                break;
            }
            Err(e) => tracing::warn!(group = %g.id, error = %e, "first-fetch-failed"),
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if !fetched {
        return Err("inviter-offline".into());
    }
    let theirs = g
        .git
        .rev(&format!("refs/pigeon/peers/{}/main", inv.inviter))
        .await
        .ok_or("inviter-offline")?;
    if g.git.root(&theirs).await.as_deref() != Some(inv.group.as_str())
        || !g.git.contains(&theirs, commit).await
    {
        return Err("invite-unknown".into());
    }
    let roster = g.git.blob(&format!("{commit}:.pigeon/roster.toml")).await;
    let ok = roster
        .and_then(|b| crate::roster_file::parse(&String::from_utf8_lossy(&b)).ok())
        .is_some_and(|doc| doc.roster.devices.contains_key(&DeviceId(me.to_owned())));
    if !ok {
        return Err("invite-unknown".into());
    }
    if let Some(ga) = g
        .git
        .rev(&format!("refs/pigeon/peers/{}/git-annex", inv.inviter))
        .await
    {
        let name = format!("refs/remotes/{}/git-annex", inv.inviter);
        let _ = g.git.run(&["update-ref", &name, &ga]).await;
        let _ = g.git.run(&["annex", "merge"]).await;
    }
    g.git
        .update_ref("refs/heads/main", &theirs, None)
        .await
        .map_err(|e| e.to_string())
}
