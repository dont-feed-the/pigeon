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
    Arc::new(Net {
        directory: Arc::new(StaticDirectory::default()),
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
