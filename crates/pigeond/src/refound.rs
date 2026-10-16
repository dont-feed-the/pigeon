//! `pigeon refound [REPOSITORY]` (09 §9.9, ADR 0047): the last resort when
//! no key of an admin survives. This device signs a new genesis whose tree
//! is `main`'s with a roster naming its person as the only admin and itself
//! as the only device, carrying `Pigeon-Refound: <old group> <old main>`.
//! The old `main` stays as `refs/pigeon/archive/<old group>/main`; the
//! repository then serves the new group in place, with the same annex UUID
//! and token, and the others join it again by invitation.

use std::path::Path;

use pigeon_core::authority;
use pigeon_core::roster::{DeviceId, Roster};
use pigeon_proto::Response;
use pigeon_proto::api::Arg;
use pigeon_proto::join::Refounded;

use crate::api::{problem, said};
use crate::daemon::Daemon;
use crate::group::Group;
use crate::roster_file::RosterDoc;

/// Where archived histories live.
pub const ARCHIVE: &str = "refs/pigeon/archive/";

/// Where the archived history of `old` lives.
pub fn archive_ref(old: &str) -> String {
    format!("{ARCHIVE}{old}/main")
}

/// The refspec that fetches a peer's archived histories beside its `main`.
pub fn refspec(peer: &str) -> String {
    format!("+refs/pigeon/archive/*:refs/pigeon/peers/{peer}/archive/*")
}

fn described_file(g: &Group) -> std::path::PathBuf {
    g.root().join("refounded.json")
}

/// What invitations of `g` say of its refounding, if its genesis refounds
/// another group.
pub fn described(g: &Group) -> Option<Refounded> {
    let bytes = std::fs::read(described_file(g)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Records what `g`'s genesis says of its refounding, for [`described`]:
/// the old group, the only person of its roster, and its time.
pub async fn remember(g: &Group) {
    let Ok(raw) = g.git.object(&g.id).await else {
        return;
    };
    let Some((from, _)) = refounded(&raw) else {
        return;
    };
    let roster = g
        .git
        .blob(&format!("{}:{}", g.id, authority::ROSTER))
        .await
        .and_then(|b| crate::roster_file::parse(&String::from_utf8_lossy(&b)).ok());
    let by = roster
        .and_then(|d| d.roster.persons.into_values().next())
        .map(|p| p.name)
        .unwrap_or_default();
    let at = authority::author_time(&raw).map_or(0, |(t, _)| t);
    let r = Refounded {
        from,
        by,
        at: crate::util::rfc3339(at),
    };
    if let Ok(bytes) = serde_json::to_vec(&r)
        && std::fs::read(described_file(g)).ok().as_deref() != Some(bytes.as_slice())
    {
        let _ = pigeon_sys::fs::write_atomic(&described_file(g), &bytes);
    }
}

/// Keeps the history the group's genesis refounds, once a fetch brought
/// it: the commit its signed trailer names, and nothing else, becomes
/// `refs/pigeon/archive/<old group>/main` (ADR 0047).
pub async fn adopt(g: &Group) {
    remember(g).await;
    if let Err(error) = crate::archive_chain::retain(g, &g.id).await {
        tracing::debug!(group = %g.id, %error, "archive-validation-waits");
    }
}

/// The trailer naming the group a genesis refounds.
pub const TRAILER: &str = "Pigeon-Refound:";

/// The old group and old `main` a refounding genesis names, from its raw
/// commit object.
pub fn refounded(raw: &[u8]) -> Option<(String, String)> {
    let text = String::from_utf8_lossy(raw);
    let line = text.lines().rev().find(|l| l.starts_with(TRAILER))?;
    let mut w = line[TRAILER.len()..].split_whitespace();
    Some((w.next()?.to_owned(), w.next()?.to_owned()))
}

/// The roster of the new group: `old`'s settings and folders, with `me`'s
/// person as the only admin and `me` as the only device.
pub fn new_roster(old: &Roster, me: &DeviceId) -> Option<Roster> {
    pigeon_core::refound::roster(old, me)
}

fn group_at(d: &Daemon, repository: Option<&Path>) -> Option<std::sync::Arc<Group>> {
    let Some(p) = repository else {
        return d.default_group();
    };
    let p = std::fs::canonicalize(p).ok()?;
    d.groups()
        .into_iter()
        .find(|g| g.worktree.as_deref() == Some(p.as_path()) || g.git_dir == p)
        .or_else(|| crate::api::group_of(d, &p))
}

/// `pigeon refound`: answers once the repository serves the new group.
pub async fn refound(d: &Daemon, repository: Option<&Path>) -> Vec<Response> {
    let Some(g) = group_at(d, repository) else {
        return vec![problem("not-in-group", &[], None)];
    };
    if crate::leave::leaving(&g).is_some() {
        return vec![problem("not-allowed", &[], Some("leaving".into()))];
    }
    let _busy = g.busy.lock().await;
    if crate::join_transition::pending(&g) {
        return vec![problem("join-incomplete", &[], None)];
    }
    if crate::evict::unsettled(&g) || crate::capture_file::pending(&g) {
        return vec![problem("repository-busy", &[], None)];
    }
    if let Err(e) = crate::erase_commit::recover(&g, &d.device, &d.tracer).await {
        return vec![problem("repository-busy", &[], Some(e.to_string()))];
    }
    let position = d
        .groups()
        .iter()
        .position(|live| std::sync::Arc::ptr_eq(live, &g));
    let (new, name, person) = match genesis(d, &g).await {
        Ok(x) => x,
        Err(e) => {
            if crate::join_transition::pending(&g) {
                g.stop_remotedaemon().await;
                d.groups
                    .write()
                    .unwrap_or_else(|p| p.into_inner())
                    .retain(|live| !std::sync::Arc::ptr_eq(live, &g));
                return vec![problem("join-incomplete", &[], Some(e))];
            }
            return vec![problem("internal-error", &[], Some(e))];
        }
    };
    if let Err(e) = switch(d, &g, &new, position).await {
        return vec![problem("join-incomplete", &[], Some(e))];
    }
    vec![said(
        "refounded",
        &[("group", Arg::Text(name)), ("name", Arg::Text(person))],
    )]
}

/// Signs the new genesis, archives the old `main` and moves `main` to it.
/// Answers the new group's ID, its name and the new admin's name.
async fn genesis(
    d: &Daemon,
    g: &std::sync::Arc<Group>,
) -> Result<(String, String, String), String> {
    let me = DeviceId(d.device.clone());
    let main = g.git.rev("refs/heads/main").await.ok_or("no main")?;
    crate::merge::judge_history(g, std::slice::from_ref(&main))
        .await
        .map_err(|e| e.to_string())?;
    if !g.lock().engine.accepted(&main) {
        return Err("refounding requires an accepted snapshot".into());
    }
    // Root every complete local epoch before selecting the archived tip.
    // Pending requests stay in that old authority domain, not the new one.
    let count = crate::purge_state::active(g, &main)
        .await
        .map_err(|e| e.to_string())?
        .len();
    for _ in 0..count {
        if !crate::purge_epoch::close(g)
            .await
            .map_err(|e| e.to_string())?
        {
            break;
        }
    }
    let doc = g.roster().ok_or("no roster")?;
    let roster = new_roster(&doc.roster, &me).ok_or("this device is not in the roster")?;
    let person = roster
        .devices
        .get(&me)
        .and_then(|x| roster.persons.get(&x.person))
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let old_main = g.git.rev("refs/heads/main").await.ok_or("no main")?;
    let fail = |e: crate::git::Error| e.to_string();
    let tree = new_tree(g, &old_main, &me).await?;
    let oid = crate::merge::write_tree(&g.git, &tree)
        .await
        .map_err(fail)?;
    let name = doc.roster.group.clone();
    let message = format!("pigeon: refound {name}\n\n{TRAILER} {} {old_main}", g.id);
    let new = g.git.commit_tree(&oid, &[], &message).await.map_err(fail)?;
    let judge = g.history_judge(&new);
    if crate::verify::judge(&judge, None, &new)
        .await
        .map_err(fail)?
        .accepted
        .as_deref()
        != Some(new.as_str())
    {
        return Err("refounded genesis failed validation".into());
    }
    let plan = crate::join_transition::Plan {
        format: 1,
        old_group: g.id.clone(),
        old_main: old_main.clone(),
        group: new.clone(),
        main: new.clone(),
        archive_main: old_main,
        stage: None,
        source: None,
    };
    crate::join_transition::record(g, &plan)?;
    g.stop_remotedaemon().await;
    d.groups
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|live| !std::sync::Arc::ptr_eq(live, g));
    crate::join_transition::finish(g, &d.dirs.config_file()).await?;
    tracing::info!(old = %g.id, group = %new, "refounded");
    Ok((new, name, person))
}

/// New authority-domain tree (proposed ADR 0077). Current files and other
/// settings survive. Old votes and erasure identities stay in their signed
/// archive; completed epochs are consumed there, with their original judge.
pub async fn new_tree(g: &Group, main: &str, me: &DeviceId) -> Result<authority::Tree, String> {
    let old = crate::membership::roster(g, main)
        .await
        .map_err(|e| e.to_string())?;
    let roster = new_roster(&old, me).ok_or("this device cannot refound")?;
    let text = crate::roster_file::render(&RosterDoc { roster })?;
    let blob = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut tree = g.git.tree(main).await.map_err(|e| e.to_string())?;
    tree.retain(|path, _| {
        !path.starts_with(crate::admission_file::PREFIX)
            && !path.starts_with(crate::purge_file::PREFIX)
            && !path.starts_with(crate::purge_epoch::PREFIX)
    });
    tree.insert(
        authority::ROSTER.into(),
        authority::Entry {
            mode: "100644".into(),
            kind: "blob".into(),
            oid: blob.trim().into(),
        },
    );
    Ok(tree)
}

/// Serves the repository of `old` as the group `new`.
async fn switch(
    d: &Daemon,
    old: &std::sync::Arc<Group>,
    new: &str,
    position: Option<usize>,
) -> Result<(), String> {
    let path = old.worktree.clone().unwrap_or_else(|| old.git_dir.clone());
    let mut g = Group::open(new, &path, &d.env)
        .await
        .ok_or("refounded repository missing")?;
    crate::shared::configure(&mut g, d.config.bridge_storage.as_ref())?;
    let g = std::sync::Arc::new(g);
    let busy = g.busy.lock().await;
    remember(&g).await;
    d.serve_group(g.clone(), None).await;
    if let Some(position) = position {
        let mut groups = d.groups.write().unwrap_or_else(|p| p.into_inner());
        if let Some(index) = groups
            .iter()
            .position(|live| std::sync::Arc::ptr_eq(live, &g))
        {
            let g = groups.remove(index);
            let position = position.min(groups.len());
            groups.insert(position, g);
        }
    }
    drop(busy);
    Ok(())
}

/// Stops serving `old`, once its round ends, and takes it out of
/// `config.toml`; its repository stays as it is.
pub async fn forget(d: &Daemon, old: &std::sync::Arc<Group>) {
    let _busy = old.busy.lock().await;
    if let Err(e) = crate::config::remove_group(&d.dirs.config_file(), &old.id) {
        tracing::warn!(group = %old.id, error = %e, "config-write-failed");
    }
    old.stop_remotedaemon().await;
    d.groups
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|x| x.id != old.id);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pigeon_core::roster::{Device, DeviceId, Person, PersonId, Role, Roster};

    use super::{new_roster, refounded};

    fn device(person: &str, role: Role) -> Device {
        Device {
            person: PersonId(person.to_owned()),
            name: format!("{person}-machine"),
            role,
            annex_uuid: Some(format!("uuid-{person}")),
            maxsize: None,
        }
    }

    fn roster() -> Roster {
        let person = |name: &str, admin| Person {
            name: name.to_owned(),
            admin,
        };
        Roster {
            rules: 1,
            group: "Family".to_owned(),
            k: 2,
            retention_days: 30,
            telemetry_device: None,
            relay: Some("https://relay.example".to_owned()),
            persons: BTreeMap::from([
                (PersonId("ana".to_owned()), person("Ana", true)),
                (PersonId("bob".to_owned()), person("Bob", false)),
            ]),
            devices: BTreeMap::from([
                (DeviceId("a1".to_owned()), device("ana", Role::Laptop)),
                (DeviceId("c1".to_owned()), device("ana", Role::Custodian)),
                (DeviceId("b1".to_owned()), device("bob", Role::Laptop)),
            ]),
            removed: BTreeMap::new(),
            folders: Vec::new(),
        }
    }

    #[test]
    fn the_refounder_is_the_only_admin_and_machine() {
        let old = roster();
        let me = DeviceId("b1".to_owned());
        let new = new_roster(&old, &me).unwrap();
        assert_eq!(new.devices.keys().collect::<Vec<_>>(), [&me]);
        assert_eq!(new.devices[&me], old.devices[&me]);
        assert_eq!(new.persons.len(), 1);
        assert!(new.persons[&PersonId("bob".to_owned())].admin);
        assert!(new.removed.is_empty());
        assert_eq!(
            (new.group, new.k, new.retention_days, new.relay),
            (old.group, old.k, old.retention_days, old.relay)
        );
        assert!(new_roster(&roster(), &DeviceId("zz".to_owned())).is_none());
    }

    #[test]
    fn the_trailer_names_the_old_group_and_main() {
        let raw = b"tree 1\nauthor x <x@pigeon> 1 +0000\n\npigeon: refound Family\n\nPigeon-Refound: g0 m0\n";
        assert_eq!(refounded(raw), Some(("g0".to_owned(), "m0".to_owned())));
        assert_eq!(refounded(b"tree 1\n\npigeon: create Family\n"), None);
    }
}
