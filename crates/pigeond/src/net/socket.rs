//! The local socket of `pigeond` (09 §9.3), as the transport helper uses it.
//!
//! One JSON request per line; after `Connected` or `Listening` the
//! connection carries raw bytes. `Address` answers this device's ID,
//! `Connect` opens a stream to a peer and relays it, `Listen` registers
//! remotedaemon's socket for the repository's group until the helper's
//! connection ends. Other requests go to the loops ([`super::Loops`]), and
//! without them are answered `not-in-this-build` (13 §13.12). Only the same
//! user may connect.

use std::collections::{BTreeMap, HashMap};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pigeon_proto::{LineDecoder, Message, PROTOCOL_VERSION, Problem, Request, Response, encode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::{UnixListener, UnixStream};

use super::{Directory, Loops, NetError, Node, sync::Syncer};

/// remotedaemon's sockets, by group, as the helper registered them.
#[derive(Debug, Default)]
pub struct Listeners {
    inner: Mutex<HashMap<String, (u64, PathBuf)>>,
    next: Mutex<u64>,
}

impl Listeners {
    /// The socket registered for `group`.
    pub fn get(&self, group: &str) -> Option<PathBuf> {
        lock(&self.inner).get(group).map(|(_, p)| p.clone())
    }

    fn register(&self, group: &str, socket: PathBuf) -> u64 {
        let mut next = lock(&self.next);
        *next += 1;
        lock(&self.inner).insert(group.to_owned(), (*next, socket));
        *next
    }

    fn unregister(&self, group: &str, id: u64) {
        let mut inner = lock(&self.inner);
        if inner.get(group).is_some_and(|(i, _)| *i == id) {
            inner.remove(group);
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Everything the network part of `pigeond` shares between its tasks.
pub struct Net {
    /// The device's endpoint.
    pub node: Node,
    /// What the roster says.
    pub directory: Arc<dyn Directory>,
    /// remotedaemon's sockets.
    pub listeners: Listeners,
    /// Syncs objects before positive answers.
    pub syncer: Arc<dyn Syncer>,
    /// The local socket's path.
    pub socket_path: PathBuf,
    /// The loops, which answer the other requests; without them those are
    /// answered `not-in-this-build`.
    pub loops: Option<Arc<dyn Loops>>,
}

impl std::fmt::Debug for Net {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Net")
            .field("socket_path", &self.socket_path)
            .finish_non_exhaustive()
    }
}

/// Binds the local socket at `path`, readable by the user only.
pub fn bind_socket(path: &Path) -> std::io::Result<UnixListener> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Serves the local socket until the listener fails.
pub async fn serve_socket(net: Arc<Net>, listener: UnixListener) {
    while let Ok((stream, _)) = listener.accept().await {
        let net = net.clone();
        tokio::spawn(async move {
            if let Err(e) = client(net, stream).await {
                tracing::debug!(error = %e, "socket-client-ended");
            }
        });
    }
}

/// The longest unix socket path every platform connects to: `sun_path`
/// holds 104 bytes on macOS and 108 on Linux, the final NUL included.
const MAX_SOCKET_PATH: usize = 100;

/// A short symbolic link next to `pigeond`'s socket for remotedaemon's
/// socket, when that path is too long to connect to; `None` if it is not.
fn short_link(own: &Path, group: &str, socket: &Path) -> std::io::Result<Option<PathBuf>> {
    if socket.as_os_str().len() <= MAX_SOCKET_PATH {
        return Ok(None);
    }
    let name: String = group
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(16)
        .collect();
    let link = own.with_file_name(format!("annex-{name}.sock"));
    if link.symlink_metadata().is_ok() {
        std::fs::remove_file(&link)?;
    }
    std::os::unix::fs::symlink(socket, &link)?;
    Ok(Some(link))
}

fn problem(code: &str, details: Option<String>) -> Response {
    Response::Problem(Problem {
        code: code.to_owned(),
        message: Message {
            id: format!("problem-{code}"),
            args: BTreeMap::new(),
        },
        hint: None,
        details,
    })
}

async fn reply(w: &mut OwnedWriteHalf, id: u64, response: &Response) -> Result<(), NetError> {
    let line = encode(id, response).map_err(|e| NetError::Connect(e.to_string()))?;
    w.write_all(&line).await?;
    Ok(())
}

async fn client(net: Arc<Net>, stream: UnixStream) -> Result<(), NetError> {
    let owner = std::fs::metadata(&net.socket_path)?.uid();
    if stream.peer_cred()?.uid() != owner {
        return Ok(());
    }
    let (mut rd, mut wr) = stream.into_split();
    let mut decoder = LineDecoder::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let request = loop {
            match decoder.next_line::<Request>() {
                Ok(Some(r)) => break r,
                Ok(None) => {}
                Err(_) => return Ok(()),
            }
            let n = rd.read(&mut buf).await?;
            if n == 0 {
                return Ok(());
            }
            decoder.push(&buf[..n]);
        };
        let id = request.id;
        match request.body {
            Request::Hello { .. } => {
                let hello = Response::Hello {
                    version: PROTOCOL_VERSION,
                };
                reply(&mut wr, id, &hello).await?;
            }
            Request::Address { .. } => {
                let device = net.node.device_id();
                reply(&mut wr, id, &Response::Address { device }).await?;
            }
            Request::Connect { repository, device } => {
                let Some(repo) = net.directory.repository(Path::new(&repository)) else {
                    return reply(&mut wr, id, &problem("not-a-pigeon-repository", None)).await;
                };
                let addresses = net.directory.addresses(&device);
                let opened = net
                    .node
                    .open_annex_stream(&device, &repo.group, addresses)
                    .await;
                let (mut send, mut recv) = match opened {
                    Ok(s) => s,
                    Err(NetError::Refused(r)) => {
                        let code = r.map_or("peer-refused".to_owned(), |r| {
                            serde_json::to_value(r)
                                .ok()
                                .and_then(|v| v.as_str().map(str::to_owned))
                                .unwrap_or_default()
                        });
                        return reply(&mut wr, id, &problem(&code, None)).await;
                    }
                    Err(e) => {
                        let p = problem("peer-unreachable", Some(e.to_string()));
                        return reply(&mut wr, id, &p).await;
                    }
                };
                reply(&mut wr, id, &Response::Connected).await?;
                let rest = std::mem::take(&mut decoder).into_rest();
                let up = async {
                    if !rest.is_empty() {
                        send.write_all(&rest).await.map_err(std::io::Error::other)?;
                    }
                    tokio::io::copy(&mut rd, &mut send).await?;
                    send.finish().map_err(std::io::Error::other)?;
                    Ok::<_, std::io::Error>(())
                };
                let down = async {
                    tokio::io::copy(&mut recv, &mut wr).await?;
                    wr.shutdown().await?;
                    Ok::<_, std::io::Error>(())
                };
                let (a, b) = tokio::join!(up, down);
                a?;
                b?;
                return Ok(());
            }
            Request::Listen { repository, socket } => {
                let Some(repo) = net.directory.repository(Path::new(&repository)) else {
                    return reply(&mut wr, id, &problem("not-a-pigeon-repository", None)).await;
                };
                let mut path = PathBuf::from(&socket);
                if path.is_relative() {
                    path = repo
                        .worktree
                        .clone()
                        .unwrap_or_else(|| repo.git_dir.clone())
                        .join(path);
                }
                let link = short_link(&net.socket_path, &repo.group, &path)?;
                let target = link.clone().unwrap_or(path);
                let registration = net.listeners.register(&repo.group, target);
                reply(&mut wr, id, &Response::Listening).await?;
                while rd.read(&mut buf).await.map(|n| n > 0).unwrap_or(false) {}
                net.listeners.unregister(&repo.group, registration);
                if let Some(link) = link {
                    let _ = std::fs::remove_file(link);
                }
                return Ok(());
            }
            other => match &net.loops {
                Some(loops) => {
                    for response in loops.request(other).await {
                        reply(&mut wr, id, &response).await?;
                    }
                }
                None => reply(&mut wr, id, &problem("not-in-this-build", None)).await?,
            },
        }
    }
}
