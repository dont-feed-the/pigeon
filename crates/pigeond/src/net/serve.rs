//! Serving peers (09 §9.3, §9.4): admission, then the filtered relay.
//!
//! Each incoming `pigeon/annex/1` stream starts with a header naming a
//! group. It is served only if this device has a repository of that group,
//! the peer's authenticated device ID is in that group's roster, and the
//! repository's remotedaemon is listening; anything else is refused with a
//! reason and never reaches git-annex (ADR 0006). An admitted stream is
//! relayed to remotedaemon through `pigeon_core::filter`, whose actions are
//! carried out in order; a sync it asks for runs beside the relay.

use std::sync::Arc;
use std::time::Duration;

use iroh::endpoint::{RecvStream, SendStream};
use pigeon_core::filter::{Action, Config, Filter};
use pigeon_proto::join::{Join, JoinAnswer};
use pigeon_proto::stream::{
    ALPN_DEBUG, ALPN_JOIN, ALPN_RECOVERY, ALPN_TELEMETRY, HEADER_VERSION, Refusal, StreamAnswer,
    StreamHeader,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::task::JoinSet;

use super::socket::Net;
use super::{Admitted, Repository, device_id, read_line};

/// How long a peer has to send its header.
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);

/// Accepts peers until the endpoint closes.
pub async fn accept_loop(net: Arc<Net>) {
    while let Some(incoming) = net.node.endpoint().accept().await {
        let net = net.clone();
        tokio::spawn(async move {
            let connection = match incoming.await {
                Ok(c) => c,
                Err(e) => {
                    tracing::debug!(error = %e, "incoming-failed");
                    return;
                }
            };
            let device = device_id(&connection.remote_id());
            if connection.alpn() == ALPN_DEBUG {
                super::debug::serve(&net, &device, &connection).await;
                return;
            }
            if connection.alpn() == ALPN_RECOVERY {
                super::recovery::serve(&net, &device, &connection).await;
                return;
            }
            if connection.alpn() == ALPN_TELEMETRY {
                if let Ok(Ok((send, recv))) =
                    tokio::time::timeout(HEADER_TIMEOUT, connection.accept_bi()).await
                {
                    super::telemetry::serve(&net, &device, send, recv).await;
                }
                return;
            }
            if connection.alpn() == ALPN_JOIN {
                if let Ok((send, recv)) = connection.accept_bi().await {
                    join(&net, device, send, recv).await;
                }
                return;
            }
            while let Ok((send, recv)) = connection.accept_bi().await {
                let net = net.clone();
                let device = device.clone();
                let connection = connection.clone();
                tokio::spawn(async move { stream(net, device, connection, send, recv).await });
            }
        });
    }
}

/// One join exchange: a [`Join`] line, answered by the loops.
async fn join(net: &Net, device: String, mut send: SendStream, mut recv: RecvStream) {
    let line = match tokio::time::timeout(HEADER_TIMEOUT, read_line(&mut recv)).await {
        Ok(Ok(line)) => line,
        _ => return,
    };
    let refused = |code: &str| JoinAnswer::Refused {
        code: code.to_owned(),
    };
    let answer = match (serde_json::from_slice::<Join>(&line), &net.loops) {
        (Ok(j), Some(loops)) => {
            let span = pigeon_telemetry::remote_span("join.receive", j.traceparent.as_deref());
            span.in_scope(loops.join(device.clone(), j)).await
        }
        (Ok(_), None) => refused("not-in-this-build"),
        (Err(_), _) => refused("invite-unknown"),
    };
    tracing::info!(peer = %device, answer = ?answer, "join-answered");
    let mut out = serde_json::to_vec(&answer).unwrap_or_default();
    out.push(b'\n');
    let _ = send.write_all(&out).await;
    let _ = send.finish();
    let _ = tokio::time::timeout(Duration::from_secs(5), send.stopped()).await;
}

async fn refuse(mut send: SendStream, device: &str, refusal: Refusal) {
    tracing::warn!(peer = %device, refusal = ?refusal, "stream-refused");
    let _ = send
        .write_all(&StreamAnswer::refuse(refusal).to_line())
        .await;
    let _ = send.finish();
    let _ = tokio::time::timeout(Duration::from_secs(5), send.stopped()).await;
}

async fn stream(
    net: Arc<Net>,
    device: String,
    connection: iroh::endpoint::Connection,
    send: SendStream,
    mut recv: RecvStream,
) {
    let header = match tokio::time::timeout(HEADER_TIMEOUT, read_line(&mut recv)).await {
        Ok(Ok(line)) => serde_json::from_slice::<StreamHeader>(&line).ok(),
        _ => None,
    };
    let Some(header) = header else {
        return refuse(send, &device, Refusal::BadHeader).await;
    };
    if header.v > HEADER_VERSION {
        return refuse(send, &device, Refusal::PeerTooOld).await;
    }
    let Some(repository) = net.directory.group(&header.group) else {
        return refuse(send, &device, Refusal::UnknownGroup).await;
    };
    let Some(admitted) = net.directory.admit(&header.group, &device) else {
        return refuse(send, &device, Refusal::NotInRoster).await;
    };
    let Some(socket) = net.listeners.get(&header.group) else {
        return refuse(send, &device, Refusal::NotListening).await;
    };
    let Ok(server) = UnixStream::connect(&socket).await else {
        return refuse(send, &device, Refusal::NotListening).await;
    };
    let mut send = send;
    if send
        .write_all(&StreamAnswer::accept().to_line())
        .await
        .is_err()
    {
        return;
    }
    tracing::info!(peer = %device, group = %header.group, "stream-admitted");
    super::observe_path(&connection, &header.group, &device, "incoming");
    let addresses = header
        .addresses
        .iter()
        .filter_map(|a| a.parse().ok())
        .collect();
    net.directory.learn(&header.group, &device, addresses);
    let mut span = pigeon_telemetry::remote_span("annex.receive", header.traceparent.as_deref());
    span.set("group", header.group).set("peer", device.clone());
    span.in_scope(relay(
        &net,
        &device,
        &repository,
        &admitted,
        send,
        recv,
        server,
    ))
    .await;
}

/// Relays one session between a peer and remotedaemon through the filter.
pub async fn relay(
    net: &Net,
    device: &str,
    repository: &Repository,
    admitted: &Admitted,
    mut send: SendStream,
    mut recv: RecvStream,
    server: UnixStream,
) {
    let config = Config {
        role: repository.role,
        auth_token: repository.auth_token.clone(),
        peer_uuid: admitted.annex_uuid.clone(),
        peer_custodian: admitted.custodian,
    };
    let mut filter = Filter::new(config, repository.checking);
    let (mut from_server, mut to_server) = server.into_split();
    let mut syncs: JoinSet<(u64, std::io::Result<bool>)> = JoinSet::new();
    let mut client_buf = vec![0u8; 64 * 1024];
    let mut server_buf = vec![0u8; 64 * 1024];
    let mut client_open = true;
    let mut seen = Seen::new();
    'session: loop {
        let actions = tokio::select! {
            n = recv.read(&mut client_buf), if client_open => match n {
                Ok(Some(n)) if n > 0 => {
                    seen.up(&client_buf[..n]);
                    filter.set_content_policy(net.directory.peer_content_policy(&repository.group, device));
                    filter.from_client(&client_buf[..n])
                }
                _ => {
                    client_open = false;
                    let _ = to_server.shutdown().await;
                    Vec::new()
                }
            },
            n = from_server.read(&mut server_buf) => match n {
                Ok(n) if n > 0 => {
                    seen.down(&server_buf[..n]);
                    filter.set_content_policy(net.directory.peer_content_policy(&repository.group, device));
                    filter.from_server(&server_buf[..n])
                }
                _ => break 'session,
            },
            Some(done) = syncs.join_next(), if !syncs.is_empty() => match done {
                Ok((ticket, Ok(matched))) => {
                    filter.set_content_policy(net.directory.peer_content_policy(&repository.group, device));
                    filter.synced(ticket, matched)
                },
                Ok((_, Err(e))) => {
                    tracing::error!(peer = %device, error = %e, "sync-failed");
                    break 'session;
                }
                Err(_) => break 'session,
            },
        };
        for action in actions {
            match action {
                Action::ToServer(bytes) | Action::ContentToServer(bytes) => {
                    if to_server.write_all(&bytes).await.is_err() {
                        break 'session;
                    }
                }
                Action::ToClient(bytes) | Action::ContentToClient(bytes) => {
                    if send.write_all(&bytes).await.is_err() {
                        break 'session;
                    }
                }
                Action::Sync { ticket, key, hash } => {
                    let sync = net.syncer.sync(repository, &key, hash);
                    syncs.spawn(async move { (ticket, sync.await) });
                }
                Action::Repair { key } => {
                    tracing::error!(peer = %device, key = %key, "object-hash-mismatch");
                    tokio::spawn(net.syncer.repair(repository, &key));
                }
                Action::Refused { rule, line } => {
                    let line = String::from_utf8_lossy(&line);
                    tracing::error!(peer = %device, rule = ?rule, line = %line, "message-refused");
                }
                Action::Close(why) => {
                    tracing::warn!(peer = %device, reason = ?why, "session-closed");
                    break 'session;
                }
            }
        }
    }
    syncs.abort_all();
    seen.log(device);
    let _ = to_server.shutdown().await;
    let _ = send.finish();
    let _ = tokio::time::timeout(Duration::from_secs(5), send.stopped()).await;
}

/// What one relayed session carried, for the debug log: its length, its
/// bytes each way and the tail of each direction.
struct Seen {
    started: std::time::Instant,
    up: (usize, Vec<u8>),
    down: (usize, Vec<u8>),
}

impl Seen {
    const TAIL: usize = 96;

    fn new() -> Self {
        Seen {
            started: std::time::Instant::now(),
            up: (0, Vec::new()),
            down: (0, Vec::new()),
        }
    }

    fn keep(side: &mut (usize, Vec<u8>), bytes: &[u8]) {
        side.0 += bytes.len();
        side.1.extend_from_slice(bytes);
        let excess = side.1.len().saturating_sub(Self::TAIL);
        side.1.drain(..excess);
    }

    fn up(&mut self, bytes: &[u8]) {
        Self::keep(&mut self.up, bytes);
    }

    fn down(&mut self, bytes: &[u8]) {
        Self::keep(&mut self.down, bytes);
    }

    fn log(&self, device: &str) {
        tracing::debug!(peer = %device, ms = self.started.elapsed().as_millis(),
            up = self.up.0, down = self.down.0,
            up_tail = %String::from_utf8_lossy(&self.up.1),
            down_tail = %String::from_utf8_lossy(&self.down.1), "relay-ended");
    }
}
