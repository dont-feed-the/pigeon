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
    /// The web interface, once it listens (10 §10.3).
    pub web: OnceLock<Arc<pigeon_web::Web>>,
    /// Starts a round at once: a request, or a peer's change notice.
    pub wake: Arc<tokio::sync::Notify>,
    /// One-use diagnostic permissions granted by this computer’s user.
    pub debug: crate::debug::Permissions,
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

    /// A refounding changes main's genesis before replacing the served group.
    /// An old round must stop even while that replacement is still pending.
    async fn current_group(&self, g: &Arc<Group>) -> bool {
        if !self
            .group(&g.id)
            .is_some_and(|current| Arc::ptr_eq(&current, g))
        {
            return false;
        }
        let Some(main) = g.git.rev("refs/heads/main").await else {
            return false;
        };
        g.git.root(&main).await.as_deref() == Some(g.id.as_str())
    }

    /// Serves a new group: records it in `config.toml` and starts its loops.
    pub async fn add_group(&self, mut g: Group, path: &Path) -> Result<Arc<Group>, String> {
        crate::shared::configure(&mut g, self.config.bridge_storage.as_ref())?;
        self.keep_key()?;
        let entry = GroupEntry {
            id: g.id.clone(),
            path: path.to_path_buf(),
        };
        if let Err(e) = config::add_group(&self.dirs.config_file(), &entry) {
            tracing::warn!(error = %e, "config-write-failed");
        }
        let g = Arc::new(g);
        self.serve_group(g.clone(), None).await;
        Ok(g)
    }

    /// Writes the key this daemon runs with back to `<config>/device-key`
    /// when leaving the last group deleted it (09 §9.7): the new group's
    /// roster names this device, which must stay the same after a restart.
    pub(crate) fn keep_key(&self) -> Result<(), String> {
        let path = self.dirs.device_key();
        if path.exists() {
            return Ok(());
        }
        let Some(key) = crate::git::signer() else {
            return Ok(());
        };
        crate::util::write_private(&path, format!("{}\n", crate::util::hex(&key)).as_bytes())
            .map_err(|e| e.to_string())
    }

    /// Serves a group in memory, optionally replacing a suspended one at
    /// its existing position. The caller holds `busy` during a pending join
    /// and records configuration only after its welcome is verified.
    pub(crate) async fn serve_group(&self, g: Arc<Group>, replacing: Option<&str>) {
        crate::settings::check(&g).await;
        {
            let mut groups = self.groups.write().unwrap_or_else(|p| p.into_inner());
            let position = groups
                .iter()
                .position(|old| old.id == g.id || Some(old.id.as_str()) == replacing);
            if let Some(position) = position {
                groups[position] = g.clone();
            } else {
                groups.push(g.clone());
            }
        }
        // The helper's Listen request must already be able to find the group.
        g.ensure_remotedaemon(&self.wake).await;
        self.wake.notify_one();
    }

    /// The direct addresses this device tells peers.
    pub fn addresses(&self) -> Vec<SocketAddr> {
        self.node.advertised_addresses()
    }

    /// One round of the loops for `g`.
    pub async fn round(&self, g: &Arc<Group>) {
        // One trace per round, a span per step (12 §12.2).
        let mut round = pigeon_telemetry::span("round");
        round
            .set("group", g.id.as_str())
            .set("device", self.device.as_str())
            .set("lamport", self.tracer.clock());
        round.in_scope(self.round_inner(g, &round)).await;
    }

    async fn round_inner(&self, g: &Arc<Group>, round: &pigeon_telemetry::Span) {
        let busy = g.busy.lock().await;
        if !self.current_group(g).await {
            return;
        }
        crate::locks::clear(&g.git_dir);
        crate::settings::check(g).await;
        g.ensure_remotedaemon(&self.wake).await;
        g.reload_roster().await;
        if let Err(error) = crate::change::repair_roster_index(g).await {
            tracing::info!(group = %g.id, %error, "roster-index-waits");
        }
        crate::recovery_txt::write(g, &self.device);
        if crate::leave::round(self, g).await {
            return;
        }
        self.use_relays(&self.group_relays()).await;
        if g.role == Role::Laptop && crate::evict::unsettled(g) {
            // A step that failed, such as a git call that found a lock,
            // left its transition half done: it ends as after a crash,
            // before anything reads or writes the folder (07 §7.5).
            crate::evict::recover(g, &self.tracer).await;
        }
        let mut settled = g.role == Role::Laptop && !crate::evict::unsettled(g);
        if settled && let Err(e) = crate::capture::recover(g).await {
            tracing::warn!(group = %g.id, error = %e, "capture-recovery-failed");
            settled = false;
        }
        if settled {
            crate::asides::release(g, &self.tracer).await;
        }
        if settled && let Err(e) = crate::bridge::recover(self, g).await {
            tracing::warn!(group = %g.id, error = %e, "bridge-recovery-waits");
            return;
        }
        if settled {
            if g.shared.is_none() {
                crate::evict::free_targets(g, &self.device, &self.tracer).await;
                if let Err(e) = crate::evict::keep(g, &self.tracer).await {
                    tracing::info!(group = %g.id, error = %e, "keep-waits");
                }
            }
            let mut step = round.child("capture");
            let captured = crate::capture::capture(g, &self.device, &self.tracer).await;
            crate::status_why::note_capture(g, captured.as_ref().err());
            if let Err(e) = captured {
                tracing::warn!(group = %g.id, error = %e, "capture-failed");
                step.fail(&e.to_string());
                settled = false;
            }
            if settled && let Err(e) = crate::shared::compact(g).await {
                tracing::warn!(group = %g.id, error = %e, "shared-storage-waits");
                return;
            }
        }
        if crate::pause::now(self).is_some() {
            // Paused by the person: all but capture waits (15 §15.6).
            crate::telemetry::flush_protection(g).await;
            crate::clock_file::save(g, self.tracer.clock());
            return;
        }
        let peers = crate::custody::prepare_accept(g, &self.device).await;
        // Fetch changes objects and peer refs only. Slow or offline peers must
        // not block conditional bridge writes, capture, or portal decisions.
        drop(busy);
        let fetched = {
            let _step = round.child("fetch");
            crate::custody::fetch_ready(g, &peers).await
        };
        let _busy = g.busy.lock().await;
        // Refounding can replace this group while its network fetch runs.
        // The old round must not write through the replacement's repository.
        if !self.current_group(g).await {
            return;
        }
        // Peers that refuse this computer tell it that it was removed.
        crate::removed::note(self, g).await;
        if crate::pause::now(self).is_some() || crate::leave::leaving(g).is_some() {
            return;
        }
        let settled = settled && !crate::evict::unsettled(g);
        if settled && let Err(e) = crate::bridge::recover(self, g).await {
            tracing::warn!(group = %g.id, error = %e, "bridge-recovery-waits");
            return;
        }
        {
            let mut step = round.child("accept");
            let moved = crate::custody::accept(g, fetched, &self.tracer).await;
            step.set("moved", moved);
        }
        if settled {
            let mut step = round.child("install");
            if let Err(e) = crate::install::incoming(g, &self.device, &self.tracer).await {
                tracing::warn!(group = %g.id, error = %e, "install-failed");
                step.fail(&e.to_string());
            }
        }
        if settled && let Err(e) = crate::shared::compact(g).await {
            tracing::warn!(group = %g.id, error = %e, "shared-storage-waits");
            return;
        }
        crate::report::receive(g, &self.device, &self.dirs.update_wanted()).await;
        self.note_lost(g);
        {
            let mut step = round.child("place");
            let made = crate::custody::place(g, &self.device).await;
            step.set("copies", made as u64);
        }
        if let Err(e) = crate::shared::provider_compact(g).await {
            tracing::warn!(group = %g.id, error = %e, "shared-provider-waits");
        }
        if settled && g.shared.is_none() && !crate::evict::unsettled(g) {
            crate::thin::rethin(g, &self.device, &self.tracer).await;
        }
        {
            let _step = round.child("repair");
            crate::repair::repair(g, &self.device).await;
        }
        {
            let _step = round.child("publish");
            crate::report::publish(g, &self.device).await;
        }
        crate::telemetry::flush_protection(g).await;
        crate::clock_file::save(g, self.tracer.clock());
    }
}

impl Daemon {
    /// The groups' own relays, from their rosters (04 §4.2).
    pub fn group_relays(&self) -> Vec<String> {
        self.groups()
            .iter()
            .filter_map(|g| g.roster()?.roster.relay)
            .collect()
    }

    /// Puts the relays `wanted` in place of n0's, or n0's back when it is
    /// empty, unless the person chose the relays (09 §9.5).
    pub async fn use_relays(&self, wanted: &[String]) {
        match self.node.use_group_relays(wanted).await {
            Ok(true) => {
                let relays = self.node.relay_urls().await;
                tracing::info!(relays = ?relays, "relays-changed");
            }
            Ok(false) => {}
            Err(e) => tracing::warn!(error = %e, "relay-refused"),
        }
    }

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
        if let Some((peer, uuid)) = g.lock().bootstrap_peer.clone() {
            return (device == peer).then_some(Admitted {
                annex_uuid: Some(uuid),
                custodian: false,
            });
        }
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

    fn telemetry_device(&self, group: &str) -> Option<String> {
        let doc = Daemon::group(self, group)?.roster()?;
        let id = doc.roster.telemetry_device?;
        doc.roster
            .devices
            .get(&id)
            .filter(|d| d.role != DeviceRole::Recovery)
            .map(|_| id.0)
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

/// The git-annex the installer put in pigeon's directory, the one above
/// `pigeond`'s (`packaging/install-git-annex`, 14 §14.3): pigeon's build in
/// `git-annex/bin`, else the stock standalone build in `git-annex.linux`.
pub fn annex_dir(exe_dir: &Path) -> Option<PathBuf> {
    let prefix = exe_dir.parent()?;
    [prefix.join("git-annex/bin"), prefix.join("git-annex.linux")]
        .into_iter()
        .find(|d| d.join("git-annex").is_file())
}

/// The directories first in the children's `PATH` (14 §14.3): `<cache>/bin`,
/// with a link to the transport helper next to `pigeond` and, when the
/// git-annex they run installs none (Homebrew), a `git-remote-p2p-annex`
/// link to `git-annex`, which dispatches on its name; then [`annex_dir`].
/// Without `<cache>/bin`, the directory of `pigeond` itself.
fn child_path(dirs: &Dirs) -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let annex = exe_dir.as_deref().and_then(annex_dir);
    let bin = dirs.cache.join("bin");
    if std::fs::create_dir_all(&bin).is_err() {
        return exe_dir.into_iter().chain(annex).collect();
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
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path: Vec<PathBuf> = annex
        .iter()
        .cloned()
        .chain(std::env::split_paths(&inherited))
        .collect();
    let find = |name: &str| path.iter().map(|d| d.join(name)).find(|p| p.is_file());
    let remote = find("git-remote-p2p-annex")
        .is_none()
        .then(|| find("git-annex"))
        .flatten();
    if remote.is_some() {
        tracing::info!("git-remote-p2p-annex-linked");
    }
    link("git-remote-p2p-annex", remote);
    if let Some(dir) = &annex {
        tracing::info!(dir = %dir.display(), "git-annex-installed");
    }
    std::iter::once(bin).chain(annex).collect()
}

/// Binds the endpoint. With port 0 it binds the port it picked before, if
/// that is still free, and keeps the one it now has: peers keep this
/// device's direct addresses (09 §9.3), and a restart, a crash's included,
/// must not leave them dialling a port nobody listens on (S9).
async fn bind_node(
    dirs: &Dirs,
    secret: [u8; 32],
    bind: SocketAddr,
    network: &config::Network,
) -> Result<Node, String> {
    let reach = network.reach().map_err(|e| e.to_string())?;
    tracing::info!(reach = ?reach, "network-reach");
    let at = |bind| NetConfig {
        secret_key: secret,
        bind,
        mdns: network.mdns,
        reach: reach.clone(),
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
    let config = config::load(&dirs.config_file()).map_err(|e| e.to_string())?;
    let secret = crate::util::device_key(&dirs.device_key()).map_err(|e| e.to_string())?;
    crate::git::set_signer(secret);
    let bind: SocketAddr = config
        .network
        .bind
        .parse()
        .map_err(|e| format!("[network] bind: {e}"))?;
    let node = bind_node(&dirs, secret, bind, &config.network).await?;
    let socket = std::env::var_os("PIGEON_SOCKET")
        .filter(|s| !s.is_empty())
        .map_or_else(|| dirs.socket(), PathBuf::from);
    if config
        .bridge_storage
        .as_ref()
        .is_some_and(|s| s.source_socket == socket)
    {
        return Err("A bridge client cannot use its own socket as a storage provider".into());
    }
    if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
        return Err(format!("pigeond already runs on {}", socket.display()));
    }
    let env = GitEnv {
        path_first: child_path(&dirs),
        extra: vec![("PIGEON_SOCKET".into(), socket.clone().into_os_string())],
    };
    let device = node.device_id();
    pigeon_telemetry::init(dirs.state.join("telemetry"), "pigeond", &device);
    crate::logs::init(dirs.logs());
    let tracer = Tracer::new(dirs.traces(), &device);
    for entry in &config.groups {
        crate::join_transition::recover(entry, &dirs.config_file(), &env).await?;
    }
    let config = config::load(&dirs.config_file()).map_err(|e| e.to_string())?;
    let mut groups = Vec::new();
    for entry in &config.groups {
        match Group::open(&entry.id, &entry.path, &env).await {
            Some(mut g) => {
                crate::shared::configure(&mut g, config.bridge_storage.as_ref())?;
                if let crate::clock_file::Read::Known(c) = crate::clock_file::read(&g) {
                    tracer.at_least(c.clock);
                }
                groups.push(Arc::new(g));
            }
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
        web: OnceLock::new(),
        wake: Arc::new(tokio::sync::Notify::new()),
        tracer,
        debug: crate::debug::Permissions::default(),
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
    crate::web::serve(&daemon).await;
    tokio::spawn(serve_socket(net.clone(), listener));
    tokio::spawn(accept_loop(net.clone()));
    tokio::spawn(crate::telemetry::collect(daemon.clone()));
    tracing::info!(device = %device, socket = %socket.display(), "started");
    for (g, busy) in groups.iter().zip(held) {
        crate::evict::recover(g, &daemon.tracer).await;
        if g.role == Role::Laptop
            && !crate::evict::unsettled(g)
            && let Err(e) = crate::bridge::recover(&daemon, g).await
        {
            tracing::warn!(group = %g.id, error = %e, "bridge-recovery-waits");
        }
        drop(busy);
    }
    let rounds = daemon.clone();
    tokio::spawn(async move {
        loop {
            for g in rounds.groups() {
                rounds.round(&g).await;
            }
            crate::notices::check(&rounds).await;
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
    pigeon_telemetry::flush();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_installed_git_annex_is_found_beside_pigeond() {
        let t = tempfile::tempdir().unwrap();
        let bin = t.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        assert_eq!(annex_dir(&bin), None);
        let stock = t.path().join("git-annex.linux");
        std::fs::create_dir_all(&stock).unwrap();
        std::fs::write(stock.join("git-annex"), "").unwrap();
        assert_eq!(annex_dir(&bin), Some(stock));
        let ours = t.path().join("git-annex/bin");
        std::fs::create_dir_all(&ours).unwrap();
        std::fs::write(ours.join("git-annex"), "").unwrap();
        assert_eq!(annex_dir(&bin), Some(ours));
    }
}
