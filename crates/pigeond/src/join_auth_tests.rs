//! Unauthenticated joins must not queue behind a busy repository (09 §9.6).
//! Preflight authentication is repeated after that wait before any edit.
#![allow(clippy::unwrap_used, reason = "isolated admission lock regressions")]

use crate::{daemon::Daemon, group::Group};
use pigeon_proto::join::{Join, JoinAnswer};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

async fn fixture() -> (tempfile::TempDir, Arc<Daemon>, Arc<Group>) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("backup.git");
    std::fs::create_dir(&repo).unwrap();
    let env = pigeon_git::GitEnv::default();
    let git = crate::git::Git::new(&repo, env.clone());
    git.run(&["init", "--bare", "-q", "-b", "main"])
        .await
        .unwrap();
    let group = Arc::new(Group::open(&"a".repeat(40), &repo, &env).await.unwrap());
    group.lock().roster = Some(
        crate::roster_file::parse(include_str!(
            "../../../fixtures/formats/roster/known/group.toml"
        ))
        .unwrap(),
    );
    let node = crate::net::Node::bind(&crate::net::NetConfig {
        secret_key: [1; 32],
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: crate::net::Reach::Local,
    })
    .await
    .unwrap();
    let dirs = pigeon_sys::paths::Dirs {
        config: temp.path().join("config"),
        state: temp.path().join("state"),
        cache: temp.path().join("cache"),
        runtime: temp.path().join("run"),
    };
    let device = node.device_id();
    let tracer = crate::trace::Tracer::new(dirs.traces(), &device);
    let daemon = Arc::new(Daemon {
        admissions: crate::admission_round::Store::open(dirs.state.join("admission.toml")),
        dirs,
        config: crate::config::Config::default(),
        device,
        env,
        groups: RwLock::new(vec![group.clone()]),
        node,
        net: OnceLock::new(),
        web: OnceLock::new(),
        wake: Arc::new(tokio::sync::Notify::new()),
        debug: crate::debug::Permissions::default(),
        tracer,
    });
    (temp, daemon, group)
}

fn request() -> Join {
    serde_json::from_value(serde_json::json!({
        "v": pigeon_proto::join::JOIN_VERSION,
        "invitation_hash": "unknown invitation", "proof": "invalid proof",
        "person": "new person", "device_name": "new laptop", "role": "laptop",
        "annex_uuid": "fixture-uuid"
    }))
    .unwrap()
}

fn refused(answer: JoinAnswer, expected: &str) {
    assert!(matches!(answer, JoinAnswer::Refused { code } if code == expected));
}

#[tokio::test]
async fn invalid_invitation_proofs_do_not_wait_for_a_busy_group() {
    let (_temp, d, g) = fixture().await;
    let busy = g.busy.lock().await;
    let outsider = pigeon_core::sig::device_id(&[22; 32]).0;
    let mut join = request();
    let answer = tokio::time::timeout(
        Duration::from_millis(500),
        crate::invite::welcome(&d, &outsider, join.clone()),
    )
    .await
    .unwrap();
    refused(answer, "invite-unknown");
    let secret = crate::util::unhex(&g.roster().unwrap().roster.invitation).unwrap();
    join.invitation_hash = crate::util::sha256_hex(&secret);
    let answer = tokio::time::timeout(
        Duration::from_millis(500),
        crate::invite::welcome(&d, &outsider, join),
    )
    .await
    .unwrap();
    refused(answer, "invite-unknown");
    drop(busy);
    d.node.close().await;
}

#[tokio::test]
async fn unknown_recovery_identities_do_not_wait_for_a_busy_group() {
    let (_temp, d, g) = fixture().await;
    let busy = g.busy.lock().await;
    let outsider = pigeon_core::sig::device_id(&[23; 32]).0;
    let new = pigeon_core::sig::device_id(&[24; 32]).0;
    let mut join = request();
    join.new_device = Some(new.clone());
    join.recovery_version = Some(1);
    let answer = tokio::time::timeout(
        Duration::from_millis(500),
        crate::invite::welcome(&d, &outsider, join),
    )
    .await
    .unwrap();
    refused(answer, "invite-unknown");
    let bundle = tokio::time::timeout(
        Duration::from_millis(500),
        crate::recovery_exchange::bundle(&d, &g.id, &outsider),
    )
    .await
    .unwrap();
    assert!(bundle.is_none());
    drop(busy);
    d.node.close().await;
}

#[tokio::test]
async fn a_recovery_revoked_while_waiting_is_not_authorized_from_preflight() {
    let (_temp, d, g) = fixture().await;
    let doc = g.roster().unwrap();
    let signer = doc
        .roster
        .devices
        .iter()
        .find(|(_, device)| device.role == pigeon_core::roster::Role::Recovery)
        .unwrap()
        .0
        .clone();
    let new = pigeon_core::sig::device_id(&[25; 32]).0;
    let mut join = request();
    join.recovery_version = Some(1);
    let busy = g.busy.lock().await;
    let mut waiting = Box::pin(crate::recovery_exchange::welcome(
        &d, &signer.0, &new, &join,
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut waiting)
            .await
            .is_err()
    );
    let mut changed = doc;
    let removed = changed.roster.devices.remove(&signer).unwrap();
    changed
        .roster
        .removed
        .insert(signer.clone(), removed.annex_uuid);
    g.lock().roster = Some(changed);
    drop(busy);
    let answer = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .unwrap();
    refused(answer, "invite-unknown");
    d.node.close().await;
}

#[tokio::test]
async fn an_invitation_device_removed_while_waiting_cannot_reuse_the_old_roster() {
    let (_temp, d, g) = fixture().await;
    let doc = g.roster().unwrap();
    let (device, entry) = doc
        .roster
        .devices
        .iter()
        .find(|(_, device)| device.name == "Ben's laptop")
        .unwrap();
    let mut join = request();
    join.person = "Ben".into();
    join.device_name = entry.name.clone();
    join.annex_uuid = entry.annex_uuid.clone().unwrap();
    join.existing = true;
    let secret = crate::util::unhex(&doc.roster.invitation).unwrap();
    join.invitation_hash = crate::util::sha256_hex(&secret);
    join.proof = crate::invite::proof(&secret, &device.0, &d.device);
    let busy = g.busy.lock().await;
    let mut waiting = Box::pin(crate::invite::welcome(&d, &device.0, join));
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut waiting)
            .await
            .is_err()
    );
    let mut changed = doc.clone();
    let removed = changed.roster.devices.remove(device).unwrap();
    changed
        .roster
        .removed
        .insert(device.clone(), removed.annex_uuid);
    g.lock().roster = Some(changed);
    drop(busy);
    let answer = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .unwrap();
    refused(answer, "invite-not-allowed");
    d.node.close().await;
}
