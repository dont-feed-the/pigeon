//! A lost welcome can be retried by the same authenticated key, without
//! reopening its invitation to another key, role, UUID or revoked member
//! (09 §9.6). Old and unbound records grant no new-group membership.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::*;
use pigeon_proto::join::{JOIN_VERSION, Join, JoinAnswer};
use pigeond::invite::{Invitation, decode, proof};
use pigeond::net::{NetConfig, Node, Reach};

static JOIN_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn node(key: u8) -> Node {
    Node::bind(&NetConfig {
        secret_key: [key; 32],
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: Reach::Local,
    })
    .await
    .unwrap()
}

fn request(node: &Node, inv: &Invitation) -> Join {
    let secret = pigeond::util::unhex(&inv.secret).unwrap();
    Join {
        v: JOIN_VERSION,
        traceparent: None,
        invitation_hash: pigeond::util::sha256_hex(&secret),
        proof: proof(&secret, &node.device_id(), &inv.inviter),
        device_name: "retry-laptop".into(),
        role: "laptop".into(),
        annex_uuid: "7b2234d2-aa26-4939-929f-3ce63bb816ae".into(),
        capacity: None,
        addresses: Vec::new(),
        new_device: None,
        recovery_commit: None,
        recovery_addresses: Vec::new(),
        recovery_version: None,
    }
}

async fn send(node: &Node, inv: &Invitation, request: &Join) -> JoinAnswer {
    node.join(
        &inv.inviter,
        inv.addresses
            .iter()
            .filter_map(|a| a.parse().ok())
            .collect(),
        request,
    )
    .await
    .unwrap()
}

fn refused(answer: JoinAnswer, expected: &str) {
    assert!(
        matches!(answer, JoinAnswer::Refused { ref code } if code == expected),
        "{answer:?}"
    );
}

fn invitation(lap: &Daemon, name: &str) -> Invitation {
    let answer = pigeon(lap, &["invite", name]);
    decode(&said(&answer, "invitation", "code").unwrap()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_the_original_current_device_can_resume_its_welcome() {
    let _one = JOIN_SERIAL.lock().await;
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    let answer = pigeon(&lap, &["create", "Family", folder.to_str().unwrap()]);
    assert!(said(&answer, "created", "group").is_some(), "{answer:?}");
    let inv = invitation(&lap, "Bob");
    let client = node(93).await;
    let request = request(&client, &inv);
    let first = send(&client, &inv, &request).await;
    assert!(matches!(first, JoinAnswer::Welcome { .. }), "{first:?}");
    let again = send(&client, &inv, &request).await;
    assert!(matches!(again, JoinAnswer::Welcome { .. }), "{again:?}");

    let mut changed = request.clone();
    changed.annex_uuid = "b7d386db-02c0-496a-8e63-ee75d64ceead".into();
    refused(send(&client, &inv, &changed).await, "invite-not-allowed");
    changed = request.clone();
    changed.role = "custodian".into();
    refused(send(&client, &inv, &changed).await, "invite-not-allowed");
    let outsider = node(94).await;
    let mut stolen = request.clone();
    stolen.proof = proof(
        &pigeond::util::unhex(&inv.secret).unwrap(),
        &outsider.device_id(),
        &inv.inviter,
    );
    refused(send(&outsider, &inv, &stolen).await, "invite-used");
    let removed = pigeon(&lap, &["remove-computer", "retry-laptop", "--yes"]);
    assert!(
        !removed
            .iter()
            .any(|r| matches!(r, pigeon_proto::Response::Problem(_))),
        "{removed:?}"
    );
    refused(send(&client, &inv, &request).await, "invite-not-allowed");
    client.close().await;
    outsider.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_persisted_first_use_finishes_but_unbound_and_old_group_tickets_refuse() {
    let _one = JOIN_SERIAL.lock().await;
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    let answer = pigeon(&lap, &["create", "Family", folder.to_str().unwrap()]);
    assert!(said(&answer, "created", "group").is_some(), "{answer:?}");
    let client = node(95).await;
    let inv = invitation(&lap, "Pending");
    let file = folder.join(".git/pigeon/invitations.json");
    let mut records: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    records["invitations"][0]["used_by"] = client.device_id().into();
    pigeond::util::write_private(&file, &serde_json::to_vec(&records).unwrap()).unwrap();
    let resumed = send(&client, &inv, &request(&client, &inv)).await;
    assert!(matches!(resumed, JoinAnswer::Welcome { .. }), "{resumed:?}");

    let legacy = invitation(&lap, "Legacy");
    let mut records: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    for record in records["invitations"].as_array_mut().unwrap() {
        if record["secret"] == legacy.secret {
            record.as_object_mut().unwrap().remove("group");
        }
    }
    pigeond::util::write_private(&file, &serde_json::to_vec(&records).unwrap()).unwrap();
    refused(
        send(&client, &legacy, &request(&client, &legacy)).await,
        "invite-unknown",
    );

    let old = invitation(&lap, "Old group");
    let answer = pigeon(&lap, &["refound", folder.to_str().unwrap(), "--yes"]);
    assert!(said(&answer, "refounded", "group").is_some(), "{answer:?}");
    refused(
        send(&client, &old, &request(&client, &old)).await,
        "invite-unknown",
    );
    client.close().await;
}
