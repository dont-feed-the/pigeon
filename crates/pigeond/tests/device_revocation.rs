//! Exact-key revocation through real signed roster commits. Computer labels
//! and signing keys occupy separate namespaces; a label may look like another
//! computer's key without changing the selected device. No git-annex is needed.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, RwLock};

use pigeon_core::{
    authority,
    roster::{Device, Person, PersonId, Role, Roster},
    sig,
};
use pigeon_git::GitEnv;
use pigeon_proto::Response;
use pigeond::{
    daemon::{Daemon, dirs_under},
    git::{self, Git},
    group::Group,
    history, merge,
    net::{NetConfig, Node, Reach},
    roster_file,
    trace::Tracer,
};

#[tokio::test]
async fn exact_key_revocation_does_not_interpret_a_key_as_a_computer_name() {
    let temp = tempfile::tempdir().unwrap();
    let git = Git::new(temp.path(), GitEnv::default());
    git.run(&["init", "--bare", "-q", "-b", "main"])
        .await
        .unwrap();
    git.run(&["config", "user.name", "revocation test"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "revocation@pigeon.invalid"])
        .await
        .unwrap();
    git::set_signer([1; 32]);
    let owner = sig::device_id(&[1; 32]);
    let mut clients = [sig::device_id(&[2; 32]), sig::device_id(&[3; 32])];
    clients.sort();
    let (survivor, target) = (&clients[0], &clients[1]);
    let roster = Roster {
        rules: 1,
        group: "exact device revocation".into(),
        invitation: "a".repeat(64),
        k: 1,
        retention_days: 30,
        relay: None,
        telemetry_device: None,
        persons: [(
            PersonId("alice".into()),
            Person {
                name: "Alice".into(),
                admin: true,
            },
        )]
        .into(),
        devices: [owner.clone(), survivor.clone(), target.clone()]
            .into_iter()
            .map(|id| {
                let device = Device {
                    person: PersonId("alice".into()),
                    name: if id == *survivor {
                        target.0.clone()
                    } else if id == owner {
                        "Owner".into()
                    } else {
                        "Nextcloud desktop".into()
                    },
                    role: Role::Laptop,
                    annex_uuid: Some(format!("annex-{}", id.0)),
                    maxsize: None,
                };
                (id, device)
            })
            .collect(),
        removed: BTreeMap::new(),
        folders: vec![],
    };
    roster.validate().unwrap();
    let text = roster_file::render(&roster_file::RosterDoc { roster }).unwrap();
    let blob = git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await
        .unwrap();
    let tree = [(
        authority::ROSTER.into(),
        authority::Entry {
            mode: "100644".into(),
            kind: "blob".into(),
            oid: blob.trim().into(),
        },
    )]
    .into();
    let tree = merge::write_tree(&git, &tree).await.unwrap();
    let genesis = git
        .commit_tree(&tree, &[], "pigeon: exact-key fixture")
        .await
        .unwrap();
    git.update_ref("refs/heads/main", &genesis, None)
        .await
        .unwrap();
    let group = Arc::new(
        Group::open(&genesis, temp.path(), &GitEnv::default())
            .await
            .unwrap(),
    );
    let node = Node::bind(&NetConfig {
        secret_key: [1; 32],
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: Reach::Local,
    })
    .await
    .unwrap();
    let daemon = Daemon {
        dirs: dirs_under(&temp.path().join("home")),
        admissions: pigeond::admission_round::Store::open(
            temp.path().join("home/state/admission-acks.toml"),
        ),
        config: Default::default(),
        device: owner.0.clone(),
        env: GitEnv::default(),
        groups: RwLock::new(vec![group.clone()]),
        node,
        net: OnceLock::new(),
        web: OnceLock::new(),
        wake: Arc::default(),
        debug: Default::default(),
        tracer: Tracer::new(temp.path().join("traces"), &owner.0),
    };
    // A name-based lookup of this exact same string selects the survivor.
    // The ID operation must select the intended key instead.
    let removed = history::remove_device_id(&daemon, &target.0).await;
    assert!(
        matches!(removed.as_slice(), [Response::Said { .. }]),
        "{removed:?}"
    );
    let after = group.roster().unwrap().roster;
    assert!(!after.devices.contains_key(target));
    assert!(after.removed.contains_key(target));
    assert!(after.devices.contains_key(survivor));
    assert!(after.devices.contains_key(&owner));
    // Check both the durable signed tree and its reconstructed in-memory view.
    let reopened = Group::open(&genesis, temp.path(), &GitEnv::default())
        .await
        .unwrap();
    assert_eq!(reopened.roster().unwrap().roster, after);
    let tip = git.rev("refs/heads/main").await.unwrap();
    assert!(matches!(
        history::remove_device_id(&daemon, &target.0)
            .await
            .as_slice(),
        [Response::Done]
    ));
    assert_eq!(
        git.rev("refs/heads/main").await.as_deref(),
        Some(tip.as_str())
    );
    assert!(
        matches!(history::remove_device_id(&daemon, &owner.0).await.as_slice(),
                     [Response::Problem(p)] if p.code == "not-allowed")
    );
    assert!(matches!(
        history::remove_device(&daemon, &target.0).await.as_slice(),
        [Response::Said { .. }]
    ));
    assert!(
        !group
            .roster()
            .unwrap()
            .roster
            .devices
            .contains_key(survivor)
    );
}
