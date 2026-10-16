//! Paper-key advice derived from accepted roster history (ADR 0047).
//! Only public roster data is read: the first appearance of each current
//! paper key records the backup machines named when it was prepared.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::roster::{DeviceId, PersonId, Role, Roster};

use crate::{group::Group, roster_file};

fn backups(roster: &Roster) -> BTreeSet<DeviceId> {
    roster
        .devices
        .iter()
        .filter(|(_, d)| d.role == Role::Custodian)
        .map(|(id, _)| id.clone())
        .collect()
}

/// People with paper keys but no current sheet naming today's backups.
pub async fn refresh(group: &Group, roster: &Roster) -> BTreeSet<PersonId> {
    let mut pending: BTreeMap<_, _> = roster
        .devices
        .iter()
        .filter(|(_, d)| d.role == Role::Recovery)
        .map(|(id, d)| (id.clone(), d.person.clone()))
        .collect();
    if pending.is_empty() {
        return BTreeSet::new();
    }
    let people: BTreeSet<_> = pending.values().cloned().collect();
    let current = backups(roster);
    let mut fresh = BTreeSet::new();
    let main = group.lock().roster_commit.clone().unwrap_or_default();
    let Ok(commits) = group
        .git
        .run(&[
            "rev-list",
            "--reverse",
            "--topo-order",
            "--full-history",
            &main,
            "--",
            ".pigeon/roster.toml",
        ])
        .await
    else {
        return BTreeSet::new();
    };
    for commit in commits.lines() {
        let Some(blob) = group
            .git
            .blob(&format!("{commit}:.pigeon/roster.toml"))
            .await
        else {
            continue;
        };
        let Ok(doc) = roster_file::parse(&String::from_utf8_lossy(&blob)) else {
            continue;
        };
        let first: Vec<_> = pending
            .keys()
            .filter(|id| doc.roster.devices.contains_key(*id))
            .cloned()
            .collect();
        let matches = backups(&doc.roster) == current;
        for key in first {
            if let Some(person) = pending.remove(&key)
                && matches
            {
                fresh.insert(person);
            }
        }
        if pending.is_empty() {
            break;
        }
    }
    people.difference(&fresh).cloned().collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;
    use crate::{git::Git, roster_file::RosterDoc};
    use pigeon_core::authority::{Entry, ROSTER};
    use pigeon_core::roster::Device;
    use pigeon_git::GitEnv;

    async fn commit(git: &Git, roster: &Roster, parent: Option<&str>) -> String {
        let text = roster_file::render(&RosterDoc {
            roster: roster.clone(),
        })
        .unwrap();
        let oid = git
            .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
            .await
            .unwrap();
        let tree = [(
            ROSTER.into(),
            Entry {
                mode: "100644".into(),
                kind: "blob".into(),
                oid: oid.trim().into(),
            },
        )]
        .into();
        let tree = crate::merge::write_tree(git, &tree).await.unwrap();
        let mut args = vec!["commit-tree", &tree, "-m", "roster"];
        if let Some(parent) = parent {
            args.extend(["-p", parent]);
        }
        let unsigned = git.run(&args).await.unwrap();
        let signed = git.sign(unsigned.trim(), &[1; 32]).await.unwrap();
        git.update_ref("refs/heads/main", &signed, parent)
            .await
            .unwrap();
        signed
    }

    #[tokio::test]
    async fn changed_backups_recommend_a_new_sheet_until_a_current_one_exists() {
        let dir = tempfile::tempdir().unwrap();
        let env = GitEnv::default();
        let git = Git::new(dir.path(), env.clone());
        git.run(&["init", "--bare", "-q", "-b", "main"])
            .await
            .unwrap();
        git.run(&["config", "user.name", "Ana"]).await.unwrap();
        git.run(&["config", "user.email", "ana@pigeon"])
            .await
            .unwrap();
        let mut roster = roster_file::parse(include_str!(
            "../../../fixtures/formats/roster/known/group.toml"
        ))
        .unwrap()
        .roster;
        let root = commit(&git, &roster, None).await;
        let group = Group::open(&root, dir.path(), &env).await.unwrap();
        assert!(refresh(&group, &roster).await.is_empty());
        let id = pigeon_core::sig::device_id(&[8; 32]);
        roster.devices.insert(
            id,
            Device {
                person: PersonId("ana".into()),
                name: "New backup".into(),
                role: Role::Custodian,
                annex_uuid: Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e08".into()),
                maxsize: Some(1_000_000),
            },
        );
        let changed = commit(&git, &roster, Some(&root)).await;
        group.reload_roster().await;
        assert_eq!(
            refresh(&group, &roster).await,
            [PersonId("ana".into())].into()
        );
        roster.devices.insert(
            pigeon_core::sig::device_id(&[9; 32]),
            Device {
                person: PersonId("ana".into()),
                name: "New paper".into(),
                role: Role::Recovery,
                annex_uuid: None,
                maxsize: None,
            },
        );
        commit(&git, &roster, Some(&changed)).await;
        group.reload_roster().await;
        assert!(refresh(&group, &roster).await.is_empty());
    }
}
