//! Invitations and joining (09 §9.6).
//!
//! `invite PERSON` writes a one-time invitation for a new person's laptop,
//! or for a new laptop of a person of the roster; `invite --backup-machine`
//! one for a backup machine of the inviter's person. The joining device
//! sends `Join` on `pigeon/join/1`; the inviter checks the proof and signs
//! the roster commit that adds it, and the new device fetches `main` from
//! the inviter and checks its genesis ([`crate::join`] for a laptop). The
//! inviter keeps each invitation's secret in `invitations.json`, mode
//! 0600, since it checks the HMAC keyed by it (13 §13.12 corrects 09 §9.6,
//! which kept only its hash).

use std::path::PathBuf;
use std::time::Duration;

use data_encoding::BASE32_NOPAD;
use pigeon_core::roster::{
    Device, DeviceId, Edit, Person, PersonId, Role as DeviceRole, Roster, apply, may_sign,
};
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
    /// The person the new device belongs to: their ID.
    pub person: String,
    /// A new person's name, when the invitation adds them; empty for a
    /// person of the roster.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub new_person: String,
    /// Whether the new person is an admin.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub admin: bool,
    /// `custodian` or `laptop`.
    pub role: String,
    /// The new device's name in the roster, if the inviter chose one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
    /// When it stops working, in Unix seconds.
    pub expires: i64,
    /// The group's own relay, which the new device uses to reach the
    /// inviter (09 §9.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
    /// For a refounded group, what the joiner accepts (ADR 0047).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refounded: Option<pigeon_proto::join::Refounded>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<String>,
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
    crate::util::write_private(&g.files.invitations(), &text)?;
    pigeon_sys::fs::fsync_dir(&g.root()).map_err(std::io::Error::other)
}

/// `pigeon invite --list`: the invitations neither used nor expired.
pub fn pending(g: &Group) -> Vec<pigeon_proto::api::InvitationEntry> {
    let t = now();
    load(g)
        .invitations
        .into_iter()
        .filter(|r| r.group.as_deref() == Some(g.id.as_str()) && r.used_by.is_none())
        .filter(|r| pigeon_proto::time::parse_rfc3339(&r.expires).is_some_and(|e| e > t))
        .map(|r| {
            let field = |kind: &str, key: &str| {
                r.purpose
                    .get(kind)
                    .and_then(|p| p.get(key))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            };
            let person = field("new-device", "person").and_then(|p| {
                g.roster()
                    .and_then(|d| d.roster.persons.get(&PersonId(p.clone())).cloned())
                    .map_or(Some(p), |x| Some(x.name))
            });
            pigeon_proto::api::InvitationEntry {
                id: r.id.clone(),
                who: field("new-person", "name").or(person).unwrap_or_default(),
                custodian: field("new-device", "role").as_deref() == Some("custodian"),
                expires: r.expires.clone(),
            }
        })
        .collect()
}

/// `pigeon invite --cancel ID`: forgets an invitation not yet used, so that
/// joining with it fails with `invite-unknown`. False if there is none.
pub fn cancel(g: &Group, id: &str) -> bool {
    let mut records = load(g);
    let before = records.invitations.len();
    records
        .invitations
        .retain(|r| r.id != id || r.used_by.is_some());
    records.invitations.len() < before && save(g, &records).is_ok()
}

/// What `pigeon invite` asks for (10 §10.1).
#[derive(Clone, Copy, Debug, Default)]
pub struct Ask<'a> {
    /// The person, by ID or name; empty for the inviter's own person.
    pub person: &'a str,
    /// A new person as an admin.
    pub admin: bool,
    /// A backup machine rather than a laptop.
    pub custodian: bool,
    /// The new device's name in the roster, if the inviter chose one.
    pub name: &'a str,
}

/// The person of `roster` that `who` names, by ID or by name, ignoring case.
pub fn find_person(roster: &Roster, who: &str) -> Option<PersonId> {
    let id = PersonId(who.to_ascii_lowercase());
    if roster.persons.contains_key(&id) {
        return Some(id);
    }
    roster
        .persons
        .iter()
        .find(|(_, p)| p.name.to_lowercase() == who.to_lowercase())
        .map(|(id, _)| id.clone())
}

/// A person ID for a new person named `name`: its ASCII letters and
/// digits, lowercase, with a number if another person has it (04 §4.2).
pub fn new_person_id(roster: &Roster, name: &str) -> PersonId {
    let mut base: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if base.is_empty() {
        base = "person".to_owned();
    }
    let mut id = base.clone();
    let mut n = 2;
    while roster.persons.contains_key(&PersonId(id.clone())) {
        id = format!("{base}{n}");
        n += 1;
    }
    PersonId(id)
}

/// Makes an invitation in `g` (09 §9.6): for a new laptop of a person of
/// the roster, from an admin or one of their devices; for a new person,
/// from an admin; for a backup machine of the inviter's person (or the
/// person named), named `ask.name` in the roster if not empty.
pub fn invite(d: &Daemon, g: &Group, ask: Ask<'_>) -> Result<String, &'static str> {
    let doc = g.roster().ok_or("not-in-group")?;
    let me = doc
        .roster
        .devices
        .get(&DeviceId(d.device.clone()))
        .ok_or("not-in-group")?;
    let admin = me.role != DeviceRole::Recovery
        && doc.roster.persons.get(&me.person).is_some_and(|p| p.admin);
    let who = ask.person.trim();
    let found = if who.is_empty() {
        Some(me.person.clone())
    } else {
        find_person(&doc.roster, who)
    };
    let (person, new_person) = match found {
        Some(p) if !ask.admin && (admin || p == me.person) => (p, None),
        None if !ask.custodian && admin => (
            new_person_id(&doc.roster, who),
            Some(Person {
                name: who.to_owned(),
                admin: ask.admin,
            }),
        ),
        _ => return Err("invite-not-allowed"),
    };
    let name = ask.name.trim();
    if !name.is_empty() && doc.roster.devices.values().any(|x| x.name == name) {
        return Err("invite-not-allowed");
    }
    let role = if ask.custodian { "custodian" } else { "laptop" };
    let secret = random::<32>().map_err(|_| "not-allowed")?;
    let expires = now() + LIFETIME;
    let purpose = match &new_person {
        Some(p) => serde_json::json!({"new-person": {
            "id": person.0, "name": p.name, "admin": p.admin}}),
        None => serde_json::json!({"new-device": {
            "person": person.0, "role": role, "name": name}}),
    };
    let mut records = load(g);
    records.invitations.push(Record {
        group: Some(g.id.clone()),
        id: format!("inv-{}", hex(secret.get(..2).unwrap_or_default())),
        secret: hex(&secret),
        secret_sha256: sha256_hex(&secret),
        purpose,
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
        person: person.0,
        new_person: new_person
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default(),
        admin: new_person.as_ref().is_some_and(|p| p.admin),
        role: role.to_owned(),
        device: name.to_owned(),
        expires,
        relay: doc.roster.relay.clone(),
        refounded: crate::refound::described(g),
    }))
}

/// The roster edits a used invitation's `purpose` makes for the device
/// `id` that sent `join`: `None` if the join asks for another role.
fn edits(
    purpose: &serde_json::Value,
    id: &DeviceId,
    join: &Join,
    roster: &Roster,
) -> Option<Vec<Edit>> {
    let text = |v: &serde_json::Value| v.as_str().map(str::to_owned);
    let mut name = join.device_name.clone();
    if roster.devices.values().any(|x| x.name == name) {
        name = format!("{name}-{}", id.0.get(..6).unwrap_or_default());
    }
    let device = |person: String, role: DeviceRole| Device {
        person: PersonId(person),
        name: name.clone(),
        role,
        annex_uuid: Some(join.annex_uuid.clone()),
        maxsize: if role == DeviceRole::Custodian {
            join.capacity
        } else {
            None
        },
    };
    if let Some(p) = purpose.get("new-person") {
        if join.role != "laptop" {
            return None;
        }
        let person = text(&p["id"])?;
        let added = Person {
            name: text(&p["name"])?,
            admin: p["admin"].as_bool().unwrap_or(false),
        };
        return Some(vec![
            Edit::AddPerson(PersonId(person.clone()), added),
            Edit::AddDevice(id.clone(), device(person, DeviceRole::Laptop)),
        ]);
    }
    let p = purpose.get("new-device")?;
    let role = match p["role"].as_str()? {
        "custodian" => DeviceRole::Custodian,
        "laptop" => DeviceRole::Laptop,
        _ => return None,
    };
    if join.role != p["role"].as_str()? {
        return None;
    }
    Some(vec![Edit::AddDevice(
        id.clone(),
        device(text(&p["person"])?, role),
    )])
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
    if let Some(new) = join.new_device.clone() {
        return crate::recovery::welcome(d, device, &new, &join).await;
    }
    for g in d.groups() {
        let _busy = g.busy.lock().await;
        let mut records = load(&g);
        let Some(rec) = records.invitations.iter_mut().find(|r| {
            r.group.as_deref() == Some(g.id.as_str())
                && equal(&r.secret_sha256, &join.invitation_hash)
        }) else {
            continue;
        };
        let secret = unhex(&rec.secret).unwrap_or_default();
        if !equal(&proof(&secret, device, &d.device), &join.proof) {
            return refused("invite-unknown");
        }
        use pigeon_core::invitation::{Redemption, redeem};
        let redemption = redeem(rec.group.as_ref(), &g.id, rec.used_by.as_deref(), device);
        if redemption == Redemption::Refused {
            return refused("invite-used");
        }
        if crate::util::parse_rfc3339(&rec.expires).is_none_or(|t| t < now()) {
            return refused("invite-expired");
        }
        if redemption == Redemption::Resume {
            let id = DeviceId(device.to_owned());
            let Some(doc) = g.roster() else {
                return refused("not-in-group");
            };
            if let Some(existing) = doc.roster.devices.get(&id) {
                let role = match existing.role {
                    DeviceRole::Laptop => "laptop",
                    DeviceRole::Custodian => "custodian",
                    DeviceRole::Recovery => "recovery",
                };
                if role != join.role
                    || existing.annex_uuid.as_deref() != Some(join.annex_uuid.as_str())
                {
                    return refused("invite-not-allowed");
                }
                let Some(commit) = g.git.rev("refs/heads/main").await else {
                    return refused("not-in-group");
                };
                g.set_addresses(
                    device,
                    join.addresses
                        .iter()
                        .filter_map(|a| a.parse().ok())
                        .collect(),
                );
                let annex_uuid = g.lock().annex_uuid.clone().unwrap_or_default();
                let peers = g.known_addresses(device);
                return JoinAnswer::Welcome {
                    commit,
                    annex_uuid,
                    peers,
                };
            }
            if doc.roster.removed.contains_key(&id) {
                return refused("invite-not-allowed");
            }
        }
        let purpose = rec.purpose.clone();
        rec.used_by = Some(device.to_owned());
        if save(&g, &records).is_err() {
            return refused("not-allowed");
        }
        let Some(mut doc) = g.roster() else {
            return refused("not-in-group");
        };
        let id = DeviceId(device.to_owned());
        let Some(change) = edits(&purpose, &id, &join, &doc.roster) else {
            return refused("invite-not-allowed");
        };
        if !may_sign(&doc.roster, &DeviceId(d.device.clone()), &change) {
            return refused("invite-not-allowed");
        }
        let applied = apply(&doc.roster, &change);
        if applied.skipped.is_some() {
            return refused("invite-not-allowed");
        }
        doc.roster = applied.roster;
        let added = doc
            .roster
            .devices
            .get(&id)
            .map(|x| x.name.clone())
            .unwrap_or_default();
        let message = format!("pigeon: add {added}");
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
        let peers = g.known_addresses(device);
        return JoinAnswer::Welcome {
            commit,
            annex_uuid,
            peers,
        };
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

/// The new device's side: answers a problem code, or the group's name.
/// `name`, when given, is the name it asks for, over the invitation's.
pub async fn join(
    d: &Daemon,
    code: &str,
    folder: Option<String>,
    name: Option<String>,
) -> Result<String, String> {
    let mut inv = decode(code).ok_or("invite-unknown")?;
    if let Some(name) = name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()) {
        inv.device = name;
    }
    if inv.expires < now() {
        return Err("invite-expired".into());
    }
    if d.group(&inv.group).is_some() {
        return Err("invite-used".into());
    }
    if let Some(relay) = &inv.relay {
        let mut wanted = d.group_relays();
        wanted.push(relay.clone());
        d.use_relays(&wanted).await;
    }
    match inv.role.as_str() {
        "custodian" => join_backup(d, &inv, folder).await,
        "laptop" => crate::join::laptop(d, &inv, folder).await,
        _ => Err("invite-unknown".into()),
    }
}

/// The name a joining device gives itself: its own choice (`join --name`),
/// else the inviter's, else `<host name>-<6 hex digits of its device ID>`
/// (13 §13.12).
pub fn device_name(d: &Daemon, inv: &Invitation) -> String {
    let short = d.device.get(..6).unwrap_or_default();
    if inv.device.is_empty() {
        format!("{}-{short}", hostname())
    } else {
        inv.device.clone()
    }
}

/// The `Join` a new device sends, for `role`, with its annex UUID.
pub fn request(d: &Daemon, inv: &Invitation, name: &str, uuid: &str) -> Result<Join, String> {
    let secret = unhex(&inv.secret).ok_or("invite-unknown")?;
    Ok(Join {
        v: JOIN_VERSION,
        traceparent: None,
        invitation_hash: sha256_hex(&secret),
        proof: proof(&secret, &d.device, &inv.inviter),
        device_name: name.to_owned(),
        role: inv.role.clone(),
        annex_uuid: uuid.to_owned(),
        capacity: None,
        addresses: d.addresses().iter().map(ToString::to_string).collect(),
        new_device: None,
        recovery_commit: None,
        recovery_addresses: Vec::new(),
        recovery_version: None,
    })
}

/// What a welcome gives: the commit adding the device, the inviter's annex
/// UUID, and the direct addresses of the peers it knows.
pub type Welcomed = (
    String,
    String,
    std::collections::BTreeMap<String, Vec<String>>,
);

/// How long a join waits for the inviter at most, unless the invitation
/// expires first (09 §9.6 step 1): the inviter may be offline, or busy
/// with a long round, as its welcome waits for the group.
pub const DIAL_FOR: Duration = Duration::from_secs(600);

/// Sends `request` to the inviter, retrying while it is offline until the
/// invitation expires or [`DIAL_FOR`] passed; a refusal answers its code.
pub async fn dial(d: &Daemon, inv: &Invitation, request: &Join) -> Result<Welcomed, String> {
    let addresses = inviter_addresses(inv);
    let mut answer = None;
    let until = tokio::time::Instant::now() + DIAL_FOR;
    while tokio::time::Instant::now() < until && crate::util::now() < inv.expires {
        match d.node.join(&inv.inviter, addresses.clone(), request).await {
            Ok(a) => {
                answer = Some(a);
                break;
            }
            Err(e) => tracing::info!(error = %e, "inviter-offline"),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    match answer.ok_or("inviter-offline")? {
        JoinAnswer::Welcome {
            commit,
            annex_uuid,
            peers,
        } => Ok((commit, annex_uuid, peers)),
        JoinAnswer::Refused { code } => Err(code),
    }
}

/// The inviter's direct addresses, as the invitation gives them.
pub fn inviter_addresses(inv: &Invitation) -> Vec<std::net::SocketAddr> {
    inv.addresses
        .iter()
        .filter_map(|a| a.parse().ok())
        .collect()
}

/// A backup machine's join: its bare repository, then the welcome.
async fn join_backup(
    d: &Daemon,
    inv: &Invitation,
    folder: Option<String>,
) -> Result<String, String> {
    let path = folder.map_or_else(
        || d.dirs.config.join(format!("{}.git", inv.group)),
        PathBuf::from,
    );
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let path = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
    if crate::join_in_place::applies(d, inv, &path).await {
        return crate::join_in_place::join(d, inv, &path).await;
    }
    if std::fs::read_dir(&path)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("folder-not-empty".into());
    }
    let git = crate::git::Git::new(&path, d.env.clone());
    let fail = |e: crate::git::Error| e.to_string();
    git.run(&["init", "-q", "--bare", "-b", "main"])
        .await
        .map_err(fail)?;
    let name = device_name(d, inv);
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
    let request = request(d, inv, &name, &uuid)?;
    let (commit, inviter_uuid, peers) = dial(d, inv, &request).await?;
    ensure_remote(&git, &inv.inviter, &inviter_uuid)
        .await
        .map_err(fail)?;
    let g = Group::open(&inv.group, &path, &d.env)
        .await
        .ok_or("not-a-repository")?;
    g.set_token(&token);
    g.set_addresses(&inv.inviter, inviter_addresses(inv));
    g.learn_peers(&peers);
    let g = d.add_group(g, &path).await?;
    first_fetch(&g, inv, &commit, &d.device).await?;
    g.reload_roster().await;
    d.wake.notify_one();
    Ok(inv.name.clone())
}

/// Fetches `main` from the inviter, checks that its root is the group's
/// genesis and that it holds the welcome commit, and takes it.
async fn first_fetch(g: &Group, inv: &Invitation, commit: &str, me: &str) -> Result<(), String> {
    let theirs = fetch_welcome(g, &inv.inviter, &inv.group, commit, me).await?;
    g.git
        .update_ref("refs/heads/main", &theirs, None)
        .await
        .map_err(|e| e.to_string())
}

/// Fetches `main` from `inviter` for a new device `me`, checks that its
/// root is `group`'s genesis, that it holds the welcome `commit` and that
/// that commit's roster has `me`, merges the git-annex branch, and answers
/// the inviter's `main`.
pub async fn fetch_welcome(
    g: &Group,
    inviter: &str,
    group: &str,
    commit: &str,
    me: &str,
) -> Result<String, String> {
    let spec = crate::settings::peer_refspec(inviter);
    let archive = crate::refound::refspec(inviter);
    let proposals = crate::proposal::refspec(inviter);
    let args = [
        "fetch",
        "-q",
        "--no-tags",
        "--no-write-fetch-head",
        inviter,
        spec.as_str(),
        archive.as_str(),
        proposals.as_str(),
    ];
    // The inviter may still be busy with the welcome's round: retry for
    // five minutes, as a P2P fetch can also hang (16 §16.4) and a loaded
    // machine is slow.
    let mut fetched = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
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
        .rev(&format!("refs/pigeon/peers/{inviter}/main"))
        .await
        .ok_or("inviter-offline")?;
    // Decisions on main refer to separately retained signed proposals.
    // Keep only valid ones before judging their decisions. Dependencies
    // between successive decisions can require another pass; every pass
    // must add a ref, so this terminates when no more can be verified.
    tokio::time::timeout(Duration::from_secs(120), async {
        while crate::proposal::take(g, inviter).await > 0 {}
    })
    .await
    .map_err(|_| "join-history-unverified".to_owned())?;
    crate::verify::bootstrap(g, group, &theirs, commit, me).await?;
    if let Some(ga) = g
        .git
        .rev(&format!("refs/pigeon/peers/{inviter}/git-annex"))
        .await
    {
        let name = format!("refs/remotes/{inviter}/git-annex");
        let _ = g.git.run(&["update-ref", &name, &ga]).await;
        let _ = g.git.run(&["annex", "merge"]).await;
    }
    Ok(theirs)
}
