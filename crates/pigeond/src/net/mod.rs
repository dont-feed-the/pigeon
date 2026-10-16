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

pub mod recovery;
mod relays;
pub mod serve;
pub mod socket;
pub mod sync;
mod telemetry;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use iroh::endpoint::{Connection, RelayMode, presets};
use iroh::{Endpoint, EndpointAddr, PublicKey, SecretKey};
use pigeon_core::filter::Role;
use pigeon_proto::join::{Join, JoinAnswer};
use pigeon_proto::stream::{
    ALPN_ANNEX, ALPN_JOIN, ALPN_RECOVERY, ALPN_TELEMETRY, MAX_HEADER, Refusal, StreamAnswer,
    StreamHeader,
};
use pigeon_proto::{Request, Response};
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
    /// (spec 09 §9.4).
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
    /// Optional collector of this group; no default data export.
    fn telemetry_device(&self, _group: &str) -> Option<String> {
        None
    }
    /// Keeps the direct addresses that `device`, admitted for `group`,
    /// gave for itself in its stream header.
    fn learn(&self, _group: &str, _device: &str, _addresses: Vec<SocketAddr>) {}
}

/// A boxed future, as the loops answer.
pub type Answer<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send>>;

/// What the loops of `pigeond` answer: the socket's requests that are not
/// the helper's, and the joins of new devices (09 §9.6).
pub trait Loops: Send + Sync + 'static {
    /// Answers one request of the local socket: progress, then the result.
    fn request(&self, request: Request) -> Answer<Vec<Response>>;
    /// Answers the join of `device`, authenticated by the QUIC handshake.
    fn join(&self, device: String, join: Join) -> Answer<JoinAnswer>;
    /// Read-only metadata for a recovery device authenticated by QUIC.
    fn recovery_bundle(&self, _group: String, _device: String) -> Answer<Option<recovery::Bundle>> {
        Box::pin(async { None })
    }
    /// Stores authenticated telemetry on the group's selected collector.
    fn telemetry(
        &self,
        _group: String,
        _device: String,
        _records: Vec<serde_json::Value>,
    ) -> Answer<bool> {
        Box::pin(async { false })
    }
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

/// How the endpoint finds peers beyond its direct addresses and mDNS (09
/// §9.5, ADR 0016).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reach {
    /// Direct addresses and mDNS only, as the lab and the tests run (13
    /// §13.12).
    Local,
    /// n0's public relays and address lookup: the default.
    N0,
    /// These relays instead of n0's, with n0's address lookup.
    Relays(Vec<String>),
}

/// How the endpoint is set up.
#[derive(Clone, Debug)]
pub struct NetConfig {
    /// The device key's 32-byte ed25519 secret.
    pub secret_key: [u8; 32],
    /// Where to bind the QUIC socket; port 0 picks one.
    pub bind: SocketAddr,
    /// Find peers on the local network with mDNS.
    pub mdns: bool,
    /// Relays and address lookup.
    pub reach: Reach,
}

/// The device's endpoint: it dials peers and accepts their streams.
#[derive(Clone, Debug)]
pub struct Node {
    endpoint: Endpoint,
    /// How it was set up to reach peers.
    reach: Reach,
    /// The relays it began with: n0's, under [`Reach::N0`].
    defaults: Vec<iroh::RelayUrl>,
    /// The relays it uses now.
    relays: Arc<tokio::sync::Mutex<Vec<iroh::RelayUrl>>>,
    connections: Arc<tokio::sync::Mutex<HashMap<PublicKey, Slot>>>,
    /// The last refusal of each peer, by group, until a stream is admitted.
    refused: Arc<Mutex<HashMap<(PublicKey, String), Refusal>>>,
}

impl Node {
    /// Binds the endpoint, with the relays and address lookup of
    /// `config.reach`.
    pub async fn bind(config: &NetConfig) -> Result<Node, NetError> {
        let (builder, map) = match &config.reach {
            Reach::Local => (
                Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled),
                iroh::RelayMap::empty(),
            ),
            Reach::N0 => (
                Endpoint::builder(presets::N0),
                RelayMode::Default.relay_map(),
            ),
            Reach::Relays(urls) => {
                let map = iroh::RelayMap::try_from_iter(urls.iter().map(String::as_str))
                    .map_err(|e| NetError::Bind(format!("relay: {e}")))?;
                (
                    Endpoint::builder(presets::N0).relay_mode(RelayMode::Custom(map.clone())),
                    map,
                )
            }
        };
        let defaults = map.urls::<Vec<_>>();
        let mut builder = builder
            .secret_key(SecretKey::from_bytes(&config.secret_key))
            .alpns(vec![
                ALPN_ANNEX.to_vec(),
                ALPN_JOIN.to_vec(),
                ALPN_RECOVERY.to_vec(),
                ALPN_TELEMETRY.to_vec(),
            ])
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
            reach: config.reach.clone(),
            relays: Arc::new(tokio::sync::Mutex::new(defaults.clone())),
            defaults,
            connections: Arc::default(),
            refused: Arc::default(),
        })
    }

    /// This device's ID.
    pub fn device_id(&self) -> String {
        device_id(&self.endpoint.id())
    }

    /// The direct addresses this device tells peers: its bound addresses,
    /// an unspecified one given as loopback (peers elsewhere find it with
    /// mDNS).
    pub fn advertised_addresses(&self) -> Vec<SocketAddr> {
        self.bound_addresses()
            .into_iter()
            .map(|a| {
                if a.ip().is_unspecified() {
                    let lo = if a.is_ipv4() {
                        std::net::IpAddr::from([127, 0, 0, 1])
                    } else {
                        std::net::IpAddr::from(std::net::Ipv6Addr::LOCALHOST)
                    };
                    SocketAddr::new(lo, a.port())
                } else {
                    a
                }
            })
            .collect()
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

    /// A stream to `device` for `group`, after its header was accepted. A
    /// cached connection that no longer answers (the peer restarted) is
    /// dropped and dialled again, once.
    pub async fn open_annex_stream(
        &self,
        device: &str,
        group: &str,
        addresses: Vec<SocketAddr>,
    ) -> Result<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream), NetError> {
        let key = parse_device_id(device).ok_or_else(|| NetError::Connect("device id".into()))?;
        let mut tries = 0;
        loop {
            tries += 1;
            let connection = self.connection(key, addresses.clone()).await?;
            let own = self.advertised_addresses();
            let admitted = Self::admit(&connection, group, &own);
            match tokio::time::timeout(CONNECT_TIMEOUT, admitted).await {
                Ok(Ok(pair)) => {
                    self.note(key, group, None);
                    return Ok(pair);
                }
                Ok(Err(e @ NetError::Refused(_))) => {
                    if let NetError::Refused(r) = &e {
                        self.note(key, group, *r);
                    }
                    return Err(e);
                }
                Ok(Err(e)) if tries > 1 => return Err(e),
                Err(_) if tries > 1 => {
                    return Err(NetError::Connect("peer-unreachable: no answer".into()));
                }
                _ => {
                    connection.close(0u32.into(), b"stale");
                    tracing::debug!(peer = %device, "connection-stale");
                }
            }
        }
    }

    fn note(&self, key: PublicKey, group: &str, refusal: Option<Refusal>) {
        let mut r = self.refused.lock().unwrap_or_else(|p| p.into_inner());
        match refusal {
            Some(why) => r.insert((key, group.to_owned()), why),
            None => r.remove(&(key, group.to_owned())),
        };
    }

    /// Why `device` last refused a stream for `group`, unless it admitted
    /// one since.
    pub fn refusal(&self, device: &str, group: &str) -> Option<Refusal> {
        let key = parse_device_id(device)?;
        let r = self.refused.lock().unwrap_or_else(|p| p.into_inner());
        r.get(&(key, group.to_owned())).copied()
    }

    /// Opens a stream on `connection` and has its header accepted.
    async fn admit(
        connection: &Connection,
        group: &str,
        own: &[SocketAddr],
    ) -> Result<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream), NetError> {
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|e| NetError::Connect(e.to_string()))?;
        let header = StreamHeader {
            v: pigeon_proto::stream::HEADER_VERSION,
            group: group.to_owned(),
            traceparent: pigeon_telemetry::traceparent(),
            addresses: own.iter().map(ToString::to_string).collect(),
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

    /// Sends `join` to the inviter `device` on `pigeon/join/1` and reads its
    /// answer, on a connection of its own.
    pub async fn join(
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

    /// Ships one bounded batch to a collector on its own authenticated ALPN.
    /// It never shares an annex connection or delays an annex transfer.
    pub async fn telemetry(
        &self,
        device: &str,
        group: &str,
        addresses: Vec<SocketAddr>,
        records: &[serde_json::Value],
    ) -> Result<(), NetError> {
        let payload = serde_json::to_vec(records).map_err(|e| NetError::Connect(e.to_string()))?;
        if payload.len() > telemetry::MAX_BATCH {
            return Err(NetError::Connect("telemetry batch too large".into()));
        }
        let key = parse_device_id(device).ok_or_else(|| NetError::Connect("device id".into()))?;
        let mut address = EndpointAddr::new(key);
        for a in addresses {
            address = address.with_ip_addr(a);
        }
        let exchange = async {
            let connection = self
                .endpoint
                .connect(address, ALPN_TELEMETRY)
                .await
                .map_err(|e| NetError::Connect(e.to_string()))?;
            let (mut send, mut recv) = connection
                .open_bi()
                .await
                .map_err(|e| NetError::Connect(e.to_string()))?;
            let header = StreamHeader {
                v: pigeon_proto::stream::HEADER_VERSION,
                group: group.to_owned(),
                traceparent: pigeon_telemetry::traceparent(),
                addresses: Vec::new(),
            };
            send.write_all(&header.to_line())
                .await
                .map_err(|e| NetError::Connect(e.to_string()))?;
            let answer = read_line(&mut recv).await?;
            let answer: StreamAnswer =
                serde_json::from_slice(&answer).map_err(|e| NetError::Connect(e.to_string()))?;
            if !answer.ok {
                return Err(NetError::Refused(answer.refusal));
            }
            send.write_all(&payload)
                .await
                .map_err(|e| NetError::Connect(e.to_string()))?;
            send.finish()
                .map_err(|e| NetError::Connect(e.to_string()))?;
            let ack = read_line(&mut recv).await?;
            connection.close(0u32.into(), b"telemetry complete");
            if ack == b"ok" {
                Ok(())
            } else {
                Err(NetError::Connect("telemetry refused".into()))
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(15), exchange)
            .await
            .map_err(|_| NetError::Connect("telemetry timeout".into()))?
    }

    /// The open connection to `key`, or a new one. Each peer has a slot
    /// of its own, so that dialling one that is offline, which waits until
    /// [`CONNECT_TIMEOUT`], never holds up the streams to the others.
    async fn connection(
        &self,
        key: PublicKey,
        addresses: Vec<SocketAddr>,
    ) -> Result<Connection, NetError> {
        let slot = self
            .connections
            .lock()
            .await
            .entry(key)
            .or_default()
            .clone();
        let mut slot = slot.lock().await;
        if let Some(c) = slot.as_ref()
            && c.close_reason().is_none()
        {
            return Ok(c.clone());
        }
        let mut addr = EndpointAddr::new(key);
        for a in addresses {
            addr = addr.with_ip_addr(a);
        }
        let c = tokio::time::timeout(CONNECT_TIMEOUT, self.endpoint.connect(addr, ALPN_ANNEX))
            .await
            .map_err(|_| NetError::Connect("peer-unreachable: timed out".into()))?
            .map_err(|e| NetError::Connect(e.to_string()))?;
        *slot = Some(c.clone());
        Ok(c)
    }
}

/// One peer's connection, dialled at most once at a time.
type Slot = Arc<tokio::sync::Mutex<Option<Connection>>>;

/// How long dialling a peer may take before it counts as unreachable.
pub const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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
