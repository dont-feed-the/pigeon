//! The group's invitation and joining (09 §9.6, ADR 0075).
//!
//! `pigeon invite`, and `pigeon create` after the genesis, write the group's
//! invitation: the roster's secret, with the devices that admit (the one
//! writing it, then the admins' laptops and backup machines) and their
//! addresses. A new device proves the secret to one of them on
//! `pigeon/join/1` and gives its person's name; the admitter decides the
//! roster change ([`pigeon_core::invitation`]) and signs it if the
//! authority rule lets it (a device of P adds devices of P), else the new
//! device asks the next. It then fetches `main` from the admitter and checks
//! its history ([`crate::join`]). The secret never changes: a refounded
//! group has its own.

use std::collections::BTreeMap;
use std::time::Duration;

use pigeon_core::invitation::{Admission, Arrival, admit};
use pigeon_core::roster::{DeviceId, Role, apply, may_sign};
use pigeon_proto::join::{
    Admitter, INVITATION_VERSION, Invitation, JOIN_VERSION, Join, JoinAnswer, encode,
};

use crate::daemon::Daemon;
use crate::group::Group;
use crate::settings::ensure_remote;
use crate::util::{hex, hmac_sha256, random, sha256_hex, unhex};

/// A new invitation secret: 256 random bits, hex.
pub fn new_secret() -> std::io::Result<String> {
    Ok(hex(&random::<32>()?))
}

/// The invitation of `g`, as `pgn2-…`: its admitters are this device, then
/// the admins' laptops and backup machines, with the addresses this device
/// knows of them.
pub fn code(d: &Daemon, g: &Group) -> Option<String> {
    let r = g.roster()?.roster;
    let known = g.known_addresses(&d.device);
    let mut admitters: Vec<Admitter> = r
        .devices
        .iter()
        .filter_map(|(id, x)| {
            let person = r
                .persons
                .get(&x.person)
                .filter(|p| p.admin || id.0 == d.device)?;
            (x.role != Role::Recovery).then(|| Admitter {
                device: id.0.clone(),
                person: person.name.clone(),
                addresses: if id.0 == d.device {
                    d.addresses().iter().map(ToString::to_string).collect()
                } else {
                    known.get(&id.0).cloned().unwrap_or_default()
                },
            })
        })
        .collect();
    admitters.sort_by_key(|a| a.device != d.device);
    Some(encode(&Invitation {
        v: INVITATION_VERSION,
        group: g.id.clone(),
        name: r.group.clone(),
        k: r.k,
        admitters,
        secret: r.invitation.clone(),
        relay: r.relay.clone(),
        refounded: crate::refound::described(g),
    }))
}

/// The proof of 09 §9.6: HMAC-SHA256(secret, "pigeon-join-1" ‖ new ‖ admitter).
pub fn proof(secret: &[u8], new_device: &str, admitter: &str) -> String {
    let message = format!("pigeon-join-1{new_device}{admitter}");
    hex(&hmac_sha256(secret, message.as_bytes()))
}

fn equal(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// `laptop` or `custodian`, as [`Join::role`] says a role.
pub fn role_name(role: Role) -> &'static str {
    match role {
        Role::Custodian => "custodian",
        Role::Laptop | Role::Recovery => "laptop",
    }
}

/// The admitter's side of a join by `device`.
pub async fn welcome(d: &Daemon, device: &str, join: Join) -> JoinAnswer {
    let refused = |code: &str| JoinAnswer::Refused {
        code: code.to_owned(),
    };
    if let Some(new) = join.new_device.clone() {
        return crate::recovery::welcome(d, device, &new, &join).await;
    }
    for g in d.groups() {
        let _busy = g.busy.lock().await;
        let Some(mut doc) = g.roster() else {
            continue;
        };
        let Some(secret) = unhex(&doc.roster.invitation) else {
            continue;
        };
        if !equal(&sha256_hex(&secret), &join.invitation_hash) {
            continue;
        }
        if !equal(&proof(&secret, device, &d.device), &join.proof) {
            return refused("invite-unknown");
        }
        let role = match join.role.as_str() {
            "laptop" => Role::Laptop,
            "custodian" => Role::Custodian,
            _ => return refused("invite-not-allowed"),
        };
        let id = DeviceId(device.to_owned());
        let arrival = Arrival {
            device: &id,
            person: &join.person,
            existing: join.existing,
            name: &join.device_name,
            role,
            annex_uuid: &join.annex_uuid,
            capacity: join.capacity,
        };
        let commit = match admit(&doc.roster, &arrival) {
            Admission::Refused => return refused("invite-not-allowed"),
            Admission::PersonExists(_) => return refused("person-exists"),
            Admission::Again => match g.git.rev("refs/heads/main").await {
                Some(c) => c,
                None => return refused("not-in-group"),
            },
            Admission::Add(change) => {
                if !may_sign(&doc.roster, &DeviceId(d.device.clone()), &change) {
                    return refused("invite-not-allowed");
                }
                doc.roster = apply(&doc.roster, &change).roster;
                let added = doc
                    .roster
                    .devices
                    .get(&id)
                    .map(|x| x.name.clone())
                    .unwrap_or_default();
                let message = format!("pigeon: add {added}");
                match crate::change::commit_roster(&g, &doc, &message, &d.tracer).await {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(group = %g.id, error = %e, "join-commit-failed");
                        return refused("not-allowed");
                    }
                }
            }
        };
        let _ = ensure_remote(&g.git, device, &join.annex_uuid).await;
        g.set_addresses(device, parse(&join.addresses));
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

fn parse(addresses: &[String]) -> Vec<std::net::SocketAddr> {
    addresses.iter().filter_map(|a| a.parse().ok()).collect()
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "backup".to_owned())
}

/// The name of this computer's account, which a person who gives none
/// is called: `$USER`, else `Me`.
pub fn account() -> String {
    std::env::var("USER")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "Me".to_owned())
}

/// What a joining device asks.
#[derive(Clone, Debug)]
pub struct Joining {
    /// The group's invitation.
    pub inv: Invitation,
    /// Its person's name.
    pub person: String,
    /// Whether it may join the person of the group with that name.
    pub existing: bool,
    /// A laptop or a backup machine.
    pub role: Role,
    /// Its name in the roster.
    pub name: String,
}

/// What `pigeon join` asks for (10 §10.1).
#[derive(Clone, Debug, Default)]
pub struct Ask {
    /// The `pgn2-` invitation.
    pub code: String,
    /// Where the folder goes; none on a backup machine.
    pub folder: Option<String>,
    /// This computer's name in the roster.
    pub name: Option<String>,
    /// Its person's name.
    pub person: Option<String>,
    /// Whether it may join the person of the group with that name.
    pub existing: bool,
    /// As a backup machine.
    pub custodian: bool,
}

/// The new device's side: answers a problem code, or the group's name.
pub async fn join(d: &Daemon, ask: Ask) -> Result<String, String> {
    let inv = pigeon_proto::join::decode(&ask.code).ok_or("invite-unknown")?;
    if d.group(&inv.group).is_some() {
        return Err("already-in-group".into());
    }
    if let Some(relay) = &inv.relay {
        let mut wanted = d.group_relays();
        wanted.push(relay.clone());
        d.use_relays(&wanted).await;
    }
    let given = |v: Option<String>| v.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
    let short = d.device.get(..6).unwrap_or_default();
    let j = Joining {
        person: given(ask.person).unwrap_or_else(account),
        existing: ask.existing,
        role: if ask.custodian {
            Role::Custodian
        } else {
            Role::Laptop
        },
        name: given(ask.name).unwrap_or_else(|| format!("{}-{short}", hostname())),
        inv,
    };
    crate::join::join(d, &j, ask.folder).await
}

/// The `Join` a new device sends to `admitter`, with its annex UUID.
pub fn request(d: &Daemon, j: &Joining, admitter: &str, uuid: &str) -> Result<Join, String> {
    let secret = unhex(&j.inv.secret).ok_or("invite-unknown")?;
    Ok(Join {
        v: JOIN_VERSION,
        traceparent: None,
        invitation_hash: sha256_hex(&secret),
        proof: proof(&secret, &d.device, admitter),
        person: j.person.clone(),
        existing: j.existing,
        device_name: j.name.clone(),
        role: role_name(j.role).to_owned(),
        annex_uuid: uuid.to_owned(),
        capacity: None,
        addresses: d.addresses().iter().map(ToString::to_string).collect(),
        new_device: None,
        recovery_commit: None,
        recovery_addresses: Vec::new(),
        recovery_version: None,
    })
}

/// What a welcome gives: who admitted, the commit adding the device, the
/// admitter's annex UUID, and the direct addresses of the peers it knows.
#[derive(Clone, Debug)]
pub struct Welcomed {
    /// The device that admitted.
    pub admitter: Admitter,
    /// The roster commit that adds the device.
    pub commit: String,
    /// The admitter's git-annex UUID.
    pub annex_uuid: String,
    /// The group's other devices' addresses, by device ID.
    pub peers: BTreeMap<String, Vec<String>>,
}

/// How long a join waits for an admitter at most (09 §9.6 step 1): they
/// may be offline, or busy with a long round, as a welcome waits for the
/// group.
pub const DIAL_FOR: Duration = Duration::from_secs(600);

/// Sends the join of `j`, made with `uuid`, to each admitter in turn, until
/// one welcomes it or [`DIAL_FOR`] passed. A refusal answers its code, but
/// `invite-not-allowed`, which says what that admitter may sign, only once
/// every admitter answered.
pub async fn dial(d: &Daemon, j: &Joining, uuid: &str) -> Result<Welcomed, String> {
    let until = tokio::time::Instant::now() + DIAL_FOR;
    while tokio::time::Instant::now() < until {
        let mut offline = false;
        for admitter in &j.inv.admitters {
            let request = request(d, j, &admitter.device, uuid)?;
            let answer = d
                .node
                .join(&admitter.device, parse(&admitter.addresses), &request)
                .await;
            match answer {
                Ok(JoinAnswer::Welcome {
                    commit,
                    annex_uuid,
                    peers,
                }) => {
                    return Ok(Welcomed {
                        admitter: admitter.clone(),
                        commit,
                        annex_uuid,
                        peers,
                    });
                }
                Ok(JoinAnswer::Refused { code }) if code == "invite-not-allowed" => {}
                Ok(JoinAnswer::Refused { code }) => return Err(code),
                Err(e) => {
                    offline = true;
                    tracing::info!(admitter = %admitter.device, error = %e, "admitter-offline");
                }
            }
        }
        if !offline {
            return Err("invite-not-allowed".into());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err("inviter-offline".into())
}

/// The addresses of `admitter`, as the invitation gives them.
pub fn admitter_addresses(admitter: &Admitter) -> Vec<std::net::SocketAddr> {
    parse(&admitter.addresses)
}

/// Fetches `main` from `admitter` for a new device `me`, checks that its
/// root is `group`'s genesis, that it holds the welcome `commit` and that
/// that commit's roster has `me`, merges the git-annex branch, and answers
/// the admitter's `main`.
pub async fn fetch_welcome(
    g: &Group,
    admitter: &str,
    group: &str,
    commit: &str,
    me: &str,
) -> Result<String, String> {
    let spec = crate::settings::peer_refspec(admitter);
    let archive = crate::refound::refspec(admitter);
    let proposals = crate::proposal::refspec(admitter);
    let args = [
        "fetch",
        "-q",
        "--no-tags",
        "--no-write-fetch-head",
        admitter,
        spec.as_str(),
        archive.as_str(),
        proposals.as_str(),
    ];
    // The admitter may still be busy with the welcome's round: retry for
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
        .rev(&format!("refs/pigeon/peers/{admitter}/main"))
        .await
        .ok_or("inviter-offline")?;
    // Decisions on main refer to separately retained signed proposals.
    // Keep only valid ones before judging their decisions. Dependencies
    // between successive decisions can require another pass; every pass
    // must add a ref, so this terminates when no more can be verified.
    tokio::time::timeout(Duration::from_secs(120), async {
        while crate::proposal::take(g, admitter).await > 0 {}
    })
    .await
    .map_err(|_| "join-history-unverified".to_owned())?;
    crate::verify::bootstrap(g, group, &theirs, commit, me).await?;
    if let Some(ga) = g
        .git
        .rev(&format!("refs/pigeon/peers/{admitter}/git-annex"))
        .await
    {
        let name = format!("refs/remotes/{admitter}/git-annex");
        let _ = g.git.run(&["update-ref", &name, &ga]).await;
        let _ = g.git.run(&["annex", "merge"]).await;
    }
    Ok(theirs)
}
