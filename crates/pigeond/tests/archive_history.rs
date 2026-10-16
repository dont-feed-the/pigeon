//! Refounded histories (09 §9.9, ADR 0047): history/view/restore see old
//! versions through signed archive links, deduplicate the boundary copy,
//! and never follow missing, substituted or unauthorized archive history.
//! An archive is judged separately from the current roster fold.
#![allow(clippy::unwrap_used, reason = "tests")]

use pigeon_core::authority::{Entry, ROSTER, Tree};
use pigeon_core::roster::{Device, Person, PersonId, Role, Roster};
use pigeon_git::GitEnv;
use pigeond::git::Git;
use pigeond::group::Group;
use pigeond::roster_file::{RosterDoc, render};

async fn blob(git: &Git, bytes: &[u8]) -> Entry {
    let oid = git
        .run_in(&["hash-object", "-w", "--stdin"], bytes)
        .await
        .unwrap();
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: oid.trim().into(),
    }
}

async fn commit(git: &Git, tree: &Tree, parent: Option<&str>, message: &str, key: u8) -> String {
    let tree = pigeond::merge::write_tree(git, tree).await.unwrap();
    let mut args = vec!["commit-tree", &tree, "-m", message];
    if let Some(parent) = parent {
        args.extend(["-p", parent]);
    }
    let unsigned = git.run(&args).await.unwrap();
    git.sign(unsigned.trim(), &[key; 32]).await.unwrap()
}

async fn setup() -> (tempfile::TempDir, Git, Tree) {
    let dir = tempfile::tempdir().unwrap();
    let git = Git::new(dir.path(), GitEnv::default());
    git.run(&["init", "--bare", "-q", "-b", "main"])
        .await
        .unwrap();
    git.run(&["config", "user.name", "Ana"]).await.unwrap();
    git.run(&["config", "user.email", "ana@pigeon"])
        .await
        .unwrap();
    let person = PersonId("ana".into());
    let roster = Roster {
        rules: 1,
        group: "Family".into(),
        k: 1,
        retention_days: 30,
        relay: None,
        telemetry_device: None,
        persons: [(
            person.clone(),
            Person {
                name: "Ana".into(),
                admin: true,
            },
        )]
        .into(),
        devices: [(
            pigeon_core::sig::device_id(&[1; 32]),
            Device {
                person,
                name: "laptop".into(),
                role: Role::Laptop,
                annex_uuid: Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e01".into()),
                maxsize: None,
            },
        )]
        .into(),
        removed: Default::default(),
        folders: Vec::new(),
    };
    let text = render(&RosterDoc { roster }).unwrap();
    let base = [(ROSTER.into(), blob(&git, text.as_bytes()).await)].into();
    (dir, git, base)
}

async fn file(git: &Git, tree: &mut Tree, n: u8) -> String {
    let key = format!("SHA256E-s1--{}.txt", format!("{n:02x}").repeat(32));
    let pointer = format!("/annex/objects/{key}\n");
    tree.insert("recipe.txt".into(), blob(git, pointer.as_bytes()).await);
    key
}

#[tokio::test]
async fn nested_refounding_keeps_old_versions_and_the_live_roster_fold() {
    let (dir, git, mut tree) = setup().await;
    let one = file(&git, &mut tree, 1).await;
    let genesis = commit(&git, &tree, None, "create", 1).await;
    let two = file(&git, &mut tree, 2).await;
    let saved = commit(&git, &tree, Some(&genesis), "save", 1).await;
    let first = commit(
        &git,
        &tree,
        None,
        &format!("refound\n\nPigeon-Refound: {genesis} {saved}"),
        1,
    )
    .await;
    let three = file(&git, &mut tree, 3).await;
    let saved_again = commit(&git, &tree, Some(&first), "save again", 1).await;
    let second = commit(
        &git,
        &tree,
        None,
        &format!("refound again\n\nPigeon-Refound: {first} {saved_again}"),
        1,
    )
    .await;
    git.update_ref("refs/heads/main", &second, None)
        .await
        .unwrap();
    git.update_ref(&pigeond::refound::archive_ref(&genesis), &saved, None)
        .await
        .unwrap();
    git.update_ref(&pigeond::refound::archive_ref(&first), &saved_again, None)
        .await
        .unwrap();
    let g = Group::open(&second, dir.path(), &GitEnv::default())
        .await
        .unwrap();
    assert_eq!(
        pigeond::verify::judge(&g, None, &second)
            .await
            .unwrap()
            .accepted,
        Some(second.clone())
    );
    let versions = pigeond::history::versions(&g, "recipe.txt").await;
    assert_eq!(
        versions,
        vec![
            (saved_again, three),
            (saved.clone(), two.clone()),
            (genesis.clone(), one)
        ]
    );
    assert_eq!(g.lock().history.as_ref().unwrap().genesis, second);
    // A file deleted before another refounding remains recoverable.
    tree.remove("recipe.txt");
    let deleted = commit(&git, &tree, Some(&second), "delete recipe", 1).await;
    let third = commit(
        &git,
        &tree,
        None,
        &format!("refound after deletion\n\nPigeon-Refound: {second} {deleted}"),
        1,
    )
    .await;
    git.update_ref(&pigeond::refound::archive_ref(&second), &deleted, None)
        .await
        .unwrap();
    git.update_ref("refs/heads/main", &third, Some(&second))
        .await
        .unwrap();
    let g = Group::open(&third, dir.path(), &GitEnv::default())
        .await
        .unwrap();
    assert_eq!(pigeond::history::versions(&g, "recipe.txt").await, versions);
    let deleted_files = pigeond::folders::deleted(&g, "").await;
    assert!(
        matches!(deleted_files, pigeon_proto::Response::Deleted { files, .. }
        if files.iter().any(|f| f.path == "recipe.txt"))
    );
    // A local archive ref can be substituted; its signed trailer cannot.
    git.update_ref(
        &pigeond::refound::archive_ref(&genesis),
        &genesis,
        Some(&saved),
    )
    .await
    .unwrap();
    assert_eq!(pigeond::history::versions(&g, "recipe.txt").await.len(), 2);
}

#[tokio::test]
async fn an_unauthorized_archive_tip_is_not_exposed_as_file_history() {
    let (dir, git, mut tree) = setup().await;
    file(&git, &mut tree, 1).await;
    let genesis = commit(&git, &tree, None, "create", 1).await;
    file(&git, &mut tree, 2).await;
    let forged = commit(&git, &tree, Some(&genesis), "unrecognized author", 2).await;
    let refounded = commit(
        &git,
        &tree,
        None,
        &format!("refound\n\nPigeon-Refound: {genesis} {forged}"),
        1,
    )
    .await;
    git.update_ref("refs/heads/main", &refounded, None)
        .await
        .unwrap();
    git.update_ref(&pigeond::refound::archive_ref(&genesis), &forged, None)
        .await
        .unwrap();
    let g = Group::open(&refounded, dir.path(), &GitEnv::default())
        .await
        .unwrap();
    assert_eq!(pigeond::history::versions(&g, "recipe.txt").await.len(), 1);
}
