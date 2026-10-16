//! Private opt-in observations of persisted identities and selected daemon
//! sockets. One append-only stream per bridge state directory spans restarts;
//! no password, credential hash, cookie or request contents are recorded.

use std::path::Path;
#[cfg(feature = "tla-trace")]
use std::sync::Mutex;

use crate::{
    config::Config,
    store::{Client, Store},
};

#[derive(Default)]
pub struct Trace {
    #[cfg(feature = "tla-trace")]
    writer: Mutex<Option<(std::fs::File, u64)>>,
}

impl Trace {
    pub fn new(config: &Config, store: &Store) -> Self {
        #[cfg(feature = "tla-trace")]
        {
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
            if std::env::var("PIGEON_TRACE").as_deref() != Ok("1") {
                return Self::default();
            }
            let open = || -> std::io::Result<(std::fs::File, u64)> {
                let root = std::env::var_os("PIGEON_TRACE_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| config.state_dir.join("lifecycle-traces"));
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(&root)?;
                let identity = crate::store::hash(config.state_dir.as_os_str().as_encoded_bytes());
                let path = root.join(format!("lifecycle-{identity}.ndjson"));
                let clock = match std::fs::read_to_string(&path) {
                    Ok(text) => text.lines().count() as u64,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
                    Err(e) => return Err(e),
                };
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .mode(0o600)
                    .open(path)?;
                Ok((file, clock))
            };
            let trace = Self {
                writer: Mutex::new(open().ok()),
            };
            trace.emit("Start", || serde_json::json!({
                "clients": store.clients.values().map(|c| Self::client(config, c)).collect::<Vec<_>>(),
                "controls": config.users.iter().map(|(user, account)| serde_json::json!({
                    "user": user, "socket": account.control_socket,
                })).collect::<Vec<_>>(),
            }));
            trace
        }
        #[cfg(not(feature = "tla-trace"))]
        {
            let _ = (config, store);
            Self::default()
        }
    }

    #[cfg(feature = "tla-trace")]
    fn client(config: &Config, client: &Client) -> serde_json::Value {
        serde_json::json!({"id":client.id, "user":client.user,
            "device":client.device, "annex":client.annex_uuid, "revoked":client.revoked,
            "socket":config.state_dir.join("run").join(format!("{}.sock",client.id))})
    }

    pub fn register(&self, config: &Config, client: &Client) {
        self.emit("Register", || {
            #[cfg(feature = "tla-trace")]
            {
                Self::client(config, client)
            }
            #[cfg(not(feature = "tla-trace"))]
            {
                let _ = (config, client);
                serde_json::Value::Null
            }
        });
    }

    pub fn revoke(&self, id: &str) {
        self.emit("Revoke", || serde_json::json!({"id":id}));
    }

    pub fn removal(&self, client: &Client, request: &pigeon_proto::Request) {
        self.emit(
            "Remove",
            || serde_json::json!({"id":client.id,"request":request}),
        );
    }

    pub fn checked(
        &self,
        client: &Client,
        socket: &Path,
        device: &str,
        annex: &str,
        shared: bool,
        accepted: bool,
    ) {
        self.emit("Check", || {
            serde_json::json!({"id":client.id,"socket":socket,
            "device":device,"annex":annex,"shared":shared,"accepted":accepted})
        });
    }

    pub fn route(&self, client: &Client, user: &str, socket: &Path) {
        self.emit(
            "Route",
            || serde_json::json!({"id":client.id,"user":user,"socket":socket}),
        );
    }

    pub fn control(&self, user: &str, socket: &Path) {
        self.emit(
            "Control",
            || serde_json::json!({"user":user,"socket":socket}),
        );
    }

    fn emit(&self, action: &str, vars: impl FnOnce() -> serde_json::Value) {
        #[cfg(feature = "tla-trace")]
        {
            use std::io::Write;
            let Ok(mut writer) = self.writer.lock() else {
                return;
            };
            let Some((file, clock)) = writer.as_mut() else {
                return;
            };
            *clock += 1;
            let event = serde_json::json!({"spec":"BridgeLifecycle","clock":clock,
                "action":action,"vars":vars()});
            if serde_json::to_writer(&mut *file, &event).is_err()
                || file.write_all(b"\n").is_err()
                || file.flush().is_err()
            {
                *writer = None;
            }
        }
        #[cfg(not(feature = "tla-trace"))]
        {
            let _ = (action, vars);
        }
    }
}
