//! A pending proposal keeps its historical author when that client's key is
//! removed. A surviving device of the person may withdraw it, while the
//! removed key and unrelated members cannot. Uses signed Git histories,
//! without daemons, network timing or git-annex.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::BTreeMap;

use pigeon_core::authority::{self, Decided, Entry, Refusal, Tree, Verdict};
use pigeon_core::roster::{Device, Person, PersonId, Role, Roster};
use pigeon_core::sig;
use pigeon_git::GitEnv;
use pigeond::decision_file::{self, DecisionDoc};
use pigeond::git::Git;
use pigeond::group::Group;
use pigeond::{decide, merge, proposal, roster_file, verify};

async fn blob(git: &Git, text: &str) -> Entry {
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: git
            .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
            .await
            .unwrap()
            .trim()
            .into(),
    }
}

async fn commit(git: &Git, tree: &Tree, parent: Option<&str>, key: u8) -> String {
    let root = merge::write_tree(git, tree).await.unwrap();
    let parent = parent.map_or(String::new(), |p| format!("parent {p}\n"));
    let raw = format!(
        "tree {root}\n{parent}author test <test@pigeon> 1791892800 +0000\ncommitter test <test@pigeon> 1791892800 +0000\n\npigeon: author regression\n"
    );
    git.run_in(
        &["hash-object", "-t", "commit", "-w", "--stdin"],
        &sig::sign(raw.as_bytes(), &[key; 32]).unwrap(),
    )
    .await
    .unwrap()
    .trim()
    .into()
}

fn roster() -> Roster {
    Roster {
        rules: 1,
        group: "historical-author".into(),
        invitation: "a".repeat(64),
        k: 1,
        retention_days: 30,
        relay: None,
        telemetry_device: None,
        persons: [("owner", true), ("member", false), ("other", false)]
            .into_iter()
            .map(|(name, admin)| {
                (
                    PersonId(name.into()),
                    Person {
                        name: name.into(),
                        admin,
                    },
                )
            })
            .collect(),
        devices: [(1, "owner"), (2, "member"), (3, "member"), (4, "other")]
            .into_iter()
            .map(|(key, person)| {
                (
                    sig::device_id(&[key; 32]),
                    Device {
                        person: PersonId(person.into()),
                        name: format!("device-{key}"),
                        role: Role::Laptop,
                        annex_uuid: Some(format!("00000000-0000-0000-0000-{key:012}")),
                        maxsize: None,
                    },
                )
            })
            .collect(),
        removed: BTreeMap::new(),
        folders: vec![],
    }
}

#[tokio::test]
async fn a_removed_clients_proposal_retains_its_person_and_sibling_authority() {
    let temp = tempfile::tempdir().unwrap();
    let git = Git::new(temp.path(), GitEnv::default());
    git.run(&["init", "-q", "--bare", "-b", "main"])
        .await
        .unwrap();
    let mut roster = roster();
    let rendered = roster_file::render(&roster_file::RosterDoc {
        roster: roster.clone(),
    })
    .unwrap();
    let original = format!("/annex/objects/SHA256E-s1--{}\n", "a".repeat(64));
    let mut tree: Tree = [
        (authority::ROSTER.into(), blob(&git, &rendered).await),
        ("review.txt".into(), blob(&git, &original).await),
    ]
    .into();
    let genesis = commit(&git, &tree, None, 1).await;
    git.update_ref("refs/heads/main", &genesis, None)
        .await
        .unwrap();
    let group = Group::open(&genesis, temp.path(), &GitEnv::default())
        .await
        .unwrap();

    let mut proposed = tree.clone();
    let replacement = format!("/annex/objects/SHA256E-s1--{}\n", "b".repeat(64));
    proposed.insert("review.txt".into(), blob(&git, &replacement).await);
    let suggestion = commit(&git, &proposed, Some(&genesis), 2).await;
    let author = sig::device_id(&[2; 32]);
    git.update_ref(
        &format!(
            "refs/pigeon/peers/client/proposals/{}/{suggestion}",
            author.0
        ),
        &suggestion,
        None,
    )
    .await
    .unwrap();
    assert_eq!(proposal::take(&group, "client").await, 1);

    let removed = roster.devices.remove(&author).unwrap();
    roster.removed.insert(author.clone(), removed.annex_uuid);
    let rendered = roster_file::render(&roster_file::RosterDoc { roster }).unwrap();
    tree.insert(authority::ROSTER.into(), blob(&git, &rendered).await);
    let removal = commit(&git, &tree, Some(&genesis), 1).await;
    let judged = verify::judge(&group, Some(&genesis), &removal)
        .await
        .unwrap();
    assert_eq!(judged.accepted.as_deref(), Some(removal.as_str()));
    git.update_ref("refs/heads/main", &removal, Some(&genesis))
        .await
        .unwrap();
    group.reload_roster().await;

    assert!(!group.roster().unwrap().roster.devices.contains_key(&author));
    let pending = decide::pending(&group).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, suggestion);
    assert_eq!(pending[0].device, author.0);
    assert_eq!(pending[0].person, Some(PersonId("member".into())));

    // Attribution preserves authorship, not authority for the removed key.
    // Only the surviving device of that person may withdraw; it still may
    // not reject the owner's suggestion on behalf of the owner.
    for (key, verdict, allowed) in [
        (2, Decided::Withdrawn, false),
        (4, Decided::Withdrawn, false),
        (3, Decided::Rejected, false),
        (3, Decided::Withdrawn, true),
    ] {
        let doc = DecisionDoc {
            proposal: suggestion.clone(),
            verdict,
            note: None,
        };
        let mut decided = tree.clone();
        decided.insert(
            authority::decision_path(&suggestion),
            blob(&git, &decision_file::render(&doc)).await,
        );
        let decision = commit(&git, &decided, Some(&removal), key).await;
        let judged = verify::judge(&group, Some(&removal), &decision)
            .await
            .unwrap();
        if allowed {
            assert_eq!(judged.accepted.as_deref(), Some(decision.as_str()));
        } else {
            assert!(judged.accepted.is_none(), "{key}: {judged:?}");
            assert!(matches!(
                judged.stopped,
                Some((
                    _,
                    Verdict::Refused(Refusal::UnknownSigner(_) | Refusal::NotAllowed(_))
                ))
            ));
        }
    }
}
