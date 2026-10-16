//! `RECOVERY.txt` on a backup machine (09 §9.9, 11 §11.3, ADR 0047): a note
//! beside the repository, outside git, written again whenever the roster
//! changes. It names the group and the other backup machines and gives the
//! commands that read the files with git and git-annex alone, as E21 runs
//! them, and with `pigeon export`.

use std::path::Path;

use pigeon_core::filter::Role;
use pigeon_core::roster::Roster;

use crate::group::Group;

/// The file's name, in the repository directory.
pub const NAME: &str = "RECOVERY.txt";

/// The note for the repository at `dir` of `roster`, on device `me`.
pub fn text(roster: &Roster, me: &str, dir: &Path) -> String {
    let others: Vec<&str> = roster
        .devices
        .iter()
        .filter(|(id, d)| d.role == pigeon_core::roster::Role::Custodian && id.0 != me)
        .map(|(_, d)| d.name.as_str())
        .collect();
    let dir = format!("'{}'", dir.to_string_lossy().replace('\'', "'\\''"));
    let mut t = format!(
        "pigeon backup of the group \"{}\"\n\
         \n\
         This folder is a git repository with git-annex: it holds the\n\
         group's files and their history. pigeon writes this note again\n\
         whenever the group changes; it is not part of the repository.\n\n",
        roster.group
    );
    if others.is_empty() {
        t.push_str("The group has no other backup machine.\n\n");
    } else {
        t.push_str("The other backup machines: ");
        t.push_str(&others.join(", "));
        t.push_str(".\n\n");
    }
    t.push_str(&format!(
        "To read the files with git and git-annex alone:\n\
         \n\
         \x20   git clone -- {dir} files\n\
         \x20   cd files\n\
         \x20   git config --local user.name 'Recovered pigeon files'\n\
         \x20   git config --local user.email recovery@localhost\n\
         \x20   git annex init\n\
         \n\
         For each other backup machine's repository (a disk, or ssh):\n\
         \n\
         \x20   git remote add NAME PATH-OR-URL\n\
         \x20   git fetch NAME\n\
         \n\
         Then the current files:\n\
         \n\
         \x20   git annex get .\n\
         \n\
         `git annex get --all` adds the history. Expect it to report some\n\
         files it cannot get: versions that only a laptop ever held.\n\
         \n\
         With pigeon, without the network or a key, into a new folder, each\n\
         file checked:\n\
         \n\
         \x20   pigeon export --to FOLDER {dir} [OTHER REPOSITORIES...]\n"
    ));
    t
}

/// Writes the note of a backup machine's repository if it changed.
pub fn write(g: &Group, me: &str) {
    if g.role != Role::Custodian {
        return;
    }
    let Some(doc) = g.roster() else {
        return;
    };
    let at = g.git_dir.join(NAME);
    let body = text(&doc.roster, me, &g.git_dir);
    if std::fs::read_to_string(&at).ok().as_deref() != Some(body.as_str())
        && let Err(e) = write_atomic(&at, body.as_bytes())
    {
        tracing::warn!(group = %g.id, error = %e, "recovery-txt-unwritten");
    }
}

fn write_atomic(at: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let part = at.with_extension("txt.part");
    std::fs::write(&part, bytes)?;
    std::fs::rename(&part, at)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;

    use pigeon_core::roster::{Device, DeviceId, Person, PersonId};

    fn device(name: &str, role: pigeon_core::roster::Role) -> Device {
        Device {
            person: PersonId("p".into()),
            name: name.into(),
            role,
            annex_uuid: None,
            maxsize: None,
        }
    }

    #[test]
    fn the_note_names_the_group_and_the_other_backup_machines() {
        let roster = Roster {
            rules: 1,
            group: "Family".into(),
            invitation: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            k: 2,
            retention_days: 30,
            telemetry_device: None,
            relay: None,
            persons: [(
                PersonId("p".into()),
                Person {
                    name: "Ana".into(),
                    admin: true,
                },
            )]
            .into(),
            devices: [
                (
                    DeviceId("d1".into()),
                    device("lap", pigeon_core::roster::Role::Laptop),
                ),
                (
                    DeviceId("d2".into()),
                    device("c1", pigeon_core::roster::Role::Custodian),
                ),
                (
                    DeviceId("d3".into()),
                    device("c2", pigeon_core::roster::Role::Custodian),
                ),
            ]
            .into(),
            removed: std::collections::BTreeMap::new(),
            folders: Vec::new(),
        };
        let t = text(&roster, "d2", Path::new("/srv/Family.git"));
        assert!(t.contains("group \"Family\""), "{t}");
        assert!(t.contains("The other backup machines: c2."), "{t}");
        assert!(t.contains("git clone -- '/srv/Family.git' files"), "{t}");
        assert!(
            t.contains("pigeon export --to FOLDER '/srv/Family.git'"),
            "{t}"
        );
        let alone = text(&roster, "d3", Path::new("/b"));
        assert!(alone.contains("machines: c1."), "{alone}");
    }
}
