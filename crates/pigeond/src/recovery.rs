//! Recovery sheets, and recovering a group onto a fresh laptop with one
//! (09 §9.9, ADR 0047).
//!
//! `recovery-sheet` (an admin's laptop) makes an ed25519 key, adds it to
//! the roster as a device of the admin with `role = "recovery"`, and
//! prints the sheet: the group, the person, K, the key and the backup
//! machines with their addresses, as text and as one `pgr1-…` code with a
//! checksum, once a backup machine has that commit. No computer keeps the
//! key. `recover SHEET FOLDER` (a fresh
//! machine) validates the custodian's history, signs its own addition with
//! the sheet key, and serves that commit until the custodian pulls and
//! verifies it. The backup machine never signs on the admin's behalf.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use data_encoding::BASE32_NOPAD;
use pigeon_core::roster::{Device, DeviceId, Edit, Role as DeviceRole, apply, may_sign};
use pigeon_proto::Response;
use pigeon_proto::api::Arg;
use pigeon_proto::join::{Join, JoinAnswer};
use serde::{Deserialize, Serialize};

use crate::api::problem;
use crate::daemon::Daemon;
use crate::group::Group;
use crate::settings::init_annex;
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
    /// The sheet's name in the roster, `YYYY-MM-DD-xxxxxx`, as the People
    /// page lists it; empty on a sheet printed before it was shown.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
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

/// Reads the last valid `pgr1-…` code, after any user-supplied labels.
pub fn decode(text: &str) -> Option<Sheet> {
    text.split_whitespace().rev().find_map(decode_code)
}

fn decode_code(code: &str) -> Option<Sheet> {
    let (body, sum) = code.strip_prefix("pgr1-")?.rsplit_once('-')?;
    if sha256_hex(body.as_bytes()).get(..8) != Some(sum) {
        return None;
    }
    let bytes = BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .ok()?;
    let sheet: Sheet = serde_json::from_slice(&bytes).ok()?;
    (sheet.v == 1).then_some(sheet)
}

fn text(s: &str) -> Arg {
    Arg::Text(s.to_owned())
}

/// `pigeon recovery-sheet`, on an admin's laptop. The work runs in its
/// own task: a client that goes away (a closed browser tab) must not stop
/// it between adding the sheet to the roster and revoking it unprinted.
pub async fn sheet(d: &Arc<Daemon>) -> Vec<Response> {
    let d = d.clone();
    tokio::spawn(async move { sheet_inner(&d).await })
        .await
        .unwrap_or_else(|e| vec![problem("internal-error", &[], Some(e.to_string()))])
}

async fn sheet_inner(d: &Daemon) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let me = DeviceId(d.device.clone());
    let Some(doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(person) = doc.roster.devices.get(&me).map(|x| x.person.clone()) else {
        return vec![problem("not-in-group", &[], None)];
    };
    if !doc.roster.persons.get(&person).is_some_and(|p| p.admin) {
        return vec![problem("not-allowed", &[], None)];
    }
    let backups: Vec<(String, String)> = doc
        .roster
        .devices
        .iter()
        .filter(|(_, x)| x.role == DeviceRole::Custodian)
        .map(|(id, x)| (id.0.clone(), x.name.clone()))
        .collect();
    if backups.is_empty() {
        return vec![problem("recovery-needs-backup", &[], None)];
    }
    // Nothing is signed until a backup machine answers: a sheet that no
    // backup could receive would only be revoked again (09 §9.9).
    let ids: Vec<String> = backups.iter().map(|(id, _)| id.clone()).collect();
    if !crate::custody::any_reachable(&g, &d.device, &ids).await {
        let names: Vec<&str> = backups.iter().map(|(_, n)| n.as_str()).collect();
        return vec![problem(
            "recovery-backup-offline",
            &[("machines", text(&names.join(", ")))],
            None,
        )];
    }
    let busy = g.busy.lock().await;
    let Some(mut doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Ok(secret) = random::<32>() else {
        return vec![problem("internal-error", &[], None)];
    };
    let id = crate::net::device_id(&iroh::SecretKey::from_bytes(&secret).public());
    let label = format!(
        "{}-{}",
        crate::util::date(crate::util::now()),
        id.get(..6).unwrap_or_default()
    );
    let added = Device {
        person: person.clone(),
        name: label.clone(),
        role: DeviceRole::Recovery,
        annex_uuid: None,
        maxsize: None,
    };
    let change = [Edit::AddDevice(DeviceId(id.clone()), added)];
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
        // The key was never shown. Revoke its entry rather than making the
        // People page count a sheet nobody could have kept.
        let _busy = g.busy.lock().await;
        if let Some(mut current) = g.roster() {
            let change = [Edit::RemoveDevice(DeviceId(id))];
            if may_sign(&current.roster, &me, &change) {
                current.roster = apply(&current.roster, &change).roster;
                if let Err(e) = crate::change::commit_roster(
                    &g,
                    &current,
                    "pigeon: revoke an unprinted recovery sheet",
                    &d.tracer,
                )
                .await
                {
                    return vec![problem("internal-error", &[], Some(e))];
                }
            }
        }
        return vec![problem("recovery-not-received", &[], None)];
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
        label,
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
        "pigeon recovery sheet\nGroup: {} ({})\nPerson: {}\nThis sheet: {}\nBackup machines:\n",
        s.name, s.group, s.person, s.label
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
    crate::recovery_exchange::welcome(d, device, new, join).await
}

/// Read-only metadata for an authenticated recovery sheet.
pub async fn bundle(d: &Daemon, group: &str, device: &str) -> Option<crate::net::recovery::Bundle> {
    crate::recovery_exchange::bundle(d, group, device).await
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
        Err(code) if matches!(code.as_str(), "invite-not-allowed" | "inviter-offline") => {
            vec![problem("recovery-not-allowed", &[], None)]
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
        return Err("already-in-group".into());
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
    let node = recovery_node(d, &secret).await?;
    let g = Group::open(&s.group, &path, &d.env)
        .await
        .ok_or("not-a-repository")?;
    g.set_token(&token);
    let result =
        crate::recovery_exchange::recover(d, &g, &s, &secret, &node, &name, &uuid, from.as_deref())
            .await;
    node.close().await;
    let theirs = result?;
    g.reload_roster().await;
    crate::trace_roster::moved(&d.tracer, &g, "Merge", None).await;
    hide_all(&g, &theirs).await?;
    let _g = d.add_group(g, &path).await?;
    d.wake.notify_one();
    Ok(s.name)
}

/// A temporary endpoint that authenticates with the sheet, never the new
/// computer's long-lived key. Its secret is never written to disk.
async fn recovery_node(d: &Daemon, secret: &[u8; 32]) -> Result<crate::net::Node, String> {
    let mut bind: SocketAddr = d
        .config
        .network
        .bind
        .parse()
        .map_err(|e| format!("[network] bind: {e}"))?;
    bind.set_port(0);
    crate::net::Node::bind(&crate::net::NetConfig {
        secret_key: *secret,
        bind,
        mdns: false,
        reach: d.config.network.reach().map_err(|e| e.to_string())?,
    })
    .await
    .map_err(|e| e.to_string())
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
            label: "2026-10-16-abc123".into(),
            key: "00".repeat(32),
            custodians: vec![SheetCustodian {
                device: "dd".into(),
                name: "nas".into(),
                addresses: vec!["127.0.0.1:1".into()],
            }],
        };
        let code = encode(&s);
        let text = format!("Some words\n{code}\nmore");
        assert_eq!(decode(&text), Some(s.clone()));
        let mut other = s.clone();
        other.name = "Other group".into();
        let label = format!("Group: {}\n{code}", encode(&other));
        assert_eq!(decode(&label), Some(s));
        let mut typo: Vec<char> = code.chars().collect();
        typo[9] = if typo[9] == 'a' { 'b' } else { 'a' };
        let typo: String = typo.into_iter().collect();
        assert_eq!(decode(&typo), None);
    }
}
