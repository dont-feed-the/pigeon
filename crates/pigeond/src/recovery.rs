//! Recovery sheets, and recovering a group onto a fresh laptop with one
//! (09 §9.9, ADR 0047).
//!
//! `recovery-sheet` (an admin's laptop) makes an ed25519 key, adds it to
//! the roster as a device of the admin with `role = "recovery"`, and
//! prints the sheet: the group, the person, K, the key and the backup
//! machines with their addresses, as text and as one `pgr1-…` code with a
//! checksum, once a backup machine has that commit. No computer keeps the
//! key. `recover SHEET FOLDER` (a fresh
//! machine) dials a backup machine of the sheet with the sheet's key and
//! joins the machine as a laptop of the admin on `pigeon/join/1`; the
//! backup machine checks that the recovery device may add it (04 §4.3)
//! and writes the roster commit, as an inviter does: in this build commits
//! are not signed (13 §13.12). The machine then takes `main` from it,
//! checked from the genesis the sheet names, hides every file, and the
//! loops bring them back like a kept folder (08 §8.5).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use data_encoding::BASE32_NOPAD;
use pigeon_core::roster::{Device, DeviceId, Edit, PersonId, Role as DeviceRole, apply, may_sign};
use pigeon_proto::Response;
use pigeon_proto::api::Arg;
use pigeon_proto::join::{JOIN_VERSION, Join, JoinAnswer};
use serde::{Deserialize, Serialize};

use crate::api::problem;
use crate::daemon::Daemon;
use crate::group::Group;
use crate::settings::{ensure_remote, init_annex};
use crate::status::msg;
use crate::util::{hex, random, sha256_hex, unhex};

/// What a sheet carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sheet {
    /// Its version, 1.
    pub v: u32,
    /// The group ID: its genesis commit.
    pub group: String,
    /// The group's name.
    pub name: String,
    /// K.
    pub k: u32,
    /// The admin whose device the sheet is.
    pub person: String,
    /// The recovery device's ed25519 secret key, hex.
    pub key: String,
    /// The backup machines when it was printed.
    pub custodians: Vec<SheetCustodian>,
}

/// A backup machine named on a sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetCustodian {
    /// Its device ID.
    pub device: String,
    /// Its name in the roster.
    pub name: String,
    /// Its direct addresses when the sheet was printed.
    pub addresses: Vec<String>,
}

/// Writes a sheet as `pgr1-<base32 JSON>-<checksum>`.
pub fn encode(sheet: &Sheet) -> String {
    let json = serde_json::to_vec(sheet).unwrap_or_default();
    let body = BASE32_NOPAD.encode(&json).to_ascii_lowercase();
    let sum = sha256_hex(body.as_bytes());
    format!("pgr1-{body}-{}", sum.get(..8).unwrap_or_default())
}

/// Reads the first `pgr1-…` code in `text`, if its checksum holds.
pub fn decode(text: &str) -> Option<Sheet> {
    let code = text.split_whitespace().find(|w| w.starts_with("pgr1-"))?;
    let (body, sum) = code.strip_prefix("pgr1-")?.rsplit_once('-')?;
    if sha256_hex(body.as_bytes()).get(..8) != Some(sum) {
        return None;
    }
    let bytes = BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn text(s: &str) -> Arg {
    Arg::Text(s.to_owned())
}

/// `pigeon recovery-sheet`, on an admin's laptop.
pub async fn sheet(d: &Daemon) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let busy = g.busy.lock().await;
    let Some(mut doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let me = DeviceId(d.device.clone());
    let Some(person) = doc.roster.devices.get(&me).map(|x| x.person.clone()) else {
        return vec![problem("not-in-group", &[], None)];
    };
    if !doc.roster.persons.get(&person).is_some_and(|p| p.admin) {
        return vec![problem("not-allowed", &[], None)];
    }
    let Ok(secret) = random::<32>() else {
        return vec![problem("internal-error", &[], None)];
    };
    let id = crate::net::device_id(&iroh::SecretKey::from_bytes(&secret).public());
    let added = Device {
        person: person.clone(),
        name: format!("recovery sheet {}", crate::util::date(crate::util::now())),
        role: DeviceRole::Recovery,
        annex_uuid: None,
        maxsize: None,
    };
    let change = [Edit::AddDevice(DeviceId(id), added)];
    if !may_sign(&doc.roster, &me, &change) {
        return vec![problem("not-allowed", &[], None)];
    }
    doc.roster = apply(&doc.roster, &change).roster;
    let message = format!("pigeon: add a recovery sheet of {}", person.0);
    let commit = match crate::change::commit_roster(&g, &doc, &message, &d.tracer).await {
        Ok(c) => c,
        Err(e) => return vec![problem("internal-error", &[], Some(e))],
    };
    drop(busy);
    if !reached_custodian(d, &g, &commit).await {
        return vec![problem("peer-unreachable", &[], None)];
    }
    let addresses = g.lock().addresses.clone();
    let custodians: Vec<SheetCustodian> = doc
        .roster
        .devices
        .iter()
        .filter(|(_, x)| x.role == DeviceRole::Custodian)
        .map(|(id, x)| SheetCustodian {
            device: id.0.clone(),
            name: x.name.clone(),
            addresses: addresses
                .get(&id.0)
                .map(|a| a.iter().map(ToString::to_string).collect())
                .unwrap_or_default(),
        })
        .collect();
    let sheet = Sheet {
        v: 1,
        group: g.id.clone(),
        name: doc.roster.group.clone(),
        k: doc.roster.k,
        person: person.0.clone(),
        key: hex(&secret),
        custodians,
    };
    vec![Response::Said {
        message: msg("recovery-sheet-text", &[("sheet", text(&render(&sheet)))]),
    }]
}

/// Waits up to two minutes for a backup machine to hold `commit`, as this
/// device sees its `main`: a sheet whose key no backup machine knows yet
/// would recover nothing if this computer were lost now.
async fn reached_custodian(d: &Daemon, g: &Group, commit: &str) -> bool {
    let custodians: Vec<String> = g
        .roster()
        .map(|doc| {
            doc.roster
                .devices
                .iter()
                .filter(|(_, x)| x.role == DeviceRole::Custodian)
                .map(|(id, _)| id.0.clone())
                .collect()
        })
        .unwrap_or_default();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    let mut n = 0u32;
    while tokio::time::Instant::now() < deadline {
        if n.is_multiple_of(5) {
            d.wake.notify_one();
        }
        n += 1;
        for c in &custodians {
            if let Some(theirs) = g.git.rev(&format!("refs/pigeon/peers/{c}/main")).await
                && g.git.contains(&theirs, commit).await
            {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    false
}

/// The sheet as a person reads it.
fn render(s: &Sheet) -> String {
    let mut out = format!(
        "pigeon recovery sheet\nGroup: {} ({})\nPerson: {}\nBackup machines:\n",
        s.name, s.group, s.person
    );
    for c in &s.custodians {
        out.push_str(&format!("  {} {}\n", c.name, c.device));
    }
    out.push_str(
        "Keep this sheet like a passport: whoever holds it can add computers for you.\n\
         On a new computer: pigeon recover THIS-FILE FOLDER\n",
    );
    out.push_str(&encode(s));
    out.push('\n');
    out
}

/// A backup machine's side of a join that a recovery device `device`
/// sends for the new machine `new` of its person.
pub async fn welcome(d: &Daemon, device: &str, new: &str, join: &Join) -> JoinAnswer {
    let refused = |code: &str| JoinAnswer::Refused {
        code: code.to_owned(),
    };
    for g in d.groups() {
        let _busy = g.busy.lock().await;
        let Some(mut doc) = g.roster() else {
            continue;
        };
        let signer = DeviceId(device.to_owned());
        let Some(r) = doc.roster.devices.get(&signer) else {
            continue;
        };
        if r.role != DeviceRole::Recovery {
            return refused("invite-not-allowed");
        }
        let added = Device {
            person: PersonId(r.person.0.clone()),
            name: join.device_name.clone(),
            role: DeviceRole::Laptop,
            annex_uuid: Some(join.annex_uuid.clone()),
            maxsize: None,
        };
        let change = [Edit::AddDevice(DeviceId(new.to_owned()), added)];
        if !may_sign(&doc.roster, &signer, &change) {
            return refused("invite-not-allowed");
        }
        let applied = apply(&doc.roster, &change);
        if applied.skipped.is_some() {
            return refused("invite-not-allowed");
        }
        doc.roster = applied.roster;
        let message = format!("pigeon: add {} with a recovery sheet", join.device_name);
        let commit = match crate::change::commit_roster(&g, &doc, &message, &d.tracer).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(group = %g.id, error = %e, "recover-commit-failed");
                return refused("not-allowed");
            }
        };
        let _ = ensure_remote(&g.git, new, &join.annex_uuid).await;
        let addresses = join
            .addresses
            .iter()
            .filter_map(|a| a.parse().ok())
            .collect();
        g.set_addresses(new, addresses);
        tracing::info!(group = %g.id, recovery = %device, peer = %new, commit = %commit, "recovered-device");
        d.wake.notify_one();
        let annex_uuid = g.lock().annex_uuid.clone().unwrap_or_default();
        let peers = g.known_addresses(new);
        return JoinAnswer::Welcome {
            commit,
            annex_uuid,
            peers,
        };
    }
    refused("invite-unknown")
}

/// `pigeon recover SHEET [FOLDER] [--from NAME]`, on a fresh machine.
pub async fn recover(
    d: &Daemon,
    sheet: &str,
    folder: Option<String>,
    from: Option<String>,
) -> Vec<Response> {
    match recover_inner(d, sheet, folder, from).await {
        Ok(name) => vec![Response::Said {
            message: msg("recovered", &[("group", text(&name))]),
        }],
        Err(code) if code.contains(' ') => {
            tracing::warn!(error = %code, "recover-failed");
            vec![problem("internal-error", &[], Some(code))]
        }
        Err(code) => vec![problem(&code, &[], None)],
    }
}

async fn recover_inner(
    d: &Daemon,
    sheet: &str,
    folder: Option<String>,
    from: Option<String>,
) -> Result<String, String> {
    let s = decode(sheet).ok_or("sheet-unreadable")?;
    let secret: [u8; 32] = unhex(&s.key)
        .and_then(|k| k.try_into().ok())
        .ok_or("sheet-unreadable")?;
    if d.group(&s.group).is_some() {
        return Err("invite-used".into());
    }
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    let path = folder.map_or_else(|| home.join(&s.name), PathBuf::from);
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let path = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
    if std::fs::read_dir(&path)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("the folder is not empty".into());
    }
    let git = crate::git::Git::new(&path, d.env.clone());
    let fail = |e: crate::git::Error| e.to_string();
    git.run(&["init", "-q", "-b", "main"]).await.map_err(fail)?;
    let name = format!("laptop-{}", d.device.get(..6).unwrap_or_default());
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
    let (uuid, token) = init_annex(&git, pigeon_core::filter::Role::Laptop, &name, s.k)
        .await
        .map_err(fail)?;
    let k = s.k.to_string();
    git.run(&["annex", "numcopies", &k]).await.map_err(fail)?;
    git.run(&["annex", "mincopies", &k]).await.map_err(fail)?;
    drop(pending);
    let request = Join {
        v: JOIN_VERSION,
        invitation_hash: String::new(),
        proof: String::new(),
        device_name: name,
        role: "laptop".to_owned(),
        annex_uuid: uuid,
        capacity: None,
        addresses: d.addresses().iter().map(ToString::to_string).collect(),
        new_device: Some(d.device.clone()),
    };
    let (custodian, commit, their_uuid, peers) =
        dial(d, &s, &secret, from.as_deref(), &request).await?;
    ensure_remote(&git, &custodian.device, &their_uuid)
        .await
        .map_err(fail)?;
    let g = Group::open(&s.group, &path, &d.env)
        .await
        .ok_or("not-a-repository")?;
    g.set_token(&token);
    for c in &s.custodians {
        g.set_addresses(&c.device, parse(&c.addresses));
    }
    g.learn_peers(&peers);
    let g = d.add_group(g, &path).await;
    // No round may capture before every annexed file is hidden: a file of
    // the index missing from the folder would read as a deletion (S10).
    let busy = g.busy.lock().await;
    let theirs =
        crate::invite::fetch_welcome(&g, &custodian.device, &s.group, &commit, &d.device).await?;
    g.git
        .update_ref("refs/heads/main", &theirs, None)
        .await
        .map_err(|e| e.to_string())?;
    d.tracer
        .event(crate::trace::Act::Merge, &[("commit", &theirs)]);
    g.reload_roster().await;
    hide_all(&g, &theirs).await?;
    drop(busy);
    d.wake.notify_one();
    Ok(s.name)
}

/// Direct addresses by device ID, as a welcome gives them.
type Peers = std::collections::BTreeMap<String, Vec<String>>;

fn parse(addresses: &[String]) -> Vec<SocketAddr> {
    addresses.iter().filter_map(|a| a.parse().ok()).collect()
}

/// Joins with the sheet's key through the first backup machine of the
/// sheet (or `from`) that welcomes it, trying them for two minutes.
async fn dial(
    d: &Daemon,
    s: &Sheet,
    secret: &[u8; 32],
    from: Option<&str>,
    request: &Join,
) -> Result<(SheetCustodian, String, String, Peers), String> {
    let mut bind: SocketAddr = d
        .config
        .network
        .bind
        .parse()
        .map_err(|e| format!("[network] bind: {e}"))?;
    bind.set_port(0);
    let node = crate::net::Node::bind(&crate::net::NetConfig {
        secret_key: *secret,
        bind,
        mdns: false,
    })
    .await
    .map_err(|e| e.to_string())?;
    let targets: Vec<&SheetCustodian> = s
        .custodians
        .iter()
        .filter(|c| from.is_none_or(|f| c.name == f || c.device == f))
        .collect();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    let mut last = "inviter-offline".to_owned();
    while tokio::time::Instant::now() < deadline {
        for c in &targets {
            match node.join(&c.device, parse(&c.addresses), request).await {
                Ok(JoinAnswer::Welcome {
                    commit,
                    annex_uuid,
                    peers,
                }) => {
                    node.close().await;
                    return Ok(((*c).clone(), commit, annex_uuid, peers));
                }
                Ok(JoinAnswer::Refused { code }) => last = code,
                Err(e) => tracing::info!(peer = %c.device, error = %e, "recover-dial-failed"),
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    node.close().await;
    Err(last)
}

/// Takes `main`'s tree into the index with every annexed file hidden and
/// writes pigeon's own files, so that nothing reads as deleted and keep
/// brings the files back (08 §8.5).
pub(crate) async fn hide_all(g: &Group, main: &str) -> Result<(), String> {
    let wt = g.worktree.clone().ok_or("not-in-group")?;
    g.git
        .run(&["read-tree", main])
        .await
        .map_err(|e| e.to_string())?;
    let files = g.git.tree_files(main).await.map_err(|e| e.to_string())?;
    let mut hidden = std::collections::BTreeSet::new();
    for f in files {
        let blob = g
            .git
            .blob(&format!("{main}:{}", f.path))
            .await
            .unwrap_or_default();
        if crate::git::pointer_key(&blob).is_some() {
            hidden.insert(f.path);
            continue;
        }
        let file = wt.join(&f.path);
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        pigeon_sys::fs::write_atomic(&file, &blob).map_err(|e| e.to_string())?;
    }
    crate::evict::write_sparse(g, &hidden);
    for chunk in hidden.iter().collect::<Vec<_>>().chunks(64) {
        let mut args = vec!["update-index", "--skip-worktree", "--"];
        args.extend(chunk.iter().map(|p| p.as_str()));
        g.git.run(&args).await.map_err(|e| e.to_string())?;
    }
    let _ = g.git.run(&["update-index", "-q", "--refresh"]).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Sheet, SheetCustodian, decode, encode};

    #[test]
    fn a_sheet_reads_back_and_a_changed_one_does_not() {
        let s = Sheet {
            v: 1,
            group: "abc".into(),
            name: "Family".into(),
            k: 2,
            person: "ines".into(),
            key: "00".repeat(32),
            custodians: vec![SheetCustodian {
                device: "dd".into(),
                name: "nas".into(),
                addresses: vec!["127.0.0.1:1".into()],
            }],
        };
        let code = encode(&s);
        let text = format!("Some words\n{code}\nmore");
        assert_eq!(decode(&text), Some(s));
        let mut typo: Vec<char> = code.chars().collect();
        typo[9] = if typo[9] == 'a' { 'b' } else { 'a' };
        let typo: String = typo.into_iter().collect();
        assert_eq!(decode(&typo), None);
    }
}
