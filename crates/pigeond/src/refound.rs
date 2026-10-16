//! `pigeon refound [REPOSITORY]` (09 §9.9, ADR 0047): the last resort when
//! no key of an admin survives. This device signs a new genesis whose tree
//! is `main`'s with a roster naming its person as the only admin and itself
//! as the only device, carrying `Pigeon-Refound: <old group> <old main>`.
//! The old `main` stays as `refs/pigeon/archive/<old group>/main`; the
//! repository then serves the new group in place, with the same annex UUID
//! and token, and the others join it again by invitation.

use std::collections::BTreeMap;
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
    if described_file(g).exists() {
        return;
    }
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
    if let Ok(bytes) = serde_json::to_vec(&r) {
        let _ = pigeon_sys::fs::write_atomic(&described_file(g), &bytes);
    }
}

/// Keeps the history the group's genesis refounds, once a fetch brought
/// it: the commit its signed trailer names, and nothing else, becomes
/// `refs/pigeon/archive/<old group>/main` (ADR 0047).
pub async fn adopt(g: &Group) {
    remember(g).await;
    let Ok(raw) = g.git.object(&g.id).await else {
        return;
    };
    let Some((old, old_main)) = refounded(&raw) else {
        return;
    };
    let name = archive_ref(&old);
    if g.git.rev(&name).await.is_some() {
        return;
    }
    let commit = format!("{old_main}^{{commit}}");
    if g.git.ok(&["cat-file", "-e", &commit]).await
        && g.git.update_ref(&name, &old_main, None).await.is_ok()
    {
        tracing::info!(group = %g.id, old = %old, "archive-kept");
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
    let device = old.devices.get(me)?.clone();
    let mut person = old.persons.get(&device.person)?.clone();
    person.admin = true;
    let r = Roster {
        persons: BTreeMap::from([(device.person.clone(), person)]),
        devices: BTreeMap::from([(me.clone(), device)]),
        removed: BTreeMap::new(),
        ..old.clone()
    };
    r.validate().ok()?;
    Some(r)
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
    let made = {
        let _busy = g.busy.lock().await;
        genesis(d, &g).await
    };
    let (new, name, person) = match made {
        Ok(x) => x,
        Err(e) => return vec![problem("internal-error", &[], Some(e))],
    };
    switch(d, &g, &new).await;
    vec![said(
        "refounded",
        &[("group", Arg::Text(name)), ("name", Arg::Text(person))],
    )]
}

/// Signs the new genesis, archives the old `main` and moves `main` to it.
/// Answers the new group's ID, its name and the new admin's name.
async fn genesis(d: &Daemon, g: &Group) -> Result<(String, String, String), String> {
    let me = DeviceId(d.device.clone());
    let doc = g.roster().ok_or("no roster")?;
    let roster = new_roster(&doc.roster, &me).ok_or("this device is not in the roster")?;
    let person = roster
        .devices
        .get(&me)
        .and_then(|x| roster.persons.get(&x.person))
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let old_main = g.git.rev("refs/heads/main").await.ok_or("no main")?;
    let text = crate::roster_file::render(&RosterDoc { roster })?;
    let fail = |e: crate::git::Error| e.to_string();
    let blob = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await
        .map_err(fail)?;
    let mut tree = g.git.tree(&old_main).await.map_err(fail)?;
    tree.insert(
        authority::ROSTER.to_owned(),
        authority::Entry {
            mode: "100644".to_owned(),
            kind: "blob".to_owned(),
            oid: blob.trim().to_owned(),
        },
    );
    let oid = crate::merge::write_tree(&g.git, &tree)
        .await
        .map_err(fail)?;
    let name = doc.roster.group.clone();
    let message = format!("pigeon: refound {name}\n\n{TRAILER} {} {old_main}", g.id);
    let new = g.git.commit_tree(&oid, &[], &message).await.map_err(fail)?;
    g.git
        .update_ref(&archive_ref(&g.id), &old_main, None)
        .await
        .map_err(fail)?;
    g.git
        .update_ref("refs/heads/main", &new, Some(&old_main))
        .await
        .map_err(fail)?;
    // The folder and the index get the new roster; nothing else changed.
    crate::change::stage_file(
        &g.git,
        g.worktree.as_deref(),
        authority::ROSTER,
        text.as_bytes(),
    )
    .await
    .map_err(fail)?;
    tracing::info!(old = %g.id, group = %new, "refounded");
    Ok((new, name, person))
}

/// Serves the repository of `old` as the group `new`.
async fn switch(d: &Daemon, old: &std::sync::Arc<Group>, new: &str) {
    let path = old.worktree.clone().unwrap_or_else(|| old.git_dir.clone());
    forget(d, old).await;
    match Group::open(new, &path, &d.env).await {
        Some(g) => {
            remember(&g).await;
            d.add_group(g, &path).await;
        }
        None => tracing::warn!(group = %new, "refound-open-failed"),
    }
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
