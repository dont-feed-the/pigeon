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
use pigeon_proto::stream::{HEADER_VERSION, Refusal, StreamAnswer, StreamHeader};
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
            while let Ok((send, recv)) = connection.accept_bi().await {
                let net = net.clone();
                let device = device.clone();
                tokio::spawn(async move { stream(net, device, send, recv).await });
            }
        });
    }
}

async fn refuse(mut send: SendStream, device: &str, refusal: Refusal) {
    tracing::warn!(peer = %device, refusal = ?refusal, "stream-refused");
    let _ = send
        .write_all(&StreamAnswer::refuse(refusal).to_line())
        .await;
    let _ = send.finish();
    let _ = tokio::time::timeout(Duration::from_secs(5), send.stopped()).await;
}

async fn stream(net: Arc<Net>, device: String, send: SendStream, mut recv: RecvStream) {
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
    relay(&net, &device, &repository, &admitted, send, recv, server).await;
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
    };
    let mut filter = Filter::new(config, repository.checking);
    let (mut from_server, mut to_server) = server.into_split();
    let mut syncs: JoinSet<(u64, std::io::Result<()>)> = JoinSet::new();
    let mut client_buf = vec![0u8; 64 * 1024];
    let mut server_buf = vec![0u8; 64 * 1024];
    let mut client_open = true;
    'session: loop {
        let actions = tokio::select! {
            n = recv.read(&mut client_buf), if client_open => match n {
                Ok(Some(n)) if n > 0 => filter.from_client(&client_buf[..n]),
                _ => {
                    client_open = false;
                    let _ = to_server.shutdown().await;
                    Vec::new()
                }
            },
            n = from_server.read(&mut server_buf) => match n {
                Ok(n) if n > 0 => filter.from_server(&server_buf[..n]),
                _ => break 'session,
            },
            Some(done) = syncs.join_next(), if !syncs.is_empty() => match done {
                Ok((ticket, Ok(()))) => filter.synced(ticket),
                Ok((_, Err(e))) => {
                    tracing::error!(peer = %device, error = %e, "sync-failed");
                    break 'session;
                }
                Err(_) => break 'session,
            },
        };
        for action in actions {
            match action {
                Action::ToServer(bytes) => {
                    if to_server.write_all(&bytes).await.is_err() {
                        break 'session;
                    }
                }
                Action::ToClient(bytes) => {
                    if send.write_all(&bytes).await.is_err() {
                        break 'session;
                    }
                }
                Action::Sync { ticket, key } => {
                    let sync = net.syncer.sync(repository, &key);
                    syncs.spawn(async move { (ticket, sync.await) });
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
    let _ = to_server.shutdown().await;
    let _ = send.finish();
    let _ = tokio::time::timeout(Duration::from_secs(5), send.stopped()).await;
}
