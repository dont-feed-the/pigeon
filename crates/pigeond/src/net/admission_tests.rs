//! Real QUIC admission boundaries: refuse excess connections/streams,
//! recover capacity after closure, and retain live work across idle timers.
#![allow(clippy::unwrap_used, reason = "isolated native QUIC capacity fixtures")]

use super::*;
use crate::net::{NetConfig, Node, Reach, StaticDirectory};
use iroh::EndpointAddr;
use pigeon_proto::stream::ALPN_ANNEX;
use std::time::Duration;

async fn node(seed: u8) -> Node {
    Node::bind(&NetConfig {
        secret_key: [seed; 32],
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: Reach::Local,
    })
    .await
    .unwrap()
}

fn net(node: Node) -> Arc<Net> {
    net_with_directory(node, Arc::new(StaticDirectory::default()))
}

fn net_with_directory(node: Node, directory: Arc<StaticDirectory>) -> Arc<Net> {
    Arc::new(Net {
        directory,
        listeners: Default::default(),
        syncer: Arc::new(crate::net::sync::AnnexSyncer::new(
            node.resources.clone(),
            pigeon_git::GitEnv::default(),
        )),
        socket_path: "/unused-admission-fixture".into(),
        loops: None,
        node,
    })
}

fn address(node: &Node) -> EndpointAddr {
    let mut address = EndpointAddr::new(node.endpoint().id());
    for ip in node.bound_addresses() {
        address = address.with_ip_addr(ip);
    }
    address
}

async fn permits(semaphore: &Semaphore, expected: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while semaphore.available_permits() != expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn excess_connections_are_refused_and_closed_peers_release_capacity() {
    let server = net(node(121).await);
    let a = node(122).await;
    let b = node(123).await;
    let limits = Limits {
        connections: Arc::new(Semaphore::new(1)),
        ..Limits::default()
    };
    let serving = tokio::spawn(run(server.clone(), limits.clone()));
    let first = a
        .endpoint()
        .connect(address(&server.node), ALPN_JOIN)
        .await
        .unwrap();
    permits(&limits.connections, 0).await;
    let refused = tokio::time::timeout(
        Duration::from_secs(5),
        b.endpoint().connect(address(&server.node), ALPN_JOIN),
    )
    .await
    .unwrap();
    assert!(refused.is_err(), "excess connection was admitted");
    first.close(0u32.into(), b"fixture done");
    permits(&limits.connections, 1).await;
    let retry = b
        .endpoint()
        .connect(address(&server.node), ALPN_JOIN)
        .await
        .unwrap();
    permits(&limits.connections, 0).await;
    retry.close(0u32.into(), b"fixture done");
    a.close().await;
    b.close().await;
    server.node.close().await;
    serving.await.unwrap();
}

#[tokio::test]
async fn streams_share_one_budget_across_connections_and_release_it_after_closure() {
    let server = net(node(124).await);
    let a = node(125).await;
    let b = node(126).await;
    let limits = Limits {
        streams: Arc::new(Semaphore::new(1)),
        ..Limits::default()
    };
    let serving = tokio::spawn(run(server.clone(), limits.clone()));
    let first = a
        .endpoint()
        .connect(address(&server.node), ALPN_ANNEX)
        .await
        .unwrap();
    let second = b
        .endpoint()
        .connect(address(&server.node), ALPN_ANNEX)
        .await
        .unwrap();
    let (mut held_send, _held_recv) = first.open_bi().await.unwrap();
    held_send.write_all(b"{").await.unwrap();
    permits(&limits.streams, 0).await;
    let (mut send, mut recv) = second.open_bi().await.unwrap();
    send.write_all(b"{").await.unwrap();
    let answer = tokio::time::timeout(Duration::from_secs(5), recv.read(&mut [0; 1]))
        .await
        .unwrap();
    assert!(
        answer.is_err() || matches!(answer, Ok(None)),
        "excess stream entered a header wait"
    );
    assert_eq!(limits.streams.available_permits(), 0);
    first.close(0u32.into(), b"fixture done");
    permits(&limits.streams, 1).await;
    let (mut send, mut recv) = second.open_bi().await.unwrap();
    send.write_all(b"{}\n").await.unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(5), crate::net::read_line(&mut recv))
        .await
        .unwrap()
        .unwrap();
    assert!(serde_json::from_slice::<pigeon_proto::stream::StreamAnswer>(&reply).is_ok());
    a.close().await;
    b.close().await;
    server.node.close().await;
    serving.await.unwrap();
}

#[tokio::test]
async fn an_idle_annex_connection_releases_its_application_slot() {
    let server = net(node(127).await);
    let client = node(128).await;
    let limits = Limits::default();
    let serving = tokio::spawn(run(server.clone(), limits.clone()));
    let connection = client
        .endpoint()
        .connect(address(&server.node), ALPN_ANNEX)
        .await
        .unwrap();
    permits(&limits.connections, CONNECTIONS - 1).await;
    tokio::time::timeout(Duration::from_secs(15), connection.closed())
        .await
        .unwrap();
    permits(&limits.connections, CONNECTIONS).await;
    client.close().await;
    server.node.close().await;
    serving.await.unwrap();
}

#[tokio::test]
async fn idle_accept_deadlines_never_cancel_a_live_stream() {
    let server = net(node(129).await);
    let client = node(130).await;
    let limits = Limits {
        idle_timeout: Duration::from_millis(100),
        ..Limits::default()
    };
    let serving = tokio::spawn(run(server.clone(), limits.clone()));
    let connection = client
        .endpoint()
        .connect(address(&server.node), ALPN_ANNEX)
        .await
        .unwrap();
    let (mut send, _recv) = connection.open_bi().await.unwrap();
    send.write_all(b"{").await.unwrap();
    permits(&limits.streams, STREAMS - 1).await;
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert!(
        connection.close_reason().is_none(),
        "idle accept canceled a live header stream"
    );
    connection.close(0u32.into(), b"fixture done");
    client.close().await;
    server.node.close().await;
    serving.await.unwrap();
}

#[tokio::test]
async fn one_peer_with_incomplete_headers_cannot_block_another_peer() {
    let server = net(node(210).await);
    let a = node(211).await;
    let b = node(212).await;
    let limits = Limits {
        streams: Arc::new(Semaphore::new(17)),
        ..Limits::default()
    };
    let serving = tokio::spawn(run(server.clone(), limits.clone()));
    let first = a
        .endpoint()
        .connect(address(&server.node), ALPN_ANNEX)
        .await
        .unwrap();
    let second = a
        .endpoint()
        .connect(address(&server.node), ALPN_ANNEX)
        .await
        .unwrap();
    let other = b
        .endpoint()
        .connect(address(&server.node), ALPN_ANNEX)
        .await
        .unwrap();
    let mut held = Vec::new();
    for n in 0..16 {
        let connection = if n % 2 == 0 { &first } else { &second };
        let (mut send, recv) = connection.open_bi().await.unwrap();
        send.write_all(b"{").await.unwrap();
        held.push((send, recv));
        permits(&limits.streams, 16 - n).await;
    }
    let (mut excess_send, mut excess_recv) = first.open_bi().await.unwrap();
    excess_send.write_all(b"{").await.unwrap();
    let excess = tokio::time::timeout(Duration::from_secs(5), excess_recv.read(&mut [0; 1])).await;
    assert!(
        matches!(excess, Ok(Err(_)) | Ok(Ok(None))),
        "one peer consumed the slot reserved for another peer"
    );
    permits(&limits.streams, 1).await;
    let (mut send, mut recv) = other.open_bi().await.unwrap();
    send.write_all(b"{}\n").await.unwrap();
    let response =
        tokio::time::timeout(Duration::from_secs(5), crate::net::read_line(&mut recv)).await;
    assert!(
        matches!(response, Ok(Ok(_))),
        "another peer could not reach header validation"
    );
    first.close(0u32.into(), b"fixture done");
    second.close(0u32.into(), b"fixture done");
    other.close(0u32.into(), b"fixture done");
    drop(held);
    permits(&limits.streams, 17).await;
    a.close().await;
    b.close().await;
    server.node.close().await;
    serving.await.unwrap();
}

#[tokio::test]
async fn initial_connection_capacity_is_per_peer_and_recovers_after_closure() {
    let server = net(node(213).await);
    let a = node(214).await;
    let b = node(215).await;
    let limits = Limits {
        connections: Arc::new(Semaphore::new(9)),
        ..Limits::default()
    };
    let serving = tokio::spawn(run(server.clone(), limits.clone()));
    let mut held = Vec::new();
    for n in 0..8 {
        held.push(
            a.endpoint()
                .connect(address(&server.node), ALPN_JOIN)
                .await
                .unwrap(),
        );
        permits(&limits.connections, 8 - n).await;
    }
    let excess = tokio::time::timeout(
        Duration::from_secs(5),
        a.endpoint().connect(address(&server.node), ALPN_JOIN),
    )
    .await
    .unwrap();
    if let Ok(excess) = excess {
        tokio::time::timeout(Duration::from_secs(5), excess.closed())
            .await
            .unwrap();
    }
    permits(&limits.connections, 1).await;
    let other = b
        .endpoint()
        .connect(address(&server.node), ALPN_JOIN)
        .await
        .unwrap();
    permits(&limits.connections, 0).await;
    held[0].close(0u32.into(), b"fixture frees one initial slot");
    permits(&limits.connections, 1).await;
    let retry = a
        .endpoint()
        .connect(address(&server.node), ALPN_JOIN)
        .await
        .unwrap();
    permits(&limits.connections, 0).await;
    for connection in held {
        connection.close(0u32.into(), b"fixture done");
    }
    other.close(0u32.into(), b"fixture done");
    retry.close(0u32.into(), b"fixture done");
    permits(&limits.connections, 9).await;
    a.close().await;
    b.close().await;
    server.node.close().await;
    serving.await.unwrap();
}

#[tokio::test]
async fn roster_admission_releases_initial_quota_without_limiting_live_connections() {
    use crate::net::{Admitted, Repository};
    use pigeon_proto::stream::{HEADER_VERSION, Refusal, StreamAnswer, StreamHeader};
    let a = node(216).await;
    let directory = Arc::new(StaticDirectory::default());
    let temp = tempfile::tempdir().unwrap();
    let group = "a".repeat(40);
    directory.add_repository(Repository {
        group: group.clone(),
        git_dir: temp.path().to_owned(),
        worktree: None,
        role: pigeon_core::filter::Role::Custodian,
        auth_token: None,
        checking: false,
    });
    directory.admit_device(
        &group,
        &a.device_id(),
        Admitted {
            annex_uuid: None,
            custodian: false,
        },
    );
    let server = net_with_directory(node(217).await, directory);
    let limits = Limits {
        connections: Arc::new(Semaphore::new(10)),
        ..Limits::default()
    };
    let serving = tokio::spawn(run(server.clone(), limits.clone()));
    let mut held = Vec::new();
    for n in 0..9 {
        let connection = a
            .endpoint()
            .connect(address(&server.node), ALPN_ANNEX)
            .await
            .unwrap();
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        send.write_all(
            &StreamHeader {
                v: HEADER_VERSION,
                group: group.clone(),
                traceparent: None,
                addresses: Vec::new(),
            }
            .to_line(),
        )
        .await
        .unwrap();
        let line = tokio::time::timeout(Duration::from_secs(5), crate::net::read_line(&mut recv))
            .await
            .unwrap()
            .unwrap();
        let answer: StreamAnswer = serde_json::from_slice(&line).unwrap();
        assert_eq!(
            answer.refusal,
            Some(Refusal::NotListening),
            "roster admission was not reached"
        );
        held.push(connection);
        permits(&limits.connections, 9 - n).await;
    }
    let peer = limits.peer(&a.device_id());
    assert_eq!(peer.connections.available_permits(), 8);
    assert_eq!(peer.headers.available_permits(), 16);
    for connection in held {
        connection.close(0u32.into(), b"fixture done");
    }
    permits(&limits.connections, 10).await;
    a.close().await;
    server.node.close().await;
    serving.await.unwrap();
}

#[test]
fn identity_churn_forgets_dead_entries_and_retains_live_peer_budgets() {
    let limits = Limits::default();
    let live: Vec<_> = (0..16).map(|n| limits.peer(&format!("live-{n}"))).collect();
    for n in 0..1000 {
        let name = format!("closed-{n}");
        let peer = limits.peer(&name);
        assert!(Arc::ptr_eq(&peer, &limits.peer(&name)));
        assert!(limits.peers.lock().unwrap().len() <= 17);
    }
    drop(live);
    let _last = limits.peer("last");
    assert_eq!(limits.peers.lock().unwrap().len(), 1);
}
