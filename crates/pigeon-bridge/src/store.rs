//! Private persistent client identities and DAV metadata. App passwords are
//! stored only as SHA-256 hashes of random 256-bit credentials. Writes replace
//! and fsync the state file before a client is told its credentials work.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Client {
    pub id: String,
    pub user: String,
    pub name: String,
    pub token_hash: String,
    pub device: String,
    pub annex_uuid: String,
    #[serde(default)]
    pub revoked: bool,
    #[serde(default)]
    pub metadata: BTreeMap<String, Metadata>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub id: u64,
    pub etag: String,
    pub modified: u64,
    pub directory: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Store {
    pub format: u32,
    pub next_file_id: u64,
    pub clients: BTreeMap<String, Client>,
}

pub fn random() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Store {
    pub fn load(root: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let store: Self = match std::fs::read(root.join("clients.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self {
                format: 1,
                next_file_id: 1,
                ..Self::default()
            },
            Err(e) => return Err(e.to_string()),
        };
        if store.format != 1 {
            return Err("Unsupported bridge state format".into());
        }
        for (id, client) in &store.clients {
            if id != &client.id
                || id.len() != 32
                || !id.bytes().all(|b| b.is_ascii_hexdigit())
                || client.token_hash.len() != 64
                || client.device.len() != 64
                || client.annex_uuid.is_empty()
            {
                return Err("Invalid persisted client identity".into());
            }
        }
        Ok(store)
    }

    pub fn save(&self, root: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        pigeon_sys::fs::write_atomic(&root.join("clients.json"), &bytes).map_err(|e| e.to_string())
    }
}
