//! Read-only recovery metadata (09 §9.9, ADR 0047). A sheet fetches the
//! signed history, then serves its own signed addition until a custodian
//! has pulled and checked it. No annex content or remote ref write is
//! exposed. Transfers use private temporary files, a size limit and a deadline.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use iroh::EndpointAddr;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use pigeon_proto::stream::{ALPN_RECOVERY, HEADER_VERSION, StreamHeader};
use tempfile::TempPath;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{Node, device_id, parse_device_id, read_line, socket::Net};

/// Maximum metadata bundle; content objects never travel on this protocol.
const MAX_BUNDLE: u64 = 64 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(120);

/// A private bundle, removed after its last sender finishes.
#[derive(Clone, Debug)]
pub struct Bundle(Arc<TempPath>);

impl Bundle {
    /// Allocates a private file in a repository's own directory.
    pub fn new(dir: &Path) -> std::io::Result<Self> {
        Ok(Self(Arc::new(
            tempfile::NamedTempFile::new_in(dir)?.into_temp_path(),
        )))
    }

    /// Its path, valid while this value is held.
    pub fn path(&self) -> &Path {
        self.0.as_ref().as_ref()
    }
}

async fn header(connection: &Connection) -> Option<(SendStream, StreamHeader)> {
    let (send, mut recv) = connection.accept_bi().await.ok()?;
    let line = read_line(&mut recv).await.ok()?;
    let header: StreamHeader = serde_json::from_slice(&line).ok()?;
    (header.v == HEADER_VERSION).then_some((send, header))
}

async fn send_bundle(mut send: SendStream, bundle: Option<Bundle>) -> std::io::Result<()> {
    let Some(bundle) = bundle else {
        send.write_all(b"refused\n").await?;
        let _ = send.finish();
        let _ = send.stopped().await;
        return Ok(());
    };
    let mut file = tokio::fs::File::open(bundle.path()).await?;
    if file.metadata().await?.len() > MAX_BUNDLE {
        send.write_all(b"too-large\n").await?;
    } else {
        send.write_all(b"ok\n").await?;
        tokio::io::copy(&mut file, &mut send).await?;
    }
    let _ = send.finish();
    let _ = send.stopped().await;
    Ok(())
}

/// Serves one authenticated sheet's metadata request through the loops.
pub(super) async fn serve(net: &Net, device: &str, connection: &Connection) {
    let exchange = async {
        let (send, header) = header(connection).await?;
        let bundle = match &net.loops {
            Some(loops) => loops.recovery_bundle(header.group, device.to_owned()).await,
            None => None,
        };
        if let Err(error) = send_bundle(send, bundle).await {
            tracing::warn!(%error, "recovery-metadata-send-failed");
        }
        Some(())
    };
    let _ = tokio::time::timeout(DEADLINE, exchange).await;
}

/// Serves a new computer's sheet-signed commit only to the chosen custodian.
/// The task is aborted and its endpoint closed after the custodian answers.
pub async fn offer(node: Node, peer: String, group: String, bundle: Bundle) {
    while let Some(incoming) = node.endpoint().accept().await {
        let exchange = async {
            let connection = incoming.await.ok()?;
            if connection.alpn() != ALPN_RECOVERY || device_id(&connection.remote_id()) != peer {
                return None;
            }
            let (send, header) = header(&connection).await?;
            let offered = (header.group == group).then(|| bundle.clone());
            send_bundle(send, offered).await.ok()
        };
        let _ = tokio::time::timeout(DEADLINE, exchange).await;
    }
}

async fn receive(mut recv: RecvStream, dir: &Path) -> Result<Bundle, String> {
    let answer = read_line(&mut recv).await.map_err(|e| e.to_string())?;
    if answer != b"ok" {
        return Err("invite-not-allowed".into());
    }
    let bundle = Bundle::new(dir).map_err(|e| e.to_string())?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(bundle.path())
        .await
        .map_err(|e| e.to_string())?;
    let n = tokio::io::copy(&mut recv.take(MAX_BUNDLE + 1), &mut file)
        .await
        .map_err(|e| e.to_string())?;
    if n > MAX_BUNDLE {
        return Err("recovery metadata exceeds the size limit".into());
    }
    file.flush().await.map_err(|e| e.to_string())?;
    Ok(bundle)
}

/// Pulls metadata from a sheet endpoint or a custodian; QUIC authenticates
/// the requested device ID before any bundle is read.
pub async fn fetch(
    node: &Node,
    peer: &str,
    addresses: Vec<std::net::SocketAddr>,
    group: &str,
    dir: &Path,
) -> Result<Bundle, String> {
    let exchange = async {
        let key = parse_device_id(peer).ok_or("invite-unknown")?;
        let mut addr = EndpointAddr::new(key);
        for address in addresses {
            addr = addr.with_ip_addr(address);
        }
        let connection = node
            .endpoint()
            .connect(addr, ALPN_RECOVERY)
            .await
            .map_err(|e| e.to_string())?;
        let (mut send, recv) = connection.open_bi().await.map_err(|e| e.to_string())?;
        send.write_all(
            &StreamHeader {
                v: HEADER_VERSION,
                group: group.to_owned(),
                traceparent: pigeon_telemetry::traceparent(),
                addresses: Vec::new(),
            }
            .to_line(),
        )
        .await
        .map_err(|e| e.to_string())?;
        let _ = send.finish();
        let result = receive(recv, dir).await;
        connection.close(0u32.into(), b"metadata fetched");
        result
    };
    tokio::time::timeout(DEADLINE, exchange)
        .await
        .map_err(|_| "inviter-offline".to_owned())?
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;

    async fn node(key: u8) -> Node {
        Node::bind(&super::super::NetConfig {
            secret_key: [key; 32],
            bind: "127.0.0.1:0".parse().unwrap(),
            mdns: false,
            reach: super::super::Reach::Local,
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn an_offer_serves_only_the_named_custodian_and_group() {
        let dir = tempfile::tempdir().unwrap();
        let (server, custodian, outsider) = (node(11).await, node(12).await, node(13).await);
        let bundle = Bundle::new(dir.path()).unwrap();
        let bytes = vec![42u8; 128 * 1024];
        std::fs::write(bundle.path(), &bytes).unwrap();
        let task = tokio::spawn(offer(
            server.clone(),
            custodian.device_id(),
            "group".into(),
            bundle,
        ));
        let fetch_from = |client: Node, group: &'static str| {
            let server = server.clone();
            let path = dir.path().to_owned();
            async move {
                fetch(
                    &client,
                    &server.device_id(),
                    server.advertised_addresses(),
                    group,
                    &path,
                )
                .await
            }
        };
        assert!(fetch_from(outsider.clone(), "group").await.is_err());
        assert!(fetch_from(custodian.clone(), "other-group").await.is_err());
        let received = fetch_from(custodian.clone(), "group").await.unwrap();
        assert_eq!(std::fs::read(received.path()).unwrap(), bytes);
        task.abort();
        let _ = task.await;
        server.close().await;
        custodian.close().await;
        outsider.close().await;
    }

    #[tokio::test]
    async fn an_oversized_offer_is_rejected_before_its_body() {
        let dir = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let (server, custodian) = (node(21).await, node(22).await);
        let bundle = Bundle::new(dir.path()).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(bundle.path())
            .unwrap()
            .set_len(MAX_BUNDLE + 1)
            .unwrap();
        let task = tokio::spawn(offer(
            server.clone(),
            custodian.device_id(),
            "group".into(),
            bundle,
        ));
        assert!(
            fetch(
                &custodian,
                &server.device_id(),
                server.advertised_addresses(),
                "group",
                output.path()
            )
            .await
            .is_err()
        );
        assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 0);
        task.abort();
        let _ = task.await;
        server.close().await;
        custodian.close().await;
    }
}
