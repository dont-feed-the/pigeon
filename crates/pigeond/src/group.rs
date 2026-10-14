//! One group on this device: its repository, its roster, what the loops
//! know of its peers, and the settings check (03 §3.4, 08 §8.1).
//!
//! What the device knows of its peers' addresses is kept in
//! `<git_dir>/pigeon/peers.json`; when each was last heard of is kept in
//! memory only (11 §11.4). The token git-annex accepts on this repository
//! (09 §9.3) is kept in `<git_dir>/pigeon/p2p-token`, mode 0600.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pigeon_core::filter::Role;
use pigeon_git::GitEnv;
use pigeon_sys::paths::RepoFiles;

use crate::git::Git;
use crate::roster_file::{self, RosterDoc};

/// What the loops know of a group, behind a lock never held across a wait.
#[derive(Debug, Default)]
pub struct Known {
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
}

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
        };
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
        let parsed = blob.and_then(|b| roster_file::parse(&String::from_utf8_lossy(&b)).ok());
        match parsed {
            Some(doc) => {
                let mut k = self.lock();
                k.roster = Some(doc);
                k.roster_commit = Some(main);
            }
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

    /// Starts remotedaemon if it is not running (09 §9.3).
    pub async fn ensure_remotedaemon(&self) {
        let mut daemon = self.daemon.lock().await;
        if let Some(child) = daemon.as_mut()
            && matches!(child.try_wait(), Ok(None))
        {
            return;
        }
        match self
            .git
            .repo
            .spawn(&["annex", "remotedaemon", "--foreground"])
        {
            Ok(child) => {
                tracing::info!(group = %self.id, "remotedaemon-started");
                *daemon = Some(child);
            }
            Err(e) => tracing::warn!(group = %self.id, error = %e, "remotedaemon-failed"),
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
