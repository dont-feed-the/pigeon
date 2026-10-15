//! Supervision and provisioning: each authorization starts an isolated daemon
//! process because Pigeon's signer and endpoint are per process. Identities and
//! homes persist; a client is never routed to a different peer after restart.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use pigeon_proto::bridge::{Operation, Reply};
use pigeon_proto::{Arg, Request, Response};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::{
    config::Config,
    rpc,
    store::{self, Client},
};

pub struct Peers {
    config: Config,
    children: Mutex<BTreeMap<String, Child>>,
}

impl Peers {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            children: Mutex::default(),
        }
    }
    pub fn home(&self, id: &str) -> PathBuf {
        self.config.state_dir.join("peers").join(id)
    }
    pub fn socket(&self, id: &str) -> PathBuf {
        self.config.state_dir.join("run").join(format!("{id}.sock"))
    }

    fn configure(&self, id: &str, user: &str) -> Result<(), String> {
        let account = self.config.users.get(user).ok_or("Unknown bridge user")?;
        let home = self.home(id);
        std::fs::create_dir_all(home.join("config")).map_err(|e| e.to_string())?;
        let file = home.join("config/config.toml");
        let old = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.to_string()),
        };
        let mut doc: toml_edit::DocumentMut = old
            .parse()
            .map_err(|e: toml_edit::TomlError| e.to_string())?;
        if old.is_empty() {
            doc["format"] = toml_edit::value(1);
            doc["notifications"] = toml_edit::value(false);
            doc["network"] = toml_edit::table();
            let local = self.config.peer_network == "off";
            doc["network"]["bind"] =
                toml_edit::value(if local { "127.0.0.1:0" } else { "0.0.0.0:0" });
            doc["network"]["mdns"] = toml_edit::value(!local);
            doc["network"]["relays"] = toml_edit::value(if local { "off" } else { "n0" });
            doc["loops"] = toml_edit::table();
            doc["loops"]["interval_seconds"] = toml_edit::value(2);
        }
        doc["bridge_storage"] = toml_edit::table();
        doc["bridge_storage"]["store"] =
            toml_edit::value(self.config.content_dir().display().to_string());
        doc["bridge_storage"]["source_socket"] = toml_edit::value(
            account
                .storage_socket
                .as_ref()
                .unwrap_or(&account.control_socket)
                .display()
                .to_string(),
        );
        let updated = doc.to_string();
        if updated != old {
            pigeon_sys::fs::write_atomic(&file, updated.as_bytes()).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub async fn ensure(&self, id: &str, user: &str) -> Result<PathBuf, String> {
        let socket = self.socket(id);
        self.configure(id, user)?;
        let mut children = self.children.lock().await;
        if tokio::net::UnixStream::connect(&socket).await.is_ok() {
            return Ok(socket);
        }
        if let Some(child) = children.get_mut(id)
            && child.try_wait().map_err(|e| e.to_string())?.is_none()
        {
            return Err("The client peer is starting or unavailable".into());
        }
        let home = self.home(id);
        std::fs::create_dir_all(home.join("config")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(socket.parent().ok_or("Missing socket directory")?)
            .map_err(|e| e.to_string())?;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(home.join("daemon.log"))
            .map_err(|e| e.to_string())?;
        let mut command = Command::new(&self.config.pigeond);
        command
            .arg("--home")
            .arg(&home)
            .env("PIGEON_SOCKET", &socket)
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log)
            .kill_on_drop(true)
            .process_group(0);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        for _ in 0..150 {
            if tokio::net::UnixStream::connect(&socket).await.is_ok() {
                children.insert(id.to_owned(), child);
                return Ok(socket);
            }
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err("The client peer could not start; check its daemon log".into());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err("The client peer did not start in time".into())
    }

    pub async fn checked(&self, client: &Client) -> Result<PathBuf, String> {
        let socket = self.ensure(&client.id, &client.user).await?;
        match rpc::bridge(&socket, Operation::Info).await? {
            Reply::Info {
                device,
                annex_uuid,
                shared: true,
                ..
            } if device == client.device && annex_uuid == client.annex_uuid => Ok(socket),
            _ => Err("The client's saved Pigeon identity does not match its daemon".into()),
        }
    }

    pub async fn provision(
        &self,
        user: &str,
        name: String,
        password: &str,
    ) -> Result<Client, String> {
        let account = self.config.users.get(user).ok_or("Unknown bridge user")?;
        let (person, group) = match rpc::bridge(&account.control_socket, Operation::Info).await? {
            Reply::Info { person, group, .. } => (person, group),
            _ => return Err("The user's control peer must belong to a Pigeon group".into()),
        };
        // Cache-only operation still works when the provider is temporarily
        // offline. An online custodian also reuses the pool for future receives.
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            rpc::bridge(
                account
                    .storage_socket
                    .as_ref()
                    .unwrap_or(&account.control_socket),
                Operation::RegisterPool {
                    group: group.clone(),
                    store: self.config.content_dir().display().to_string(),
                },
            ),
        )
        .await;
        let invitation = rpc::call(
            &account.control_socket,
            Request::Invite {
                person,
                admin: false,
                custodian: false,
                name: None,
            },
        )
        .await?;
        let code = match invitation {
            Response::Said { message } => message.args.get("code").and_then(|a| match a {
                Arg::Text(s) => Some(s.clone()),
                _ => None,
            }),
            _ => None,
        }
        .ok_or("The control peer could not invite another device of this person")?;
        let id = store::random()?[..32].to_owned();
        let socket = self.ensure(&id, user).await?;
        let joined = rpc::call(
            &socket,
            Request::Join {
                invitation: code,
                folder: Some(self.home(&id).join("files").display().to_string()),
                keep: true,
                name: Some(name.clone()),
            },
        )
        .await?;
        if let Response::Problem(p) = joined {
            self.stop(&id).await;
            return Err(format!("The client peer could not join: {}", p.code));
        }
        match rpc::bridge(&socket, Operation::Info).await? {
            Reply::Info {
                device,
                annex_uuid,
                group: joined_group,
                shared: true,
                ..
            } if group == joined_group => Ok(Client {
                id,
                user: user.to_owned(),
                name,
                token_hash: store::hash(password.as_bytes()),
                device,
                annex_uuid,
                revoked: false,
                metadata: BTreeMap::new(),
            }),
            _ => {
                self.stop(&id).await;
                Err("The client joined an unexpected group".into())
            }
        }
    }

    pub async fn stop(&self, id: &str) {
        if let Some(mut child) = self.children.lock().await.remove(id) {
            if let Some(pid) = child
                .id()
                .and_then(|p| rustix::process::Pid::from_raw(p as i32))
            {
                let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::TERM);
            }
            if tokio::time::timeout(Duration::from_secs(3), child.wait())
                .await
                .is_err()
            {
                let _ = child.start_kill();
            }
        }
    }

    pub async fn shutdown(&self) {
        let ids: Vec<_> = self.children.lock().await.keys().cloned().collect();
        for id in ids {
            self.stop(&id).await;
        }
    }

    /// Restores all active client processes and exchanges direct hints only
    /// within the same group. Hints never confer roster membership.
    pub async fn mesh(&self, clients: Vec<Client>) -> usize {
        let mut sockets = Vec::new();
        let mut failures = 0;
        for client in clients {
            match self.ensure(&client.id, &client.user).await {
                Ok(socket) => sockets.push((socket, Some((client.device, client.annex_uuid)))),
                Err(_) => failures += 1,
            }
        }
        for user in self.config.users.values() {
            sockets.push((user.control_socket.clone(), None));
        }
        let mut groups: BTreeMap<String, Vec<(PathBuf, String, Vec<String>)>> = BTreeMap::new();
        for (socket, expected) in sockets {
            match rpc::bridge(&socket, Operation::Info).await {
                Ok(Reply::Info {
                    group,
                    device,
                    annex_uuid,
                    addresses,
                    ..
                }) if expected
                    .as_ref()
                    .is_none_or(|(d, a)| d == &device && a == &annex_uuid) =>
                {
                    groups
                        .entry(group)
                        .or_default()
                        .push((socket, device, addresses));
                }
                _ => failures += 1,
            }
        }
        for peers in groups.into_values() {
            let hints: BTreeMap<_, _> = peers
                .iter()
                .map(|(_, device, addresses)| (device.clone(), addresses.clone()))
                .collect();
            for (socket, _, _) in peers {
                if rpc::bridge(
                    &socket,
                    Operation::LearnPeers {
                        peers: hints.clone(),
                    },
                )
                .await
                .is_err()
                {
                    failures += 1;
                }
            }
        }
        failures
    }
}
