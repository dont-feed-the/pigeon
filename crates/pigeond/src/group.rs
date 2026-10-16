//! One group on this device: its repository, its roster, what the loops
//! know of its peers, and the settings check (03 §3.4, 08 §8.1).
//!
//! What the device knows of its peers' addresses is kept in
//! `<git_dir>/pigeon/peers.json`; when each was last heard of is kept in
//! memory only (11 §11.4). The token git-annex accepts on this repository
//! (09 §9.3) is kept in `<git_dir>/pigeon/p2p-token`, mode 0600.
//!
//! The repository's remotedaemon carries peers' change notices (08 §8.1):
//! its `CONNECTED` and `DONESYNCING` messages start a round at once, and
//! one whose fetch began (`SYNCING`) and never ended is restarted (08 §8.2).

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::Notify;

use pigeon_core::filter::Role;
use pigeon_git::GitEnv;
use pigeon_sys::paths::RepoFiles;

use crate::git::Git;
use crate::roster_file::{self, RosterDoc};

/// What the loops know of a group, behind a lock never held across a wait.
#[derive(Debug, Default)]
pub struct Known {
    /// A temporary, non-listening metadata fetch admits only its inviter.
    pub bootstrap_peer: Option<(String, String)>,
    /// The roster of `main`.
    pub roster: Option<RosterDoc>,
    /// The commit that roster was read from.
    pub roster_commit: Option<String>,
    /// When each peer was last heard of, by device ID.
    pub heard: BTreeMap<String, i64>,
    /// Direct addresses by device ID.
    pub addresses: BTreeMap<String, Vec<SocketAddr>>,
    /// This repository's annex UUID.
    pub annex_uuid: Option<String>,
    /// The token git-annex accepts here.
    pub token: Option<String>,
    /// Paths capture leaves out, with why (07 §7.9).
    pub not_shared: Vec<(String, String)>,
    /// Peers whose last fetch failed: place sends them nothing this round.
    pub unreachable: BTreeSet<String>,
    /// Custodians that answer but serve this group no more: their
    /// repository is gone, so they count for nothing (06 §6.3, S7).
    pub lost: BTreeSet<String>,
    /// What admission deferred in the last round, on a custodian (08 §8.9).
    pub shortfall: Option<crate::admission::Shortfall>,
    /// The first commit of each peer's `main` that accept refused, and its
    /// problem code (04 §4.4).
    pub refused: BTreeMap<String, (String, &'static str)>,
    /// The paths this laptop's person changed while `main` gave them
    /// another version that install has not written (07 §7.6).
    pub diverging: BTreeSet<String>,
    /// The latest status report of each other device that counted (06 §6.3).
    pub reports: pigeon_core::report::Latest,
    /// What those reports say, by device ID.
    pub report_files: BTreeMap<String, crate::report_file::Status>,
    /// This device's last publication: its gist, and when.
    pub published: Option<(String, i64)>,
    /// The bytes place left to send (a laptop) or to receive (a custodian)
    /// in its last round.
    pub queue_bytes: u64,
    /// Every commit judged so far and the catalogue of each (05 §5.2):
    /// accept's memory, rebuilt from the genesis after a restart.
    pub engine: pigeon_core::catalogue::Engine<pigeon_core::authority::Entry>,
    /// The roster changes of the accepted commits, for the fold (`fold`).
    pub history: Option<pigeon_core::roster::History<String>>,
    /// Immutable archive tips checked in isolated history judges.
    pub verified_archives: BTreeSet<String>,
    /// The verdict on each judged commit that was not accepted (04 §4.4).
    pub verdicts: BTreeMap<String, pigeon_core::authority::Verdict>,
    /// The paths install lately wrote over another version: when, in
    /// nanoseconds since the epoch, and the index entry they had. A save
    /// within [`crate::capture::STALE_SAVE_NS`] of it is concurrent with
    /// the installed version (ADR 0071); kept in memory only.
    pub installed: BTreeMap<String, (i64, pigeon_core::authority::Entry)>,
}

/// How long remotedaemon's fetch of a notice may last before it is
/// restarted (08 §8.2, E12).
pub const STUCK_SYNC_SECONDS: i64 = 90;

/// One group of this device.
#[derive(Debug)]
pub struct Group {
    /// The group ID: its genesis commit.
    pub id: String,
    /// This device's role in it.
    pub role: Role,
    /// The git directory.
    pub git_dir: PathBuf,
    /// The folder, on a laptop.
    pub worktree: Option<PathBuf>,
    /// git in the repository.
    pub git: Git,
    /// pigeon's files in the repository.
    pub files: RepoFiles,
    /// What the loops know.
    pub known: Mutex<Known>,
    /// Held by whatever changes the repository: a round of the loops, or a
    /// request.
    pub busy: tokio::sync::Mutex<()>,
    /// The repository's remotedaemon.
    pub daemon: tokio::sync::Mutex<Option<tokio::process::Child>>,
    /// The fetches remotedaemon began and has not ended, by remote URL,
    /// with when each began.
    pub syncing: Arc<Mutex<BTreeMap<String, i64>>>,
    /// The peers remotedaemon was last told of; empty when it starts.
    pub daemon_peers: Mutex<BTreeSet<String>>,
}

/// What one line of remotedaemon's output asks of `pigeond`: whether to
/// start a round, and the fetch it began or ended.
fn on_message(line: &str, syncing: &Mutex<BTreeMap<String, i64>>) -> bool {
    let mut words = line.split_whitespace();
    let (Some(verb), url) = (words.next(), words.next().unwrap_or_default()) else {
        return false;
    };
    let mut s = syncing.lock().unwrap_or_else(|p| p.into_inner());
    match verb {
        "SYNCING" => {
            s.insert(url.to_owned(), crate::util::now());
            false
        }
        "DONESYNCING" => {
            s.remove(url);
            true
        }
        "CONNECTED" => true,
        "DISCONNECTED" => {
            s.remove(url);
            false
        }
        _ => false,
    }
}

/// How a path holds a group's repository.
pub fn locate(path: &Path) -> Option<(Role, PathBuf, Option<PathBuf>)> {
    let path = std::fs::canonicalize(path).ok()?;
    if path.join(".git").is_dir() {
        Some((Role::Laptop, path.join(".git"), Some(path)))
    } else if path.join("HEAD").is_file() && path.join("objects").is_dir() {
        Some((Role::Custodian, path, None))
    } else {
        None
    }
}

impl Group {
    /// The group `id` at `path`, as this device left it.
    pub async fn open(id: &str, path: &Path, env: &GitEnv) -> Option<Group> {
        let (role, git_dir, worktree) = locate(path)?;
        let dir = worktree.clone().unwrap_or_else(|| git_dir.clone());
        let files = RepoFiles::new(&git_dir);
        let _ = files.create();
        let g = Group {
            id: id.to_owned(),
            role,
            git_dir,
            worktree,
            git: Git::new(dir, env.clone()),
            files,
            known: Mutex::new(Known::default()),
            busy: tokio::sync::Mutex::new(()),
            daemon: tokio::sync::Mutex::new(None),
            syncing: Arc::new(Mutex::new(BTreeMap::new())),
            daemon_peers: Mutex::new(BTreeSet::new()),
        };
        if let crate::clock_file::Read::Known(c) = crate::clock_file::read(&g) {
            g.lock().heard = c.heard;
        }
        let uuid = g.git.config("annex.uuid").await;
        let token = std::fs::read_to_string(g.token_file())
            .ok()
            .map(|t| t.trim().to_owned());
        let addresses = g.load_peers();
        {
            let mut k = g.lock();
            k.annex_uuid = uuid;
            k.token = token;
            k.addresses = addresses;
        }
        g.reload_roster().await;
        Some(g)
    }

    /// An isolated in-memory judge for archived history. Judging another
    /// genesis must not replace the live group's roster fold or catalogue.
    pub(crate) fn history_judge(&self, genesis: &str) -> Group {
        Group {
            id: genesis.to_owned(),
            role: self.role,
            git_dir: self.git_dir.clone(),
            worktree: self.worktree.clone(),
            git: self.git.clone(),
            files: self.files.clone(),
            known: Mutex::new(Known::default()),
            busy: tokio::sync::Mutex::new(()),
            daemon: tokio::sync::Mutex::new(None),
            syncing: Arc::new(Mutex::new(BTreeMap::new())),
            daemon_peers: Mutex::new(BTreeSet::new()),
        }
    }

    /// The lock of what the loops know.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, Known> {
        self.known
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// `<git_dir>/pigeon/`.
    pub fn root(&self) -> PathBuf {
        self.git_dir.join("pigeon")
    }

    fn token_file(&self) -> PathBuf {
        self.root().join("p2p-token")
    }

    fn peers_file(&self) -> PathBuf {
        self.root().join("peers.json")
    }

    fn load_peers(&self) -> BTreeMap<String, Vec<SocketAddr>> {
        let Ok(text) = std::fs::read_to_string(self.peers_file()) else {
            return BTreeMap::new();
        };
        let raw: BTreeMap<String, Vec<String>> = serde_json::from_str(&text).unwrap_or_default();
        raw.into_iter()
            .map(|(d, a)| (d, a.iter().filter_map(|s| s.parse().ok()).collect()))
            .collect()
    }

    /// Records the direct addresses of `device` and writes `peers.json`.
    pub fn set_addresses(&self, device: &str, addresses: Vec<SocketAddr>) {
        let raw: BTreeMap<String, Vec<String>> = {
            let mut k = self.lock();
            k.addresses.insert(device.to_owned(), addresses);
            k.addresses
                .iter()
                .map(|(d, a)| (d.clone(), a.iter().map(ToString::to_string).collect()))
                .collect()
        };
        if let Ok(text) = serde_json::to_vec_pretty(&raw) {
            let _ = pigeon_sys::fs::write_atomic(&self.peers_file(), &text);
        }
    }

    /// The direct addresses known of the group's devices but `except`, as a
    /// welcome gives them to a new device.
    pub fn known_addresses(&self, except: &str) -> BTreeMap<String, Vec<String>> {
        self.lock()
            .addresses
            .iter()
            .filter(|(d, a)| d.as_str() != except && !a.is_empty())
            .map(|(d, a)| (d.clone(), a.iter().map(ToString::to_string).collect()))
            .collect()
    }

    /// Records the addresses a welcome gave of the group's other devices,
    /// keeping those this device already has.
    pub fn learn_peers(&self, peers: &BTreeMap<String, Vec<String>>) {
        for (device, addresses) in peers {
            if self.lock().addresses.contains_key(device) {
                continue;
            }
            let parsed: Vec<SocketAddr> = addresses.iter().filter_map(|a| a.parse().ok()).collect();
            if !parsed.is_empty() {
                self.set_addresses(device, parsed);
            }
        }
    }

    /// Records the token git-annex accepts here.
    pub fn set_token(&self, token: &str) {
        let _ = crate::util::write_private(&self.token_file(), format!("{token}\n").as_bytes());
        self.lock().token = Some(token.to_owned());
    }

    /// Reads the roster of `main` again, if `main` moved.
    pub async fn reload_roster(&self) {
        let Some(main) = self.git.rev("refs/heads/main").await else {
            return;
        };
        if self.lock().roster_commit.as_deref() == Some(main.as_str()) {
            return;
        }
        let blob = self.git.blob(&format!("{main}:.pigeon/roster.toml")).await;
        let parsed = blob.map(|b| roster_file::parse(&String::from_utf8_lossy(&b)));
        match parsed {
            Some(Ok(doc)) => {
                let mut k = self.lock();
                k.roster = Some(doc);
                k.roster_commit = Some(main);
            }
            Some(Err(why)) => tracing::warn!(group = %self.id, %why, "roster-unreadable"),
            None => tracing::warn!(group = %self.id, "roster-unreadable"),
        }
    }

    /// The roster, if one was read.
    pub fn roster(&self) -> Option<RosterDoc> {
        self.lock().roster.clone()
    }

    /// The repository, as the network serves it.
    pub fn repository(&self) -> crate::net::Repository {
        crate::net::Repository {
            group: self.id.clone(),
            git_dir: self.git_dir.clone(),
            worktree: self.worktree.clone(),
            role: self.role,
            auth_token: self.lock().token.clone(),
            checking: false,
        }
    }

    /// Starts remotedaemon if it is not running (09 §9.3), and restarts it
    /// if a fetch it began has lasted [`STUCK_SYNC_SECONDS`]. Its messages
    /// wake `wake`.
    pub async fn ensure_remotedaemon(&self, wake: &Arc<Notify>) {
        let stuck = {
            let s = self.syncing.lock().unwrap_or_else(|p| p.into_inner());
            let now = crate::util::now();
            s.values().any(|since| now - since > STUCK_SYNC_SECONDS)
        };
        if stuck {
            tracing::warn!(group = %self.id, "remotedaemon-stuck");
            self.stop_remotedaemon().await;
        }
        let mut daemon = self.daemon.lock().await;
        if let Some(child) = daemon.as_mut()
            && matches!(child.try_wait(), Ok(None))
        {
            return;
        }
        self.syncing
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        self.daemon_peers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        match self
            .git
            .repo
            .spawn(&["annex", "remotedaemon", "--foreground"])
        {
            Ok(mut child) => {
                if let Some(out) = child.stdout.take() {
                    let (wake, syncing, id) = (wake.clone(), self.syncing.clone(), self.id.clone());
                    tokio::spawn(async move {
                        let mut lines = tokio::io::BufReader::new(out).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            tracing::debug!(group = %id, message = %line, "remotedaemon");
                            if on_message(&line, &syncing) {
                                wake.notify_one();
                            }
                        }
                    });
                }
                tracing::info!(group = %self.id, "remotedaemon-started");
                *daemon = Some(child);
            }
            Err(e) => tracing::warn!(group = %self.id, error = %e, "remotedaemon-failed"),
        }
    }

    /// Tells remotedaemon to read the remotes again when `peers`, the
    /// remotes of this round, differ from those it was last told of.
    pub async fn reload_remotedaemon(&self, peers: BTreeSet<String>) {
        {
            let mut seen = self.daemon_peers.lock().unwrap_or_else(|p| p.into_inner());
            if *seen == peers {
                return;
            }
            *seen = peers;
        }
        let mut daemon = self.daemon.lock().await;
        if let Some(stdin) = daemon.as_mut().and_then(|c| c.stdin.as_mut()) {
            let _ = stdin.write_all(b"RELOAD\n").await;
            let _ = stdin.flush().await;
        }
    }

    /// Stops remotedaemon and what it started: it leads its own process
    /// group, so a fetch it left hanging stops too.
    pub async fn stop_remotedaemon(&self) {
        if let Some(mut child) = self.daemon.lock().await.take() {
            if let Some(pid) = child.id() {
                use pigeon_sys::signal::{Action, send_group};
                let _ = send_group(pid, Action::Kill);
            }
            let _ = child.kill().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[test]
    fn a_notice_wakes_and_a_begun_fetch_is_remembered_until_it_ends() {
        let s = Mutex::new(BTreeMap::new());
        let url = "p2p-annex::iroh:abc";
        assert!(!super::on_message(&format!("SYNCING {url}"), &s));
        assert!(s.lock().unwrap().contains_key(url));
        assert!(super::on_message(&format!("DONESYNCING {url} 1"), &s));
        assert!(s.lock().unwrap().is_empty());
        assert!(super::on_message(&format!("CONNECTED {url}"), &s));
        assert!(!super::on_message("WARNING x y", &s));
        assert!(!super::on_message("", &s));
    }
}
