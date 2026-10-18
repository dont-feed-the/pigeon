//! A join exchange has one deadline across dial, stream and reply (09 §9.6).
//! A silent peer must not retain the invitation dial indefinitely.

use super::{NetError, Node, parse_device_id, read_line};
use iroh::EndpointAddr;
use pigeon_proto::join::{Join, JoinAnswer};
use pigeon_proto::stream::ALPN_JOIN;
use std::net::SocketAddr;
use std::time::Duration;

const EXCHANGE_FOR: Duration = Duration::from_secs(30);

impl Node {
    /// Send one bounded invitation exchange to an authenticated admitter.
    pub async fn join(
        &self,
        device: &str,
        addresses: Vec<SocketAddr>,
        join: &Join,
    ) -> Result<JoinAnswer, NetError> {
        self.join_for(device, addresses, join, EXCHANGE_FOR).await
    }

    async fn join_for(
        &self,
        device: &str,
        addresses: Vec<SocketAddr>,
        join: &Join,
        budget: Duration,
    ) -> Result<JoinAnswer, NetError> {
        tokio::time::timeout(budget, self.join_exchange(device, addresses, join))
            .await
            .map_err(|_| NetError::Connect("join exchange timed out".into()))?
    }
    /// Sends `join` to the inviter `device` on `pigeon/join/1` and reads its
    /// answer, on a connection of its own.
    async fn join_exchange(
        &self,
        device: &str,
        addresses: Vec<SocketAddr>,
        join: &Join,
    ) -> Result<JoinAnswer, NetError> {
        let key = parse_device_id(device).ok_or_else(|| NetError::Connect("device id".into()))?;
        let mut addr = EndpointAddr::new(key);
        for a in addresses {
            addr = addr.with_ip_addr(a);
        }
        let connection = self
            .endpoint
            .connect(addr, ALPN_JOIN)
            .await
            .map_err(|e| NetError::Connect(e.to_string()))?;
        struct Closing<'a>(&'a iroh::endpoint::Connection);
        impl Drop for Closing<'_> {
            fn drop(&mut self) {
                self.0.close(0u32.into(), b"join exchange ended");
            }
        }
        // Timeout or caller cancellation also closes this connection.
        let _closing = Closing(&connection);
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|e| NetError::Connect(e.to_string()))?;
        let mut join = join.clone();
        join.traceparent = pigeon_telemetry::traceparent();
        let mut line = serde_json::to_vec(&join).map_err(|e| NetError::Connect(e.to_string()))?;
        line.push(b'\n');
        send.write_all(&line)
            .await
            .map_err(|e| NetError::Connect(e.to_string()))?;
        let answer = read_line(&mut recv).await?;
        let _ = send.finish();
        connection.close(0u32.into(), b"joined");
        serde_json::from_slice(&answer).map_err(|e| NetError::Connect(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "real silent-admitter QUIC fixture")]
    use super::*;
    use crate::net::{NetConfig, Reach};
    use std::sync::Arc;

    async fn node(seed: u8) -> Arc<Node> {
        Arc::new(
            Node::bind(&NetConfig {
                secret_key: [seed; 32],
                bind: "127.0.0.1:0".parse().unwrap(),
                mdns: false,
                reach: Reach::Local,
            })
            .await
            .unwrap(),
        )
    }

    fn request() -> Join {
        serde_json::from_value(serde_json::json!({
            "v": pigeon_proto::join::JOIN_VERSION,
            "invitation_hash": "synthetic hash", "proof": "synthetic proof",
            "person": "fixture", "device_name": "fixture laptop", "role": "laptop",
            "annex_uuid": "fixture-uuid"
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn a_silent_admitter_is_bounded_and_its_connection_closes() {
        let a = node(112).await;
        let b = node(113).await;
        let client = a.clone();
        let device = b.device_id();
        let addresses = b.bound_addresses();
        let joining = tokio::spawn(async move {
            client
                .join_for(&device, addresses, &request(), Duration::from_secs(2))
                .await
        });
        let connection = tokio::time::timeout(Duration::from_secs(10), async {
            let c = b.endpoint().accept().await.unwrap().await.unwrap();
            let (_send, mut recv) = c.accept_bi().await.unwrap();
            let line = read_line(&mut recv).await.unwrap();
            assert!(serde_json::from_slice::<Join>(&line).is_ok());
            // Keep the sending half alive while deliberately never replying.
            (c, _send)
        })
        .await
        .unwrap();
        let answer = tokio::time::timeout(Duration::from_secs(4), joining)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(answer, Err(NetError::Connect(s)) if s == "join exchange timed out"));
        tokio::time::timeout(Duration::from_secs(5), connection.0.closed())
            .await
            .unwrap();
        a.close().await;
        b.close().await;
    }

    #[tokio::test]
    async fn canceling_a_join_closes_its_authenticated_connection() {
        let a = node(114).await;
        let b = node(115).await;
        let client = a.clone();
        let device = b.device_id();
        let addresses = b.bound_addresses();
        let joining = tokio::spawn(async move {
            client
                .join_for(&device, addresses, &request(), Duration::from_secs(60))
                .await
        });
        let (connection, _send) = tokio::time::timeout(Duration::from_secs(10), async {
            let c = b.endpoint().accept().await.unwrap().await.unwrap();
            let (send, mut recv) = c.accept_bi().await.unwrap();
            read_line(&mut recv).await.unwrap();
            (c, send)
        })
        .await
        .unwrap();
        joining.abort();
        assert!(joining.await.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_secs(5), connection.closed())
            .await
            .unwrap();
        a.close().await;
        b.close().await;
    }

    #[tokio::test]
    async fn an_idle_join_connection_cannot_wait_forever_to_open_a_stream() {
        let a = node(116).await;
        let b = node(117).await;
        let server = Arc::new(super::super::socket::Net {
            node: b.as_ref().clone(),
            directory: Arc::new(super::super::StaticDirectory::default()),
            listeners: Default::default(),
            syncer: Arc::new(super::super::sync::AnnexSyncer::new(
                b.resources.clone(),
                pigeon_git::GitEnv::default(),
            )),
            socket_path: "/unused-join-fixture".into(),
            loops: None,
        });
        let serving = tokio::spawn(super::super::serve::accept_loop(server));
        let mut address = EndpointAddr::new(b.endpoint().id());
        for ip in b.bound_addresses() {
            address = address.with_ip_addr(ip);
        }
        let connection = a.endpoint().connect(address, ALPN_JOIN).await.unwrap();
        tokio::time::timeout(Duration::from_secs(15), connection.closed())
            .await
            .unwrap();
        a.close().await;
        b.close().await;
        serving.await.unwrap();
    }
}
