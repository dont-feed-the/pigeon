//! `pigeond` itself: its directories, settings and device key, the iroh
//! endpoint, the local socket, and the rounds of the loops (08 §8.1).
//!
//! A round runs, for each group in turn, the settings check, free and keep
//! toward each folder's target and capture (on a laptop), accept, place,
//! re-thinning (on a laptop) and repair; requests that change a repository
//! wait for its round to end. Rounds start every `[loops]
//! interval_seconds`, or at once after a request or a peer's change notice
//! (remotedaemon) asks for one (13 §13.12).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use pigeon_core::filter::Role;
use pigeon_core::roster::{DeviceId, Role as DeviceRole};
use pigeon_git::GitEnv;
use pigeon_proto::stream::Refusal;
use pigeon_sys::paths::Dirs;

use crate::config::{self, Config, GroupEntry};
use crate::group::Group;
use crate::net::serve::accept_loop;
use crate::net::socket::{Listeners, Net, bind_socket, serve_socket};
use crate::net::sync::AnnexSyncer;
use crate::net::{Admitted, Directory, NetConfig, Node, Repository};
use crate::trace::Tracer;

/// The running daemon.
#[derive(Debug)]
pub struct Daemon {
    /// Its directories.
    pub dirs: Dirs,
    /// The settings it started with.
    pub config: Config,
    /// This device's ID.
    pub device: String,
    /// How git and git-annex are started.
    pub env: GitEnv,
    /// The groups of this device.
    pub groups: RwLock<Vec<Arc<Group>>>,
    /// The endpoint.
    pub node: Node,
    /// The network part, once it is up.
    pub net: OnceLock<Arc<Net>>,
    /// Starts a round at once: a request, or a peer's change notice.
    pub wake: Arc<tokio::sync::Notify>,
    /// Trace events.
    pub tracer: Tracer,
}

impl Daemon {
    /// The groups.
    pub fn groups(&self) -> Vec<Arc<Group>> {
        self.groups
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// The group with ID `id`.
    pub fn group(&self, id: &str) -> Option<Arc<Group>> {
        self.groups().into_iter().find(|g| g.id == id)
    }

    /// The group whose folder or repository holds `path`.
    pub fn group_at(&self, path: &Path) -> Option<Arc<Group>> {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.groups().into_iter().find(|g| {
            path.starts_with(&g.git_dir) || g.worktree.as_ref().is_some_and(|w| path.starts_with(w))
        })
    }

    /// The group a request without a path is about: the first laptop
    /// group, else the first group.
    pub fn default_group(&self) -> Option<Arc<Group>> {
        let groups = self.groups();
        groups
            .iter()
            .find(|g| g.role == Role::Laptop)
            .or(groups.first())
            .cloned()
    }

    /// Serves a new group: records it in `config.toml` and starts its loops.
    pub async fn add_group(&self, g: Group, path: &Path) -> Arc<Group> {
        let entry = GroupEntry {
            id: g.id.clone(),
            path: path.to_path_buf(),
        };
        if let Err(e) = config::add_group(&self.dirs.config_file(), &entry) {
            tracing::warn!(error = %e, "config-write-failed");
        }
        let g = Arc::new(g);
        crate::settings::check(&g).await;
        g.ensure_remotedaemon(&self.wake).await;
        self.groups
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .push(g.clone());
        self.wake.notify_one();
        g
    }

    /// The direct addresses this device tells peers.
    pub fn addresses(&self) -> Vec<SocketAddr> {
        self.node.advertised_addresses()
    }

    /// One round of the loops for `g`.
    pub async fn round(&self, g: &Group) {
        let _busy = g.busy.lock().await;
        crate::settings::check(g).await;
        g.ensure_remotedaemon(&self.wake).await;
        g.reload_roster().await;
        if g.role == Role::Laptop {
            crate::evict::free_targets(g, &self.device, &self.tracer).await;
            if let Err(e) = crate::evict::keep(g, &self.tracer).await {
                tracing::info!(group = %g.id, error = %e, "keep-waits");
            }
            if let Err(e) = crate::capture::capture(g, &self.tracer).await {
                tracing::warn!(group = %g.id, error = %e, "capture-failed");
            }
        }
        crate::custody::accept(g, &self.device, &self.tracer).await;
        self.note_lost(g);
        crate::custody::place(g, &self.device).await;
        if g.role == Role::Laptop {
            crate::thin::rethin(g, &self.device, &self.tracer).await;
        }
        crate::repair::repair(g, &self.device).await;
    }
}

impl Daemon {
    /// Marks lost each peer of `g` whose last fetch failed because it
    /// answers but serves the group no more (`UnknownGroup`); one that
    /// serves it again is lost no more.
    fn note_lost(&self, g: &Group) {
        let unreachable = g.lock().unreachable.clone();
        let lost = unreachable
            .into_iter()
            .filter(|p| self.node.refusal(p, &g.id) == Some(Refusal::UnknownGroup))
            .collect::<std::collections::BTreeSet<_>>();
        let mut k = g.lock();
        if k.lost != lost {
            tracing::warn!(group = %g.id, lost = ?lost, "repository-lost");
        }
        k.lost = lost;
    }
}

impl Directory for Daemon {
    fn repository(&self, path: &Path) -> Option<Repository> {
        self.group_at(path).map(|g| g.repository())
    }

    fn group(&self, group: &str) -> Option<Repository> {
        Daemon::group(self, group).map(|g| g.repository())
    }

    fn admit(&self, group: &str, device: &str) -> Option<Admitted> {
        let g = Daemon::group(self, group)?;
        let doc = g.roster()?;
        let d = doc.roster.devices.get(&DeviceId(device.to_owned()))?;
        (d.role != DeviceRole::Recovery).then(|| Admitted {
            annex_uuid: d.annex_uuid.clone(),
            custodian: d.role == DeviceRole::Custodian,
        })
    }

    fn learn(&self, group: &str, device: &str, addresses: Vec<SocketAddr>) {
        if let Some(g) = Daemon::group(self, group)
            && !addresses.is_empty()
            && g.lock().addresses.get(device) != Some(&addresses)
        {
            g.set_addresses(device, addresses);
        }
    }

    fn addresses(&self, device: &str) -> Vec<SocketAddr> {
        let mut out: Vec<SocketAddr> = Vec::new();
        for g in self.groups() {
            if let Some(a) = g.lock().addresses.get(device) {
                out.extend(a.iter().copied());
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// The directories under `home`, for tests and the lab: `config/`,
/// `state/`, `cache/` and `run/`.
pub fn dirs_under(home: &Path) -> Dirs {
    Dirs {
        config: home.join("config"),
        state: home.join("state"),
        cache: home.join("cache"),
        runtime: home.join("run"),
    }
}

/// The directory first in the children's `PATH` (14 §14.3): `<cache>/bin`,
/// with a link to the transport helper next to `pigeond` and, when the
/// git-annex on `PATH` installs none (Homebrew), a `git-remote-p2p-annex`
/// link to `git-annex`, which dispatches on its name. Without it, the
/// directory of `pigeond` itself.
fn child_bin(dirs: &Dirs) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let bin = dirs.cache.join("bin");
    if std::fs::create_dir_all(&bin).is_err() {
        return exe_dir;
    }
    let link = |name: &str, target: Option<PathBuf>| {
        let at = bin.join(name);
        let _ = std::fs::remove_file(&at);
        if let Some(target) = target
            && let Err(e) = std::os::unix::fs::symlink(&target, &at)
        {
            tracing::warn!(link = %at.display(), error = %e, "link-failed");
        }
    };
    let helper = exe_dir
        .as_ref()
        .map(|d| d.join("git-annex-p2p-iroh"))
        .filter(|p| p.is_file());
    link("git-annex-p2p-iroh", helper);
    let path = std::env::var_os("PATH").unwrap_or_default();
    let find = |name: &str| {
        std::env::split_paths(&path)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    };
    let remote = find("git-remote-p2p-annex")
        .is_none()
        .then(|| find("git-annex"))
        .flatten();
    if remote.is_some() {
        tracing::info!("git-remote-p2p-annex-linked");
    }
    link("git-remote-p2p-annex", remote);
    Some(bin)
}

/// Binds the endpoint. With port 0 it binds the port it picked before, if
/// that is still free, and keeps the one it now has: peers keep this
/// device's direct addresses (09 §9.3), and a restart, a crash's included,
/// must not leave them dialling a port nobody listens on (S9).
async fn bind_node(
    dirs: &Dirs,
    secret: [u8; 32],
    bind: SocketAddr,
    mdns: bool,
) -> Result<Node, String> {
    let at = |bind| NetConfig {
        secret_key: secret,
        bind,
        mdns,
    };
    let kept = std::fs::read_to_string(dirs.port())
        .ok()
        .and_then(|t| t.trim().parse::<u16>().ok())
        .filter(|_| bind.port() == 0);
    let node = match kept {
        Some(port) => {
            let mut again = bind;
            again.set_port(port);
            match Node::bind(&at(again)).await {
                Ok(n) => n,
                Err(e) => {
                    tracing::info!(port, error = %e, "kept-port-taken");
                    Node::bind(&at(bind)).await.map_err(|e| e.to_string())?
                }
            }
        }
        None => Node::bind(&at(bind)).await.map_err(|e| e.to_string())?,
    };
    if bind.port() == 0
        && let Some(port) = node.bound_addresses().first().map(SocketAddr::port)
    {
        let _ = std::fs::create_dir_all(&dirs.state);
        let _ = pigeon_sys::fs::write_atomic(&dirs.port(), port.to_string().as_bytes());
    }
    Ok(node)
}

/// Runs `pigeond` until it is told to stop.
pub async fn run(dirs: Dirs) -> Result<(), String> {
    crate::logs::init(dirs.logs());
    let config = config::load(&dirs.config_file()).map_err(|e| e.to_string())?;
    let secret = crate::util::device_key(&dirs.device_key()).map_err(|e| e.to_string())?;
    let bind: SocketAddr = config
        .network
        .bind
        .parse()
        .map_err(|e| format!("[network] bind: {e}"))?;
    let node = bind_node(&dirs, secret, bind, config.network.mdns).await?;
    let socket = std::env::var_os("PIGEON_SOCKET")
        .filter(|s| !s.is_empty())
        .map_or_else(|| dirs.socket(), PathBuf::from);
    if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
        return Err(format!("pigeond already runs on {}", socket.display()));
    }
    let env = GitEnv {
        path_first: child_bin(&dirs),
        extra: vec![("PIGEON_SOCKET".into(), socket.clone().into_os_string())],
    };
    let device = node.device_id();
    let tracer = Tracer::new(dirs.traces(), &device);
    let mut groups = Vec::new();
    for entry in &config.groups {
        match Group::open(&entry.id, &entry.path, &env).await {
            Some(g) => groups.push(Arc::new(g)),
            None => {
                tracing::warn!(group = %entry.id, path = %entry.path.display(), "group-missing")
            }
        }
    }
    let interval = Duration::from_secs(config.loops.interval_seconds.max(1));
    let daemon = Arc::new(Daemon {
        dirs,
        config,
        device: device.clone(),
        env,
        groups: RwLock::new(groups),
        node: node.clone(),
        net: OnceLock::new(),
        wake: Arc::new(tokio::sync::Notify::new()),
        tracer,
    });
    // Each group's install journal is replayed while the socket already
    // answers: whatever needs the worktree waits on `busy`, held from here.
    let groups = daemon.groups();
    let mut held = Vec::new();
    for g in &groups {
        held.push(g.busy.lock().await);
    }
    let listener = bind_socket(&socket).map_err(|e| format!("{}: {e}", socket.display()))?;
    let net = Arc::new(Net {
        node: node.clone(),
        directory: daemon.clone(),
        listeners: Listeners::default(),
        syncer: Arc::new(AnnexSyncer),
        socket_path: socket.clone(),
        loops: Some(Arc::new(crate::api::Api(daemon.clone()))),
    });
    let _ = daemon.net.set(net.clone());
    tokio::spawn(serve_socket(net.clone(), listener));
    tokio::spawn(accept_loop(net.clone()));
    tracing::info!(device = %device, socket = %socket.display(), "started");
    for (g, busy) in groups.iter().zip(held) {
        crate::evict::recover(g, &daemon.tracer).await;
        drop(busy);
    }
    let rounds = daemon.clone();
    tokio::spawn(async move {
        loop {
            for g in rounds.groups() {
                rounds.round(&g).await;
            }
            tokio::select! {
                () = tokio::time::sleep(interval) => {}
                () = rounds.wake.notified() => {}
            }
        }
    });
    stop_signal().await;
    tracing::info!("stopping");
    for g in daemon.groups() {
        g.stop_remotedaemon().await;
    }
    node.close().await;
    let _ = std::fs::remove_file(&socket);
    Ok(())
}

async fn stop_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    match signal(SignalKind::terminate()) {
        Ok(mut term) => {
            tokio::select! {
                _ = term.recv() => {}
                _ = tokio::signal::ctrl_c() => {}
            }
        }
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}
