//! Outgoing content gate (08 §8.6): even an older peer's serving daemon
//! cannot make this device fetch or upload a released key. The ordinary
//! laptop-role filter supplies framing only; the remote custodian still
//! performs its own durable positive-answer checks on the serving side.

use std::io;

use iroh::endpoint::{RecvStream, SendStream};
use pigeon_core::filter::{Action, Config, Filter, Role};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use super::socket::Net;

async fn forward(
    actions: Vec<Action>,
    send: &mut SendStream,
    writer: &mut OwnedWriteHalf,
    counts: &mut (u64, u64),
) -> io::Result<()> {
    for action in actions {
        match action {
            Action::ToServer(bytes) | Action::ContentToServer(bytes) => {
                send.write_all(&bytes).await.map_err(io::Error::other)?;
                counts.0 = counts.0.saturating_add(bytes.len() as u64);
            }
            Action::ToClient(bytes) | Action::ContentToClient(bytes) => {
                writer.write_all(&bytes).await?;
                counts.1 = counts.1.saturating_add(bytes.len() as u64);
            }
            Action::Close(why) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("pigeon content gate: {why:?}"),
                ));
            }
            Action::Refused { rule, .. } => tracing::warn!(?rule, "outgoing-message-refused"),
            Action::Sync { .. } | Action::Repair { .. } => {
                return Err(io::Error::other(
                    "outgoing content-only filter requested local proof work",
                ));
            }
        }
    }
    Ok(())
}

/// Relay an authenticated outgoing session, including protocol bytes that
/// shared the initial JSON read. Policies refresh before every chunk, in
/// both directions, without rereading history or cloning released sets.
pub async fn outgoing(
    net: &Net,
    target: (&str, &str),
    mut reader: OwnedReadHalf,
    mut writer: OwnedWriteHalf,
    mut send: SendStream,
    mut recv: RecvStream,
    initial: &[u8],
) -> io::Result<(u64, u64)> {
    let (group, peer) = target;
    let mut filter = Filter::new(
        Config {
            role: Role::Laptop,
            auth_token: None,
            peer_uuid: None,
            peer_custodian: false,
        },
        false,
    );
    let mut counts = (0, 0);
    filter.set_content_policy(net.directory.peer_content_policy(group, peer));
    forward(
        filter.from_client(initial),
        &mut send,
        &mut writer,
        &mut counts,
    )
    .await?;
    let mut up = vec![0; 64 * 1024];
    let mut down = vec![0; 64 * 1024];
    let mut client_open = true;
    loop {
        let actions = tokio::select! {
            read = reader.read(&mut up), if client_open => {
                let n = read?;
                if n == 0 {
                    client_open = false;
                    let _ = send.finish();
                    continue;
                }
                filter.set_content_policy(net.directory.peer_content_policy(group, peer));
                filter.from_client(&up[..n])
            }
            read = recv.read(&mut down) => {
                let Some(n) = read.map_err(io::Error::other)? else { break };
                if n == 0 { break }
                filter.set_content_policy(net.directory.peer_content_policy(group, peer));
                filter.from_server(&down[..n])
            }
        };
        forward(actions, &mut send, &mut writer, &mut counts).await?;
    }
    let _ = send.finish();
    writer.shutdown().await?;
    Ok(counts)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "real QUIC content boundary fixture"
    )]
    use super::*;
    use crate::net::{NetConfig, Node, Reach, StaticDirectory};
    use pigeon_core::filter::ContentPolicy;
    use pigeon_proto::stream::ALPN_ANNEX;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn a_local_release_stops_an_older_peers_active_content_stream() {
        tokio::time::timeout(Duration::from_secs(20), async {
            async fn node(n: u8) -> Node {
                Node::bind(&NetConfig {
                    secret_key: [n; 32],
                    bind: "127.0.0.1:0".parse().unwrap(),
                    mdns: false,
                    reach: Reach::Local,
                })
                .await
                .unwrap()
            }
            let (a, b) = (node(81).await, node(82).await);
            let directory = Arc::new(StaticDirectory::default());
            let net = Arc::new(Net {
                node: a.clone(),
                directory: directory.clone(),
                listeners: Default::default(),
                syncer: Arc::new(crate::net::sync::AnnexSyncer),
                socket_path: "/unused-content-fixture".into(),
                loops: None,
            });
            let (continued, resume) = tokio::sync::oneshot::channel();
            let peer = b.clone();
            let server = tokio::spawn(async move {
                let connection = peer.endpoint().accept().await.unwrap().await.unwrap();
                let (mut send, mut recv) = connection.accept_bi().await.unwrap();
                let mut request = vec![0; b"GET 0 file old-key\n".len()];
                recv.read_exact(&mut request).await.unwrap();
                assert_eq!(request, b"GET 0 file old-key\n");
                send.write_all(b"DATA 8\nfirst").await.unwrap();
                resume.await.unwrap();
                // This deliberately permissive peer knows nothing of the
                // local release and continues sending the old object's bytes.
                let _ = send.write_all(b"end").await;
                let _ = send.finish();
                connection.closed().await;
            });
            let mut address = iroh::EndpointAddr::new(b.endpoint().id());
            for ip in b.bound_addresses() {
                address = address.with_ip_addr(ip);
            }
            let connection = a.endpoint().connect(address, ALPN_ANNEX).await.unwrap();
            let (send, recv) = connection.open_bi().await.unwrap();
            let (mut client, bridge) = UnixStream::pair().unwrap();
            let (reader, writer) = bridge.into_split();
            let gate_net = net.clone();
            let gate = tokio::spawn(async move {
                outgoing(
                    &gate_net,
                    ("group", "peer"),
                    reader,
                    writer,
                    send,
                    recv,
                    b"GET 0 file old-key\n",
                )
                .await
            });
            let mut first = vec![0; b"DATA 8\nfirst".len()];
            client.read_exact(&mut first).await.unwrap();
            assert_eq!(first, b"DATA 8\nfirst");
            directory.set_content_policy(
                "group",
                ContentPolicy {
                    waiting: false,
                    released: ["old-key".into()].into(),
                },
            );
            continued.send(()).unwrap();
            let error = gate.await.unwrap().unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            let mut extra = Vec::new();
            let _ = client.read_to_end(&mut extra).await;
            assert!(
                extra.is_empty(),
                "released payload must not reach the local client"
            );
            connection.close(0u32.into(), b"done");
            server.await.unwrap();
            a.close().await;
            b.close().await;
        })
        .await
        .expect("the local QUIC boundary test finishes");
    }
}
