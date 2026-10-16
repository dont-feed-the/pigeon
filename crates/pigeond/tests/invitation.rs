//! The group's invitation on the wire (09 §9.6, ADR 0075): it admits every
//! device that proves it, again and again; a device retrying its welcome
//! gets it again only with its own role and annex UUID; a proof binds the
//! device that made it; a name of the group joins its person only once the
//! device agrees; a removed device and a refounded group's old invitation
//! refuse.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::*;
use pigeon_proto::join::{Invitation, JOIN_VERSION, Join, JoinAnswer, decode};
use pigeond::invite::proof;
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

/// The join `node` sends as `person`, named `name`.
fn request(node: &Node, inv: &Invitation, person: &str, name: &str) -> Join {
    let secret = pigeond::util::unhex(&inv.secret).unwrap();
    Join {
        v: JOIN_VERSION,
        traceparent: None,
        invitation_hash: pigeond::util::sha256_hex(&secret),
        proof: proof(&secret, &node.device_id(), &inv.admitters[0].device),
        person: person.into(),
        existing: false,
        device_name: name.into(),
        role: "laptop".into(),
        annex_uuid: format!("7b2234d2-aa26-4939-929f-{}", &node.device_id()[..12]),
        capacity: None,
        addresses: Vec::new(),
        new_device: None,
        recovery_commit: None,
        recovery_addresses: Vec::new(),
        recovery_version: None,
    }
}

async fn send(node: &Node, inv: &Invitation, request: &Join) -> JoinAnswer {
    let admitter = &inv.admitters[0];
    node.join(
        &admitter.device,
        admitter
            .addresses
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

fn welcomed(answer: &JoinAnswer) -> bool {
    matches!(answer, JoinAnswer::Welcome { .. })
}

fn group(lap: &Daemon) -> Invitation {
    decode(&invitation(lap)).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_invitation_admits_again_and_only_as_the_device_that_proves_it() {
    let _one = JOIN_SERIAL.lock().await;
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    let answer = pigeon(&lap, &["create", "Family", folder.to_str().unwrap()]);
    assert!(said(&answer, "created", "group").is_some(), "{answer:?}");
    let inv = group(&lap);
    let bob = node(93).await;
    let asked = request(&bob, &inv, "Bob", "bob-laptop");
    assert!(welcomed(&send(&bob, &inv, &asked).await));
    assert!(
        welcomed(&send(&bob, &inv, &asked).await),
        "a retry is welcomed"
    );

    let mut changed = asked.clone();
    changed.annex_uuid = "b7d386db-02c0-496a-8e63-ee75d64ceead".into();
    refused(send(&bob, &inv, &changed).await, "invite-not-allowed");
    changed = asked.clone();
    changed.role = "custodian".into();
    refused(send(&bob, &inv, &changed).await, "invite-not-allowed");

    let outsider = node(94).await;
    refused(send(&outsider, &inv, &asked).await, "invite-unknown");

    let cara = node(95).await;
    let as_bob = request(&cara, &inv, "bob", "cara-laptop");
    refused(send(&cara, &inv, &as_bob).await, "person-exists");
    let as_cara = request(&cara, &inv, "Cara", "cara-laptop");
    assert!(welcomed(&send(&cara, &inv, &as_cara).await), "reused");
    match pigeon(&lap, &["people"]).as_slice() {
        [pigeon_proto::Response::People { people }] => {
            let bob = people.iter().find(|p| p.name == "Bob").unwrap();
            assert!(!bob.admin, "{people:?}");
            assert!(people.iter().any(|p| p.name == "Cara"), "{people:?}");
        }
        other => panic!("people: {other:?}"),
    }

    let dan = node(96).await;
    let mut as_ana = request(&dan, &inv, "ana", "dan-laptop");
    refused(send(&dan, &inv, &as_ana).await, "person-exists");
    as_ana.existing = true;
    assert!(welcomed(&send(&dan, &inv, &as_ana).await), "joins Ana");

    let removed = pigeon(&lap, &["remove-computer", "bob-laptop", "--yes"]);
    assert!(
        !removed
            .iter()
            .any(|r| matches!(r, pigeon_proto::Response::Problem(_))),
        "{removed:?}"
    );
    refused(send(&bob, &inv, &asked).await, "invite-not-allowed");
    for n in [bob, outsider, cara, dan] {
        n.close().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refounded_group_has_a_new_invitation() {
    let _one = JOIN_SERIAL.lock().await;
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let lap = start(&root, sockets.path(), "lap");
    let folder = root.join("files");
    std::fs::create_dir(&folder).unwrap();
    let answer = pigeon(&lap, &["create", "Family", folder.to_str().unwrap()]);
    assert!(said(&answer, "created", "group").is_some(), "{answer:?}");
    let old = group(&lap);
    let answer = pigeon(&lap, &["refound", folder.to_str().unwrap(), "--yes"]);
    assert!(said(&answer, "refounded", "group").is_some(), "{answer:?}");
    let new = group(&lap);
    assert_ne!(old.secret, new.secret);
    assert_ne!(old.group, new.group);
    let client = node(97).await;
    refused(
        send(&client, &old, &request(&client, &old, "Bob", "bob-laptop")).await,
        "invite-unknown",
    );
    assert!(welcomed(
        &send(&client, &new, &request(&client, &new, "Bob", "bob-laptop")).await
    ));
    client.close().await;
}
