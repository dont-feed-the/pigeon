//! Consent-gated diagnostic snapshots over `pigeon/debug/1` (12 §12.2).
//! Only authenticated roster members may request a part. Neither this
//! ALPN nor a signed roster can grant consent on another computer.

use std::time::Duration;

use iroh::EndpointAddr;
use iroh::endpoint::Connection;
use pigeon_proto::stream::{ALPN_DEBUG, HEADER_VERSION, StreamHeader};
use serde_json::Value;

use super::{Node, parse_device_id, read_line, socket::Net};

const LIMIT: usize = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(45);

pub(super) async fn serve(net: &Net, peer: &str, connection: &Connection) {
    let exchange = async {
        let (mut send, mut recv) = connection.accept_bi().await.ok()?;
        let line = read_line(&mut recv).await.ok()?;
        let header: StreamHeader = serde_json::from_slice(&line).ok()?;
        let admitted =
            header.v == HEADER_VERSION && net.directory.admit(&header.group, peer).is_some();
        let value = if admitted {
            match &net.loops {
                Some(loops) => {
                    let mut span = pigeon_telemetry::remote_span(
                        "debug.receive",
                        header.traceparent.as_deref(),
                    );
                    span.set("group", header.group.clone());
                    span.in_scope(loops.diagnostics(header.group.clone(), peer.to_owned()))
                        .await
                }
                None => None,
            }
        } else {
            None
        };
        let bytes = value
            .and_then(|v| serde_json::to_vec(&v).ok())
            .filter(|b| b.len() <= LIMIT);
        if let Some(bytes) = bytes.filter(|_| net.directory.admit(&header.group, peer).is_some()) {
            send.write_all(b"ok\n").await.ok()?;
            send.write_all(&bytes).await.ok()?;
        } else {
            send.write_all(b"refused\n").await.ok()?;
        }
        send.finish().ok()?;
        let _ = send.stopped().await;
        Some(())
    };
    let _ = tokio::time::timeout(TIMEOUT, exchange).await;
}

impl Node {
    /// Requests one part. A refusal means the peer's person has not granted
    /// a live one-use permission; retries cannot bypass that choice.
    pub async fn diagnostics(
        &self,
        peer: &str,
        addresses: Vec<std::net::SocketAddr>,
        group: &str,
    ) -> Result<Value, String> {
        let exchange = async {
            let key = parse_device_id(peer).ok_or("peer-unreachable")?;
            let mut addr = EndpointAddr::new(key);
            for address in addresses {
                addr = addr.with_ip_addr(address)
            }
            let connection = self
                .endpoint()
                .connect(addr, ALPN_DEBUG)
                .await
                .map_err(|_| "peer-unreachable")?;
            let (mut send, mut recv) =
                connection.open_bi().await.map_err(|_| "peer-unreachable")?;
            send.write_all(
                &StreamHeader {
                    v: HEADER_VERSION,
                    group: group.into(),
                    traceparent: pigeon_telemetry::traceparent(),
                    addresses: Vec::new(),
                }
                .to_line(),
            )
            .await
            .map_err(|_| "peer-unreachable")?;
            let _ = send.finish();
            let answer = read_line(&mut recv).await.map_err(|_| "peer-unreachable")?;
            if answer != b"ok" {
                return Err("debug-not-allowed");
            }
            let bytes = recv
                .read_to_end(LIMIT)
                .await
                .map_err(|_| "peer-unreachable")?;
            connection.close(0u32.into(), b"diagnostics received");
            let value: Value = serde_json::from_slice(&bytes).map_err(|_| "peer-unreachable")?;
            if value["format"] != 1 || value["group"] != group || value["device"] != peer {
                return Err("debug-not-allowed");
            }
            Ok(value)
        };
        tokio::time::timeout(TIMEOUT, exchange)
            .await
            .map_err(|_| "peer-unreachable".to_owned())?
            .map_err(str::to_owned)
    }
}
