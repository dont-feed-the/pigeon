//! The network of a device (09): its iroh endpoint, the local socket that
//! the transport helper speaks to, and the filtered relay to git-annex.
//!
//! `pigeond` owns the device's only endpoint (ADR 0011). git-annex reaches
//! a peer through `git-annex-p2p-iroh`, which asks this module over the
//! local socket for a stream ([`socket`]); each incoming stream is admitted
//! by the roster of the group its header names, then relayed through the
//! filter of `pigeon_core::filter` to the repository's remotedaemon
//! ([`serve`]). What the roster says comes from a [`Directory`]; the loops
//! of `pigeond` provide it, and tests use [`StaticDirectory`].

pub mod serve;
pub mod socket;
pub mod sync;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use iroh::endpoint::{Connection, RelayMode, presets};
use iroh::{Endpoint, EndpointAddr, PublicKey, SecretKey};
use pigeon_core::filter::Role;
use pigeon_proto::stream::{ALPN_ANNEX, MAX_HEADER, Refusal, StreamAnswer, StreamHeader};
use tokio::io::{AsyncRead, AsyncReadExt};

/// A repository this device serves, as the loops know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    /// The group it belongs to.
    pub group: String,
    /// Its git directory, canonical: `.git` of a laptop, the bare repository
    /// of a custodian.
    pub git_dir: PathBuf,
    /// Its working tree, if it has one.
    pub worktree: Option<PathBuf>,
    /// This device's role in the group.
    pub role: Role,
    /// The token its git-annex accepts (09 §9.2).
    pub auth_token: Option<String>,
    /// Whether it is being re-checked (06 §6.3).
    pub checking: bool,
}

/// A device the roster admits for a group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admitted {
    /// Its git-annex UUID in that group, from the roster.
    pub annex_uuid: Option<String>,
    /// Whether it is a custodian of that group: its locks are hashed
    /// (ADR 0054).
    pub custodian: bool,
}

/// What the network needs to know of the groups; the loops provide it.
pub trait Directory: Send + Sync + 'static {
    /// The repository at `path`, its git directory, working tree or inside.
    fn repository(&self, path: &Path) -> Option<Repository>;
    /// This device's repository of `group`.
    fn group(&self, group: &str) -> Option<Repository>;
    /// Whether `device` may open streams for `group`.
    fn admit(&self, group: &str, device: &str) -> Option<Admitted>;
    /// Direct addresses known for `device`, besides what mDNS finds.
    fn addresses(&self, device: &str) -> Vec<SocketAddr>;
}

/// A [`Directory`] held in memory, set up by its owner.
#[derive(Debug, Default)]
pub struct StaticDirectory {
    inner: Mutex<StaticInner>,
}

#[derive(Debug, Default)]
struct StaticInner {
    repositories: Vec<Repository>,
    admitted: HashMap<(String, String), Admitted>,
    addresses: HashMap<String, Vec<SocketAddr>>,
}

impl StaticDirectory {
    /// An empty directory.
    pub fn new() -> Self {
        StaticDirectory::default()
    }

    /// Serves `repository`; its paths are made canonical.
    pub fn add_repository(&self, mut repository: Repository) {
        repository.git_dir = canonical(&repository.git_dir);
        repository.worktree = repository.worktree.as_deref().map(canonical);
        self.lock().repositories.push(repository);
    }

    /// Admits `device` for `group`.
    pub fn admit_device(&self, group: &str, device: &str, admitted: Admitted) {
        self.lock()
            .admitted
            .insert((group.to_owned(), device.to_owned()), admitted);
    }

    /// Records direct addresses of `device`.
    pub fn set_addresses(&self, device: &str, addresses: Vec<SocketAddr>) {
        self.lock().addresses.insert(device.to_owned(), addresses);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, StaticInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Directory for StaticDirectory {
    fn repository(&self, path: &Path) -> Option<Repository> {
        let path = canonical(path);
        self.lock()
            .repositories
            .iter()
            .find(|r| {
                path.starts_with(&r.git_dir)
                    || r.worktree.as_ref().is_some_and(|w| path.starts_with(w))
            })
            .cloned()
    }

    fn group(&self, group: &str) -> Option<Repository> {
        self.lock()
            .repositories
            .iter()
            .find(|r| r.group == group)
            .cloned()
    }

    fn admit(&self, group: &str, device: &str) -> Option<Admitted> {
        self.lock()
            .admitted
            .get(&(group.to_owned(), device.to_owned()))
            .cloned()
    }

    fn addresses(&self, device: &str) -> Vec<SocketAddr> {
        self.lock()
            .addresses
            .get(device)
            .cloned()
            .unwrap_or_default()
    }
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A device ID: its ed25519 public key as 64 lowercase hex digits (09 §9.1).
pub fn device_id(key: &PublicKey) -> String {
    key.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

/// The public key of a device ID, if it is 64 hex digits of a valid key.
pub fn parse_device_id(id: &str) -> Option<PublicKey> {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(id.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    PublicKey::from_bytes(&bytes).ok()
}

/// Errors of the network.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    /// The endpoint could not bind.
    #[error("bind: {0}")]
    Bind(String),
    /// A peer could not be reached.
    #[error("connect: {0}")]
    Connect(String),
    /// A peer refused the stream.
    #[error("refused: {0:?}")]
    Refused(Option<Refusal>),
    /// Local I/O failed.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// How the endpoint is set up (13 §13.12: direct addresses and mDNS only).
#[derive(Clone, Debug)]
pub struct NetConfig {
    /// The device key's 32-byte ed25519 secret.
    pub secret_key: [u8; 32],
    /// Where to bind the QUIC socket; port 0 picks one.
    pub bind: SocketAddr,
    /// Find peers on the local network with mDNS.
    pub mdns: bool,
}

/// The device's endpoint: it dials peers and accepts their streams.
#[derive(Clone, Debug)]
pub struct Node {
    endpoint: Endpoint,
    connections: Arc<tokio::sync::Mutex<HashMap<PublicKey, Connection>>>,
}

impl Node {
    /// Binds the endpoint, without relays or global discovery.
    pub async fn bind(config: &NetConfig) -> Result<Node, NetError> {
        let mut builder = Endpoint::builder(presets::Minimal)
            .secret_key(SecretKey::from_bytes(&config.secret_key))
            .alpns(vec![ALPN_ANNEX.to_vec()])
            .relay_mode(RelayMode::Disabled)
            .clear_ip_transports()
            .bind_addr(config.bind)
            .map_err(|e| NetError::Bind(e.to_string()))?;
        if config.mdns {
            builder =
                builder.address_lookup(iroh_mdns_address_lookup::MdnsAddressLookup::builder());
        }
        let endpoint = builder
            .bind()
            .await
            .map_err(|e| NetError::Bind(e.to_string()))?;
        Ok(Node {
            endpoint,
            connections: Arc::default(),
        })
    }

    /// This device's ID.
    pub fn device_id(&self) -> String {
        device_id(&self.endpoint.id())
    }

    /// The local addresses the endpoint is bound to.
    pub fn bound_addresses(&self) -> Vec<SocketAddr> {
        self.endpoint.bound_sockets()
    }

    /// The underlying endpoint, for the accept loop.
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Closes the endpoint and every connection.
    pub async fn close(&self) {
        self.endpoint.close().await;
    }

    /// A stream to `device` for `group`, after its header was accepted.
    pub async fn open_annex_stream(
        &self,
        device: &str,
        group: &str,
        addresses: Vec<SocketAddr>,
    ) -> Result<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream), NetError> {
        let key = parse_device_id(device).ok_or_else(|| NetError::Connect("device id".into()))?;
        let connection = self.connection(key, addresses).await?;
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|e| NetError::Connect(e.to_string()))?;
        let header = StreamHeader {
            v: pigeon_proto::stream::HEADER_VERSION,
            group: group.to_owned(),
            traceparent: None,
        };
        send.write_all(&header.to_line())
            .await
            .map_err(|e| NetError::Connect(e.to_string()))?;
        let line = read_line(&mut recv).await?;
        let answer: StreamAnswer =
            serde_json::from_slice(&line).map_err(|e| NetError::Connect(e.to_string()))?;
        if !answer.ok {
            return Err(NetError::Refused(answer.refusal));
        }
        Ok((send, recv))
    }

    async fn connection(
        &self,
        key: PublicKey,
        addresses: Vec<SocketAddr>,
    ) -> Result<Connection, NetError> {
        let mut connections = self.connections.lock().await;
        if let Some(c) = connections.get(&key)
            && c.close_reason().is_none()
        {
            return Ok(c.clone());
        }
        let mut addr = EndpointAddr::new(key);
        for a in addresses {
            addr = addr.with_ip_addr(a);
        }
        let c = self
            .endpoint
            .connect(addr, ALPN_ANNEX)
            .await
            .map_err(|e| NetError::Connect(e.to_string()))?;
        connections.insert(key, c.clone());
        Ok(c)
    }
}

/// Reads one line of at most [`MAX_HEADER`] bytes, its newline dropped,
/// without reading past it.
pub async fn read_line<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, NetError> {
    let mut line = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            return Err(NetError::Connect("end of stream in header".into()));
        }
        if byte[0] == b'\n' {
            return Ok(line);
        }
        line.push(byte[0]);
        if line.len() > MAX_HEADER {
            return Err(NetError::Connect("header too long".into()));
        }
    }
}
