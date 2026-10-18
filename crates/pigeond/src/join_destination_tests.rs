//! Reject received default paths before creating a repository, while retaining
//! explicit destination choice (09 §9.6, §9.9).
#![allow(
    clippy::unwrap_used,
    reason = "isolated destination ownership fixtures"
)]

use crate::{daemon::Daemon, invite::Joining, recovery::Sheet};
use pigeon_core::roster::Role;
use std::sync::{Arc, OnceLock, RwLock};

async fn fixture() -> (tempfile::TempDir, Daemon) {
    let root = tempfile::tempdir().unwrap();
    let dirs = crate::daemon::dirs_under(root.path());
    std::fs::create_dir_all(&dirs.config).unwrap();
    let node = crate::net::Node::bind(&crate::net::NetConfig {
        secret_key: [9; 32],
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: crate::net::Reach::Local,
    })
    .await
    .unwrap();
    let device = node.device_id();
    let tracer = crate::trace::Tracer::new(dirs.traces(), &device);
    let daemon = Daemon {
        admissions: crate::admission_round::Store::open(dirs.state.join("admission.toml")),
        dirs,
        config: toml_edit::de::from_str(
            "[network]\nbind = '127.0.0.1:0'\nmdns = false\nrelays = 'off'",
        )
        .unwrap(),
        device,
        env: pigeon_git::GitEnv::default(),
        groups: RwLock::new(Vec::new()),
        node,
        net: OnceLock::new(),
        web: OnceLock::new(),
        wake: Arc::new(tokio::sync::Notify::new()),
        debug: crate::debug::Permissions::default(),
        tracer,
    };
    (root, daemon)
}

fn joining(name: String, group: String, role: Role) -> Joining {
    Joining {
        inv: pigeon_proto::join::Invitation {
            v: pigeon_proto::join::INVITATION_VERSION,
            group,
            name,
            k: 1,
            admitters: Vec::new(),
            secret: "00".repeat(32),
            relay: None,
            refounded: None,
        },
        person: "New person".into(),
        existing: false,
        role,
        name: "New computer".into(),
    }
}

#[tokio::test]
async fn received_absolute_names_cannot_choose_a_default_laptop_folder() {
    let (root, daemon) = fixture().await;
    let outside = root.path().join("not-the-home-default");
    let request = joining(outside.display().to_string(), "a".repeat(40), Role::Laptop);
    let result = crate::join::join(&daemon, &request, None).await;
    assert_eq!(result, Err("join-folder-needed".into()));
    assert!(
        !outside.exists(),
        "received name created an absolute destination"
    );
}

#[tokio::test]
async fn received_group_ids_cannot_escape_the_backup_configuration_folder() {
    let (root, daemon) = fixture().await;
    let outside = root.path().join("escaped.git");
    let request = joining("Family".into(), "../escaped".into(), Role::Custodian);
    let result = crate::join::join(&daemon, &request, None).await;
    assert_eq!(result, Err("invite-unknown".into()));
    assert!(
        !outside.exists(),
        "received group ID created a parent destination"
    );
}

#[tokio::test]
async fn received_sheet_names_cannot_choose_a_default_recovery_folder() {
    let (root, daemon) = fixture().await;
    let outside = root.path().join("not-the-recovery-default");
    let sheet = sheet(outside.display().to_string(), "a".repeat(40));
    let result =
        crate::recovery::recover(&daemon, &crate::recovery::encode(&sheet), None, None).await;
    assert!(matches!(
        result.first(),
        Some(pigeon_proto::Response::Problem(p)) if p.code == "join-folder-needed"
    ));
    assert!(
        !outside.exists(),
        "received sheet created an absolute destination"
    );
}

fn sheet(name: String, group: String) -> Sheet {
    Sheet {
        v: 1,
        group,
        name,
        k: 1,
        person: "Admin".into(),
        label: String::new(),
        key: "00".repeat(32),
        custodians: Vec::new(),
    }
}

#[tokio::test]
async fn explicit_folders_remain_the_persons_choice_for_join_and_recovery() {
    for recovery in [false, true] {
        let (root, daemon) = fixture().await;
        let received = root.path().join("received-default");
        let chosen = root.path().join("chosen-by-the-person");
        let name = received.display().to_string();
        if recovery {
            let sheet = sheet(name, "a".repeat(40));
            let _ = crate::recovery::recover(
                &daemon,
                &crate::recovery::encode(&sheet),
                Some(chosen.display().to_string()),
                None,
            )
            .await;
        } else {
            let request = joining(name, "a".repeat(40), Role::Laptop);
            assert!(
                crate::join::join(&daemon, &request, Some(chosen.display().to_string()))
                    .await
                    .is_err()
            );
        }
        assert!(!received.exists());
        assert!(chosen.join(".git").is_dir());
    }
}

#[tokio::test]
async fn invalid_group_ids_refuse_even_when_a_folder_was_chosen_explicitly() {
    for recovery in [false, true] {
        let (root, daemon) = fixture().await;
        let chosen = root.path().join("not-created");
        if recovery {
            let sheet = sheet("Family".into(), "../not-a-genesis".into());
            let result = crate::recovery::recover(
                &daemon,
                &crate::recovery::encode(&sheet),
                Some(chosen.display().to_string()),
                None,
            )
            .await;
            assert!(
                matches!(result.first(), Some(pigeon_proto::Response::Problem(p)) if p.code == "sheet-unreadable")
            );
        } else {
            let request = joining("Family".into(), "../not-a-genesis".into(), Role::Custodian);
            assert_eq!(
                crate::join::join(&daemon, &request, Some(chosen.display().to_string())).await,
                Err("invite-unknown".into())
            );
        }
        assert!(!chosen.exists());
    }
}

#[hegel::test(test_cases = 1000)]
fn a_portable_single_name_keeps_its_spelling_and_parent(tc: hegel::TestCase) {
    use hegel::generators as gs;
    let base = std::path::PathBuf::from(format!("/home/person-{}", tc.draw(gs::integers::<u16>())));
    let name = format!("Famille é {}", tc.draw(gs::integers::<u64>()));
    let selected = super::default_folder(&base, &name).unwrap();
    assert_eq!(selected.parent(), Some(base.as_path()));
    assert_eq!(
        selected.file_name().and_then(|n| n.to_str()),
        Some(name.as_str())
    );
}

#[hegel::test(test_cases = 1000)]
fn paths_and_nonportable_received_names_never_supply_a_default(tc: hegel::TestCase) {
    use hegel::generators as gs;
    let name = format!("family-{}", tc.draw(gs::integers::<u64>()));
    let input = match tc.draw(gs::integers::<u8>().max_value(11)) {
        0 => format!("../{name}"),
        1 => format!("/{name}"),
        2 => format!("x/{name}"),
        3 => format!("x\\{name}"),
        4 => "..".into(),
        5 => "".into(),
        6 => format!("C:{name}"),
        7 => "CON".into(),
        8 => format!("{name}/.."),
        9 => format!("{name}\0"),
        10 => ".".into(),
        _ => "x".repeat(pigeon_core::names::MAX_COMPONENT + 1),
    };
    assert_eq!(
        super::default_folder(std::path::Path::new("/home/person"), &input),
        Err("join-folder-needed".into())
    );
}
