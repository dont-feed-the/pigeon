//! A decision can arrive before its separately relayed proposal. Once the
//! proposal is validated and kept, retry that decision and its descendants;
//! an incomplete judgement must not freeze future catalogue propagation.
//! Uses signed Git objects without daemons, transfers or timing assumptions.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::BTreeMap;

use pigeon_core::authority::{Decided, Entry, Tree, Verdict};
use pigeon_core::roster::{Device, Person, PersonId, Role, Roster};
use pigeon_core::sig;
use pigeon_git::GitEnv;
use pigeond::decision_file::{self, DecisionDoc};
use pigeond::git::Git;
use pigeond::group::Group;
use pigeond::{merge, proposal, roster_file, verify};
use sha2::{Digest, Sha256};

async fn blob(git: &Git, bytes: &[u8]) -> Entry {
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: git
            .run_in(&["hash-object", "-w", "--stdin"], bytes)
            .await
            .unwrap()
            .trim()
            .to_owned(),
    }
}

async fn pointer(git: &Git, bytes: &[u8]) -> Entry {
    let hash: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    blob(
        git,
        format!("/annex/objects/SHA256E-s{}--{hash}\n", bytes.len()).as_bytes(),
    )
    .await
}

async fn commit(git: &Git, tree: &Tree, parent: Option<&str>, signer: u8) -> String {
    let root = merge::write_tree(git, tree).await.unwrap();
    let parent = parent.map_or(String::new(), |id| format!("parent {id}\n"));
    let body = format!(
        "tree {root}\n{parent}author test <test@pigeon> 1791892800 +0000\ncommitter test <test@pigeon> 1791892800 +0000\n\npigeon: regression\n"
    );
    let signed = sig::sign(body.as_bytes(), &[signer; 32]).unwrap();
    git.run_in(&["hash-object", "-t", "commit", "-w", "--stdin"], &signed)
        .await
        .unwrap()
        .trim()
        .to_owned()
}

fn roster() -> Roster {
    let persons = [("alice", true), ("bob", false)]
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
        .collect();
    let devices = [(1, "alice"), (2, "bob")]
        .into_iter()
        .map(|(key, person)| {
            (
                sig::device_id(&[key; 32]),
                Device {
                    person: PersonId(person.into()),
                    name: person.into(),
                    role: Role::Laptop,
                    annex_uuid: Some(format!("00000000-0000-0000-0000-{key:012}")),
                    maxsize: None,
                },
            )
        })
        .collect();
    Roster {
        rules: 1,
        group: "retry".into(),
        k: 1,
        retention_days: 30,
        relay: None,
        persons,
        devices,
        removed: BTreeMap::new(),
        folders: vec![],
    }
}

#[tokio::test]
async fn a_late_proposal_unblocks_its_decision_and_later_native_edits() {
    for (verdict, decider) in [(Decided::Rejected, 1), (Decided::Withdrawn, 2)] {
        let temp = tempfile::tempdir().unwrap();
        let git = Git::new(temp.path(), GitEnv::default());
        git.run(&["init", "-q", "--bare", "-b", "main"])
            .await
            .unwrap();
        let text = roster_file::render(&roster_file::RosterDoc { roster: roster() }).unwrap();
        let mut tree: Tree = [
            (
                ".pigeon/roster.toml".into(),
                blob(&git, text.as_bytes()).await,
            ),
            (
                ".gitattributes".into(),
                blob(&git, pigeond::settings::GITATTRIBUTES.as_bytes()).await,
            ),
            (
                "Docs/review.txt".into(),
                pointer(&git, b"Alice's original\n").await,
            ),
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
        proposed.insert(
            "Docs/review.txt".into(),
            pointer(&git, b"Bob's suggestion\n").await,
        );
        let suggestion = commit(&git, &proposed, Some(&genesis), 2).await;
        let decision_path = pigeon_core::authority::decision_path(&suggestion);
        let decision = DecisionDoc {
            proposal: suggestion.clone(),
            verdict,
            note: None,
        };
        tree.insert(
            decision_path,
            blob(&git, decision_file::render(&decision).as_bytes()).await,
        );
        let decided = commit(&git, &tree, Some(&genesis), decider).await;
        tree.insert(
            "Docs/deep/native.txt".into(),
            pointer(&git, b"native edit\n").await,
        );
        let later = commit(&git, &tree, Some(&decided), 1).await;

        // Bob is the proposal's author, but not a decider for Alice's file.
        let mut invalid = git.tree(&decided).await.unwrap();
        let rejected = DecisionDoc {
            verdict: Decided::Rejected,
            ..decision
        };
        invalid.insert(
            pigeon_core::authority::decision_path(&suggestion),
            blob(&git, decision_file::render(&rejected).as_bytes()).await,
        );
        let forged = commit(&git, &invalid, Some(&genesis), 2).await;

        // Main arrives before a proposal ref has been validated and retained.
        for tip in [&later, &forged] {
            let waiting = verify::judge(&group, Some(&genesis), tip).await.unwrap();
            assert!(matches!(
                waiting.stopped,
                Some((_, Verdict::Unknown("decision")))
            ));
        }
        assert!(!group.lock().engine.accepted(&decided));
        assert!(!group.lock().engine.accepted(&later));

        let device = sig::device_id(&[2; 32]).0;
        let remote = format!("refs/pigeon/peers/remote/proposals/{device}/{suggestion}");
        git.update_ref(&remote, &suggestion, None).await.unwrap();
        assert_eq!(proposal::take(&group, "remote").await, 1);

        // No restart or cache reset: a normal subsequent accept must retry.
        let retried = verify::judge(&group, Some(&genesis), &later).await.unwrap();
        assert_eq!(
            retried.accepted.as_deref(),
            Some(later.as_str()),
            "{verdict:?}: {retried:?}"
        );
        assert!(retried.stopped.is_none());
        assert!(group.lock().engine.accepted(&decided));
        assert!(group.lock().engine.accepted(&later));
        assert!(!group.lock().verdicts.contains_key(&decided));
        assert!(!group.lock().verdicts.contains_key(&later));

        // Knowing a proposal still cannot authorize an unrelated rejection.
        let judged = verify::judge(&group, Some(&genesis), &forged)
            .await
            .unwrap();
        assert!(judged.accepted.is_none());
        assert!(matches!(judged.stopped, Some((_, Verdict::Refused(_)))));
    }
}
