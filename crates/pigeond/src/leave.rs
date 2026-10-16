//! `pigeon leave` (09 §9.7): this computer signs its own removal, keeps
//! serving the group until another machine has it, then forgets the group.
//!
//! The removal waits in `<git_dir>/pigeon/leaving`. A peer has it once its
//! `main`, as last fetched, holds it, or once it refuses this device as
//! `not-in-roster` after previously including this device (09 §9.3).
//! Peers that refused it before, or never learned its join, do not count.
//! Meanwhile each round does nothing else: what it would capture or install now
//! its peers would revoke (04 §4.4). Then the group leaves `config.toml`,
//! its remotedaemon stops and, for a folder, the files it does not hold
//! (pointer files) go, and `.git` and `.pigeon` move to
//! `<state>/left/<group>-<time>/`: the folder keeps its files as plain
//! files. A custodian's repository stays where it is. Once no group is
//! left, the device key is deleted. The folder itself is never deleted
//! here **[no ADR]**. A computer alone in its group has nobody to tell, so
//! it forgets the group at once, last admin or not.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pigeon_core::filter::Role;
use pigeon_core::roster::{DeviceId, Edit, apply, may_sign};
use pigeon_proto::Response;
use pigeon_proto::api::Arg;
use pigeon_proto::stream::Refusal;
use serde::{Deserialize, Serialize};

use crate::api::problem;
use crate::daemon::Daemon;
use crate::group::Group;
use crate::removed::peers;
use crate::status::msg;

/// The removal this device waits to see on a peer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leaving {
    /// The roster commit removing this device.
    pub commit: String,
    /// The peers that already refused this device as `not-in-roster`.
    #[serde(default)]
    pub refusing: BTreeSet<String>,
}

fn marker(g: &Group) -> PathBuf {
    g.files.root.join("leaving")
}

/// The removal `g` waits on, if this device is leaving it.
pub fn leaving(g: &Group) -> Option<Leaving> {
    let bytes = std::fs::read(marker(g)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn text(s: &str) -> Arg {
    Arg::Text(s.to_owned())
}

/// `pigeon leave`: signs this device's removal from the default group.
pub async fn leave(d: &Daemon) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![problem("not-in-group", &[], None)];
    };
    if leaving(&g).is_some() {
        return vec![Response::Said {
            message: msg("leaving-already", &[]),
        }];
    }
    let _busy = g.busy.lock().await;
    let Some(mut doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let me = DeviceId(d.device.clone());
    let name = doc
        .roster
        .devices
        .get(&me)
        .map_or_else(|| d.device.clone(), |x| x.name.clone());
    if peers(&doc.roster, &me).is_empty() {
        if let Err(e) = finish(d, &g).await {
            return vec![problem("internal-error", &[], Some(e))];
        }
        return vec![Response::Said {
            message: msg(
                "left",
                &[("name", text(&name)), ("group", text(&doc.roster.group))],
            ),
        }];
    }
    let change = [Edit::RemoveDevice(me.clone())];
    if !may_sign(&doc.roster, &me, &change) {
        return vec![problem("not-allowed", &[], None)];
    }
    let applied = apply(&doc.roster, &change);
    if let Some(why) = applied.skipped {
        return vec![problem("not-allowed", &[], Some(format!("{why:?}")))];
    }
    if !signs_for_an_admin(&applied.roster) {
        // No key could sign the group's admin changes any more.
        return vec![problem("leave-last-admin", &[], None)];
    }
    let refusing = peers(&doc.roster, &me)
        .into_iter()
        .filter(|p| d.node.refusal(p, &g.id) == Some(Refusal::NotInRoster))
        .collect();
    doc.roster = applied.roster;
    let message = format!("pigeon: {name} leaves");
    let commit = match crate::change::commit_roster(&g, &doc, &message, &d.tracer).await {
        Ok(c) => c,
        Err(e) => return vec![problem("internal-error", &[], Some(e))],
    };
    let state = Leaving { commit, refusing };
    let written = serde_json::to_vec(&state)
        .map_err(|e| e.to_string())
        .and_then(|b| pigeon_sys::fs::write_atomic(&marker(&g), &b).map_err(|e| e.to_string()));
    if let Err(e) = written {
        return vec![problem("internal-error", &[], Some(e))];
    }
    tracing::info!(group = %g.id, commit = %state.commit, "leaving");
    d.wake.notify_one();
    vec![Response::Said {
        message: msg("leaving", &[("name", text(&name))]),
    }]
}

/// Whether some device of `roster`, a recovery sheet included, belongs to
/// an admin.
pub fn signs_for_an_admin(roster: &pigeon_core::roster::Roster) -> bool {
    roster
        .devices
        .values()
        .any(|x| roster.persons.get(&x.person).is_some_and(|p| p.admin))
}

/// Whether a peer has the removal `l`: its fetched `main` holds it, or it
/// refuses this device since previously including it. A stale roster that
/// never learned the join also refuses it, but proves no removal was saved.
/// With no peer at all, at once.
async fn seen(d: &Daemon, g: &Group, l: &Leaving) -> bool {
    let me = DeviceId(d.device.clone());
    let Some(doc) = g.roster() else {
        return true;
    };
    let peers = peers(&doc.roster, &me);
    if peers.is_empty() {
        return true;
    }
    for p in &peers {
        let theirs = format!("refs/pigeon/peers/{p}/main");
        if g.git
            .ok(&["merge-base", "--is-ancestor", &l.commit, &theirs])
            .await
        {
            return true;
        }
        if !l.refusing.contains(p) && crate::removed::refuses_after_knowing(d, g, p, &me).await {
            return true;
        }
    }
    false
}

/// A round of a group this device leaves: forgets it once a peer has the
/// removal. Answers whether `g` is being left, so the round does nothing
/// else.
pub async fn round(d: &Daemon, g: &std::sync::Arc<Group>) -> bool {
    let Some(l) = leaving(g) else {
        return false;
    };
    if seen(d, g, &l).await
        && let Err(e) = finish(d, g).await
    {
        tracing::warn!(group = %g.id, error = %e, "config-write-failed");
    }
    true
}

/// Forgets `g`: out of `config.toml` and the served groups, its
/// remotedaemon stopped, a folder's repository moved out of it. Fails,
/// changing nothing, when `config.toml` cannot be written.
async fn finish(d: &Daemon, g: &std::sync::Arc<Group>) -> Result<(), String> {
    crate::config::remove_group(&d.dirs.config_file(), &g.id).map_err(|e| e.to_string())?;
    let absent = if g.role == Role::Laptop {
        absent_files(g).await
    } else {
        Vec::new()
    };
    g.stop_remotedaemon().await;
    d.groups
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|x| x.id != g.id);
    if let Some(wt) = &g.worktree {
        for (rel, key) in absent {
            let path = wt.join(rel);
            if is_pointer(&path, &key) {
                let _ = std::fs::remove_file(path);
            }
        }
        let to = d
            .dirs
            .state
            .join("left")
            .join(format!("{}-{}", g.id, crate::util::now()));
        if let Err(e) = move_out(wt, &to) {
            tracing::warn!(group = %g.id, error = %e, "leave-move-failed");
        }
    }
    tracing::info!(group = %g.id, "left-group");
    if d.groups().is_empty() {
        let _ = std::fs::remove_file(d.dirs.device_key());
        tracing::info!("device-key-deleted");
    }
    Ok(())
}

/// The files of `g`'s folder whose committed content is not here, which
/// are pointer files unless changed since.
async fn absent_files(g: &Group) -> Vec<(String, String)> {
    let out = g
        .git
        .run(&["annex", "find", "--not", "--in=here"])
        .await
        .unwrap_or_default();
    let keys = g.git.keys("refs/heads/main").await.unwrap_or_default();
    out.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|path| keys.get(path).map(|key| (path.to_owned(), key.clone())))
        .collect()
}

/// Whether `path` holds only git-annex's pointer to absent content: a
/// symbolic link or a short file naming an annex object. A file the person
/// changed, whose committed version is absent here, holds their bytes and
/// stays, as leaving promises.
fn is_pointer(path: &Path, key: &str) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => std::fs::read_link(path).is_ok_and(|target| {
            target.to_string_lossy().contains("annex/objects/")
                && target.file_name().is_some_and(|name| name == key)
        }),
        Ok(m) if m.is_file() && m.len() < 1024 => std::fs::read(path).is_ok_and(|bytes| {
            bytes.strip_suffix(b"\n").unwrap_or(&bytes)
                == format!("/annex/objects/{key}").as_bytes()
        }),
        _ => false,
    }
}

/// Moves `.git` and `.pigeon` of the folder `wt` into `to`.
fn move_out(wt: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    std::fs::rename(wt.join(".git"), to.join("git"))?;
    if wt.join(".pigeon").exists() {
        std::fs::rename(wt.join(".pigeon"), to.join("pigeon"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pigeon_core::roster::{Device, Person, PersonId, Role as DeviceRole, Roster};

    fn device(person: &str, role: DeviceRole) -> Device {
        Device {
            person: PersonId(person.into()),
            name: person.into(),
            role,
            annex_uuid: None,
            maxsize: None,
        }
    }

    #[test]
    fn alone_a_computer_has_no_peer_and_else_an_admin_must_remain() {
        let ana = DeviceId("a1".into());
        let mut roster = Roster {
            rules: 1,
            group: "Family".into(),
            invitation: String::new(),
            k: 1,
            retention_days: 30,
            relay: None,
            telemetry_device: None,
            persons: [("ana", true), ("bob", false)]
                .into_iter()
                .map(|(id, admin)| {
                    let name = id.into();
                    (PersonId(id.into()), Person { name, admin })
                })
                .collect(),
            devices: [(ana.clone(), device("ana", DeviceRole::Laptop))].into(),
            removed: std::collections::BTreeMap::new(),
            folders: Vec::new(),
        };
        let sheet = DeviceId("sheet".into());
        roster
            .devices
            .insert(sheet, device("ana", DeviceRole::Recovery));
        assert!(peers(&roster, &ana).is_empty(), "a sheet is no peer");
        roster
            .devices
            .insert(DeviceId("b1".into()), device("bob", DeviceRole::Laptop));
        assert_eq!(peers(&roster, &ana), ["b1"]);
        roster.devices.retain(|id, _| *id != ana);
        assert!(signs_for_an_admin(&roster), "Ana's sheet still signs");
        roster.devices.retain(|_, x| x.role != DeviceRole::Recovery);
        assert!(!signs_for_an_admin(&roster), "only Bob's laptop is left");
    }

    #[test]
    fn only_pointers_to_absent_content_are_removed_on_leaving() {
        let dir = tempfile::tempdir().unwrap();
        let key = "SHA256E-s9--abc.txt";
        let pointer = dir.path().join("pointer.txt");
        std::fs::write(&pointer, "/annex/objects/SHA256E-s9--abc.txt\n").unwrap();
        let edited = dir.path().join("edited.txt");
        std::fs::write(&edited, "Bob's edit of recipes\n").unwrap();
        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(".git/annex/objects/xx/yy/SHA256E-s9--abc.txt", &link).unwrap();
        assert!(is_pointer(&pointer, key));
        assert!(is_pointer(&link, key));
        assert!(!is_pointer(&edited, key));
        assert!(!is_pointer(&dir.path().join("gone.txt"), key));
        std::fs::write(&edited, "/annex/objects/a private note\nkeep these bytes\n").unwrap();
        assert!(!is_pointer(&edited, key));
        std::fs::write(
            &edited,
            format!("/annex/objects/{key}\nkeep this private suffix\n"),
        )
        .unwrap();
        assert!(!is_pointer(&edited, key));
        assert!(!is_pointer(&pointer, "SHA256E-s9--different.txt"));
        assert!(!is_pointer(&link, "SHA256E-s9--different.txt"));
    }
}
