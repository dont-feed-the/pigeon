//! Bound peer application tasks before spawning or waiting (M7 join review).
//! Slots cover handshakes and connection handlers; annex stream slots are
//! shared across connections. Excess work is refused, never queued as a task.
//! An idle annex connection closes only after every active stream has ended.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

use iroh::endpoint::Connection;
use pigeon_proto::stream::{ALPN_DEBUG, ALPN_JOIN, ALPN_RECOVERY, ALPN_TELEMETRY};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;

use super::super::socket::Net;
use super::HEADER_TIMEOUT;

const CONNECTIONS: usize = 64;
const STREAMS: usize = 128;
const PENDING_CONNECTIONS_PER_PEER: usize = 8;
const PENDING_HEADERS_PER_PEER: usize = 16;

struct Peer {
    connections: Arc<Semaphore>,
    headers: Arc<Semaphore>,
}

impl Default for Peer {
    fn default() -> Self {
        Self {
            connections: Arc::new(Semaphore::new(PENDING_CONNECTIONS_PER_PEER)),
            headers: Arc::new(Semaphore::new(PENDING_HEADERS_PER_PEER)),
        }
    }
}

#[derive(Clone)]
struct PendingConnection(Arc<Mutex<Option<OwnedSemaphorePermit>>>);

/// Capacity for a header which has not established roster membership yet.
/// Every exit drops the header permit. Admission releases the connection's
/// initial quota too; active work still holds its original global permits.
pub(super) struct Header {
    _peer: Arc<Peer>,
    _slot: OwnedSemaphorePermit,
    connection: PendingConnection,
}

impl Header {
    pub(super) fn admitted(self) {
        self.connection
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
    }
}

#[derive(Clone)]
struct Limits {
    connections: Arc<Semaphore>,
    streams: Arc<Semaphore>,
    peers: Arc<Mutex<BTreeMap<String, Weak<Peer>>>>,
    idle_timeout: std::time::Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connections: Arc::new(Semaphore::new(CONNECTIONS)),
            streams: Arc::new(Semaphore::new(STREAMS)),
            peers: Arc::default(),
            idle_timeout: HEADER_TIMEOUT,
        }
    }
}

impl Limits {
    fn peer(&self, device: &str) -> Arc<Peer> {
        let mut peers = self.peers.lock().unwrap_or_else(|p| p.into_inner());
        // Only live handlers own peer state: identity churn cannot grow a
        // permanent map. The global connection budget bounds active entries.
        peers.retain(|_, peer| peer.strong_count() > 0);
        if let Some(peer) = peers.get(device).and_then(Weak::upgrade) {
            return peer;
        }
        let peer = Arc::new(Peer::default());
        peers.insert(device.to_owned(), Arc::downgrade(&peer));
        peer
    }
}

pub(super) async fn accept_loop(net: Arc<Net>) {
    run(net, Limits::default()).await;
}

async fn run(net: Arc<Net>, limits: Limits) {
    while let Some(incoming) = net.node.endpoint().accept().await {
        let Ok(slot) = limits.connections.clone().try_acquire_owned() else {
            incoming.refuse();
            continue;
        };
        let net = net.clone();
        let limits = limits.clone();
        tokio::spawn(async move {
            let _slot = slot;
            let connection = match tokio::time::timeout(HEADER_TIMEOUT, incoming).await {
                Ok(Ok(connection)) => connection,
                _ => return,
            };
            handle(net, connection, limits).await;
        });
    }
}

async fn handle(net: Arc<Net>, connection: Connection, limits: Limits) {
    let device = super::super::device_id(&connection.remote_id());
    let peer = limits.peer(&device);
    let Ok(slot) = peer.connections.clone().try_acquire_owned() else {
        connection.close(1u32.into(), b"peer initial connection capacity");
        return;
    };
    let pending = PendingConnection(Arc::new(Mutex::new(Some(slot))));
    if connection.alpn() == ALPN_DEBUG {
        super::super::debug::serve(&net, &device, &connection).await;
        return;
    }
    if connection.alpn() == ALPN_RECOVERY {
        super::super::recovery::serve(&net, &device, &connection).await;
        return;
    }
    if connection.alpn() == ALPN_TELEMETRY {
        if let Ok(Ok((send, recv))) =
            tokio::time::timeout(HEADER_TIMEOUT, connection.accept_bi()).await
        {
            super::super::telemetry::serve(&net, &device, send, recv).await;
        }
        return;
    }
    if connection.alpn() == ALPN_JOIN {
        if let Ok(Ok((send, recv))) =
            tokio::time::timeout(HEADER_TIMEOUT, connection.accept_bi()).await
        {
            super::join(&net, device, send, recv).await;
        } else {
            connection.close(0u32.into(), b"join stream timeout");
        }
        return;
    }
    annex(net, device, connection, limits, peer, pending).await;
}

async fn annex(
    net: Arc<Net>,
    device: String,
    connection: Connection,
    limits: Limits,
    peer: Arc<Peer>,
    pending: PendingConnection,
) {
    let mut streams = JoinSet::new();
    loop {
        // Completed task records cannot accumulate during a fast stream flood.
        while streams.try_join_next().is_some() {}
        tokio::select! {
            Some(_) = streams.join_next(), if !streams.is_empty() => {}
            incoming = tokio::time::timeout(limits.idle_timeout, connection.accept_bi()) => match incoming {
                Ok(Ok((mut send, mut recv))) => {
                    let Ok(slot) = limits.streams.clone().try_acquire_owned() else {
                        let _ = send.reset(1u32.into());
                        let _ = recv.stop(1u32.into());
                        continue;
                    };
                    let Ok(header_slot) = peer.headers.clone().try_acquire_owned() else {
                        let _ = send.reset(1u32.into());
                        let _ = recv.stop(1u32.into());
                        continue;
                    };
                    let header = Header {
                        _peer: peer.clone(), _slot: header_slot, connection: pending.clone(),
                    };
                    let net = net.clone();
                    let device = device.clone();
                    let connection = connection.clone();
                    streams.spawn(async move {
                        let _slot = slot;
                        super::stream(net, device, connection, send, recv, header).await;
                    });
                }
                Ok(Err(_)) => break,
                Err(_) if streams.is_empty() => {
                    connection.close(0u32.into(), b"idle annex connection");
                    break;
                }
                // A live transfer is never cut off by connection idleness.
                Err(_) => {}
            }
        }
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
