//! A suggestion of several files decided file by file (ADR 0078, 04 §4.5),
//! on signed git objects without daemons: Bob suggests changes of three of
//! Alice's files; Alice accepts two and declines the third with a note, in
//! one commit on `main` that any release's judge accepts. Bob's computer
//! reads which files were taken and which were not; a claim of a file the
//! commit does not hold is not believed; Bob cannot decide his own
//! suggestion that way; and a computer that joins later verifies the
//! history from the signed objects.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::authority::{self, Decided, Entry, Tree};
use pigeon_core::roster::{Device, Person, PersonId, Role, Roster};
use pigeon_core::sig;
use pigeon_git::GitEnv;
use pigeond::decision_file::{self, DecisionDoc};
use pigeond::git::Git;
use pigeond::group::Group;
use pigeond::{decide, decide_files, decided, merge, proposal, roster_file, verify};
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
        format!("/annex/objects/SHA256E-s{}--{hash}.txt\n", bytes.len()).as_bytes(),
    )
    .await
}

/// A commit of `tree` on `parent`, signed with key `signer`, saying
/// `message`.
async fn commit(git: &Git, tree: &Tree, parent: Option<&str>, signer: u8, message: &str) -> String {
    let root = merge::write_tree(git, tree).await.unwrap();
    let parent = parent.map_or(String::new(), |id| format!("parent {id}\n"));
    let body = format!(
        "tree {root}\n{parent}author test <test@pigeon> 1791892800 +0000\ncommitter test <test@pigeon> 1791892800 +0000\n\n{message}\n"
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
        group: "parts".into(),
        invitation: "0".repeat(64),
        k: 1,
        retention_days: 30,
        relay: None,
        telemetry_device: None,
        persons,
        devices,
        removed: BTreeMap::new(),
        folders: vec![],
    }
}

/// A bare repository at `dir` whose commits have an author, as the
/// daemon's do.
async fn repo(dir: &std::path::Path) -> Git {
    let git = Git::new(dir, GitEnv::default());
    git.run(&["init", "-q", "--bare", "-b", "main"])
        .await
        .unwrap();
    git.run(&["config", "user.name", "pigeon"]).await.unwrap();
    git.run(&["config", "user.email", "pigeon@localhost"])
        .await
        .unwrap();
    git
}

fn set(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|p| (*p).to_owned()).collect()
}

#[tokio::test]
async fn some_files_of_a_suggestion_are_accepted_and_the_others_declined() {
    // This test's daemon is Alice's computer.
    pigeond::git::set_signer([1; 32]);
    let temp = tempfile::tempdir().unwrap();
    let git = repo(temp.path()).await;
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
    ]
    .into();
    for name in ["recipes", "groceries", "soup"] {
        let content = format!("Alice's {name}\n");
        tree.insert(
            format!("Kitchen/{name}.txt"),
            pointer(&git, content.as_bytes()).await,
        );
    }
    let genesis = commit(&git, &tree, None, 1, "pigeon: create").await;
    git.update_ref("refs/heads/main", &genesis, None)
        .await
        .unwrap();
    let group = Group::open(&genesis, temp.path(), &GitEnv::default())
        .await
        .unwrap();

    // Bob changes two of Alice's files and deletes the third: a suggestion
    // to the admins, since the default rule lets only them change these.
    let mut suggested = tree.clone();
    suggested.insert(
        "Kitchen/recipes.txt".into(),
        pointer(&git, b"Bob's recipes\n").await,
    );
    suggested.insert(
        "Kitchen/groceries.txt".into(),
        pointer(&git, b"Bob's groceries\n").await,
    );
    suggested.remove("Kitchen/soup.txt");
    let suggestion = commit(&git, &suggested, Some(&genesis), 2, "pigeon: suggest").await;
    let bob = sig::device_id(&[2; 32]).0;
    git.update_ref(
        &format!("refs/pigeon/peers/bob/proposals/{bob}/{suggestion}"),
        &suggestion,
        None,
    )
    .await
    .unwrap();
    assert_eq!(proposal::take(&group, "bob").await, 1);
    let pending = decide::pending(&group).await.unwrap();
    assert_eq!(pending.len(), 1);
    let p = &pending[0];
    assert_eq!(
        p.paths,
        [
            "Kitchen/groceries.txt",
            "Kitchen/recipes.txt",
            "Kitchen/soup.txt"
        ]
    );
    // Each file says what changes in it.
    let files = pigeond::suggestion_files::files(&group, &p.at, &p.tree).await;
    let changes: Vec<_> = files.iter().map(|f| (f.path.as_str(), f.change)).collect();
    use pigeon_proto::api::FileChange;
    assert_eq!(
        changes,
        [
            ("Kitchen/groceries.txt", FileChange::Changed),
            ("Kitchen/recipes.txt", FileChange::Changed),
            ("Kitchen/soup.txt", FileChange::Deleted)
        ]
    );
    assert_eq!(files[1].before, Some(16));
    assert_eq!(files[1].after, Some(14));

    // Bob may not decide his own suggestion by writing its change himself.
    let mut own = suggested.clone();
    let (path, entry) = (
        authority::decision_path(&suggestion),
        blob(
            &git,
            decision_file::render(&DecisionDoc {
                proposal: suggestion.clone(),
                verdict: Decided::Rejected,
                note: None,
            })
            .as_bytes(),
        )
        .await,
    );
    own.insert(path, entry);
    let forged = commit(&git, &own, Some(&genesis), 2, "pigeon: accept part").await;
    let judged = verify::judge(&group, Some(&genesis), &forged)
        .await
        .unwrap();
    assert!(judged.accepted.is_none(), "{judged:?}");

    // Alice accepts the recipes and the deletion of the soup, and declines
    // the groceries with a note.
    let accepted = set(&["Kitchen/recipes.txt", "Kitchen/soup.txt"]);
    let decided_at = decide_files::part(
        &group,
        p,
        &accepted,
        Some("On garde la liste de Mamie."),
        None,
    )
    .await
    .unwrap();
    let main = git.rev("refs/heads/main").await.unwrap();
    assert_eq!(main, decided_at);
    assert!(group.lock().engine.accepted(&main));
    let now = git.tree(&main).await.unwrap();
    assert_eq!(
        now.get("Kitchen/recipes.txt"),
        suggested.get("Kitchen/recipes.txt")
    );
    assert!(!now.contains_key("Kitchen/soup.txt"));
    assert_eq!(
        now.get("Kitchen/groceries.txt"),
        tree.get("Kitchen/groceries.txt")
    );
    // An ordinary commit, not a merge: the suggestion is not adopted, and
    // its decision is the format an older release reads, a decline.
    let raw = git.object(&main).await.unwrap();
    assert_eq!(authority::parents(&raw), std::slice::from_ref(&genesis));
    assert!(!git.contains(&main, &suggestion).await);
    let doc = git
        .blob(&format!("{main}:{}", authority::decision_path(&suggestion)))
        .await
        .unwrap();
    let doc = decision_file::parse(&String::from_utf8_lossy(&doc)).unwrap();
    assert_eq!(doc.verdict, Decided::Rejected);
    assert_eq!(doc.note.as_deref(), Some("On garde la liste de Mamie."));
    assert_eq!(
        authority::accepted(&raw),
        ["Kitchen/recipes.txt", "Kitchen/soup.txt"]
    );
    assert!(decide::pending(&group).await.unwrap().is_empty());

    // Bob's computer is told which files were taken and which were not.
    let told = decided::outcomes(&group, &PersonId("bob".into())).await;
    assert_eq!(told.len(), 1, "{told:?}");
    assert_eq!(told[0].by, "alice");
    assert_eq!(
        told[0].accepted,
        ["Kitchen/recipes.txt", "Kitchen/soup.txt"]
    );
    assert_eq!(told[0].declined, ["Kitchen/groceries.txt"]);
    assert_eq!(told[0].note.as_deref(), Some("On garde la liste de Mamie."));
    assert!(
        decided::outcomes(&group, &PersonId("alice".into()))
            .await
            .is_empty()
    );

    // A computer that joins later verifies the whole history from the
    // signed objects: Bob's suggestion and Alice's decision on it.
    let joined = tempfile::tempdir().unwrap();
    let fresh_git = Git::new(joined.path(), GitEnv::default());
    fresh_git
        .run(&["init", "-q", "--bare", "-b", "main"])
        .await
        .unwrap();
    let source = temp.path().to_str().unwrap();
    fresh_git
        .run(&[
            "fetch",
            "-q",
            source,
            "+refs/heads/main:refs/pigeon/peers/source/main",
        ])
        .await
        .unwrap();
    let fresh = Group::open(&genesis, joined.path(), &GitEnv::default())
        .await
        .unwrap();
    // The decision cannot be verified without the suggestion it names...
    assert!(
        verify::bootstrap(&fresh, &genesis, &main, &genesis, &bob)
            .await
            .is_err()
    );
    // ...which Alice's computer relays as it keeps it.
    fresh_git
        .run(&["fetch", "-q", source, &proposal::refspec("source")])
        .await
        .unwrap();
    assert_eq!(proposal::take(&fresh, "source").await, 1);
    verify::bootstrap(&fresh, &genesis, &main, &genesis, &bob)
        .await
        .unwrap();
    assert!(fresh.lock().engine.accepted(&main));
    // Once it takes that main, it reads the same decision, as Bob's own
    // computer would.
    fresh_git
        .update_ref("refs/heads/main", &main, None)
        .await
        .unwrap();
    let fresh = Group::open(&genesis, joined.path(), &GitEnv::default())
        .await
        .unwrap();
    assert!(decide::pending(&fresh).await.unwrap().is_empty());
    let there = decided::outcomes(&fresh, &PersonId("bob".into())).await;
    assert_eq!(there, told);
}

#[tokio::test]
async fn a_claim_of_a_file_the_decision_does_not_hold_is_not_believed() {
    pigeond::git::set_signer([1; 32]);
    let temp = tempfile::tempdir().unwrap();
    let git = repo(temp.path()).await;
    let text = roster_file::render(&roster_file::RosterDoc { roster: roster() }).unwrap();
    let tree: Tree = [
        (
            ".pigeon/roster.toml".into(),
            blob(&git, text.as_bytes()).await,
        ),
        (
            ".gitattributes".into(),
            blob(&git, pigeond::settings::GITATTRIBUTES.as_bytes()).await,
        ),
        ("a.txt".into(), pointer(&git, b"Alice's a\n").await),
        ("b.txt".into(), pointer(&git, b"Alice's b\n").await),
    ]
    .into();
    let genesis = commit(&git, &tree, None, 1, "pigeon: create").await;
    git.update_ref("refs/heads/main", &genesis, None)
        .await
        .unwrap();
    let group = Group::open(&genesis, temp.path(), &GitEnv::default())
        .await
        .unwrap();
    let mut suggested = tree.clone();
    suggested.insert("a.txt".into(), pointer(&git, b"Bob's a\n").await);
    suggested.insert("b.txt".into(), pointer(&git, b"Bob's b\n").await);
    let suggestion = commit(&git, &suggested, Some(&genesis), 2, "pigeon: suggest").await;
    let bob = sig::device_id(&[2; 32]).0;
    git.update_ref(
        &format!("refs/pigeon/peers/bob/proposals/{bob}/{suggestion}"),
        &suggestion,
        None,
    )
    .await
    .unwrap();
    assert_eq!(proposal::take(&group, "bob").await, 1);

    // Alice's decision takes a.txt but claims b.txt too.
    let mut decided_tree = tree.clone();
    decided_tree.insert("a.txt".into(), suggested["a.txt"].clone());
    let doc = DecisionDoc {
        proposal: suggestion.clone(),
        verdict: Decided::Rejected,
        note: None,
    };
    decided_tree.insert(
        authority::decision_path(&suggestion),
        blob(&git, decision_file::render(&doc).as_bytes()).await,
    );
    let message = format!(
        "pigeon: accept part of a suggestion\n\n{}\n{}",
        authority::accepted_trailer("a.txt"),
        authority::accepted_trailer("b.txt")
    );
    let claim = commit(&git, &decided_tree, Some(&genesis), 1, &message).await;
    let judged = verify::judge(&group, Some(&genesis), &claim).await.unwrap();
    assert_eq!(
        judged.accepted.as_deref(),
        Some(claim.as_str()),
        "{judged:?}"
    );
    git.update_ref("refs/heads/main", &claim, Some(&genesis))
        .await
        .unwrap();
    let told = decided::outcomes(&group, &PersonId("bob".into())).await;
    assert_eq!(told.len(), 1, "{told:?}");
    assert_eq!(told[0].accepted, ["a.txt"]);
    assert_eq!(told[0].declined, ["b.txt"]);
}
