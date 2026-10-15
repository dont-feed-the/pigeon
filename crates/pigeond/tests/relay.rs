//! Reach (09 §9.5, ADR 0016): two endpoints with n0's relays and address
//! lookup find each other by device ID alone, neither telling the other an
//! address, and a stream crosses. Needs the internet: skipped unless
//! `PIGEON_NET_TESTS` is set, as the reach job sets it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
#![allow(clippy::print_stderr, reason = "tests")]

use std::time::Duration;

use iroh::EndpointAddr;
use pigeon_proto::stream::ALPN_JOIN;
use pigeond::net::{NetConfig, Node, Reach};

async fn node(i: u8) -> Node {
    let config = NetConfig {
        secret_key: [i; 32],
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: Reach::N0,
    };
    Node::bind(&config).await.unwrap()
}

#[tokio::test]
async fn two_devices_meet_by_id_over_n0() {
    if std::env::var_os("PIGEON_NET_TESTS").is_none() {
        eprintln!("skipped: PIGEON_NET_TESTS is not set");
        return;
    }
    let (a, b) = (node(1).await, node(2).await);
    let online = tokio::time::timeout(Duration::from_secs(60), b.endpoint().online());
    online.await.expect("b reaches a relay");
    let id = b.endpoint().id();
    let echo = tokio::spawn(async move {
        let incoming = b.endpoint().accept().await.unwrap();
        let connection = incoming.await.unwrap();
        let (mut send, mut recv) = connection.accept_bi().await.unwrap();
        let mut buf = [0u8; 5];
        recv.read_exact(&mut buf).await.unwrap();
        send.write_all(&buf).await.unwrap();
        send.finish().unwrap();
        connection.closed().await;
        b
    });
    let dial = a.endpoint().connect(EndpointAddr::new(id), ALPN_JOIN);
    let connection = tokio::time::timeout(Duration::from_secs(60), dial)
        .await
        .expect("a finds b in time")
        .unwrap();
    let (mut send, mut recv) = connection.open_bi().await.unwrap();
    send.write_all(b"hello").await.unwrap();
    send.finish().unwrap();
    let mut buf = [0u8; 5];
    recv.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"hello");
    connection.close(0u32.into(), b"done");
    let b = echo.await.unwrap();
    a.close().await;
    b.close().await;
}
