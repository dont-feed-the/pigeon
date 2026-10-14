//! `config.toml` (11 §11.2): read with serde, changed in place with
//! `toml_edit`, so that comments, order and keys this build does not know
//! survive (13 §13.12).
//!
//! The slice reads `format`, `language`, `[[groups]]` and two tables the
//! spec did not have: `[network]` (where the endpoint binds, whether mDNS
//! runs) and `[loops]` (how often the loops run). A group is a laptop's if
//! its path holds a `.git` directory, a custodian's if it is a bare
//! repository.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The settings `pigeond` reads.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    /// The format, 1.
    #[serde(default)]
    pub format: u32,
    /// `auto`, `en`, …
    #[serde(default)]
    pub language: Option<String>,
    /// The endpoint.
    #[serde(default)]
    pub network: Network,
    /// The loops.
    #[serde(default)]
    pub loops: Loops,
    /// The groups of this device.
    #[serde(default)]
    pub groups: Vec<GroupEntry>,
}

/// `[network]`.
#[derive(Clone, Debug, Deserialize)]
pub struct Network {
    /// Where the QUIC socket binds.
    #[serde(default = "default_bind")]
    pub bind: String,
    /// Find peers with mDNS.
    #[serde(default = "yes")]
    pub mdns: bool,
}

impl Default for Network {
    fn default() -> Self {
        Network {
            bind: default_bind(),
            mdns: true,
        }
    }
}

/// `[loops]`.
#[derive(Clone, Debug, Deserialize)]
pub struct Loops {
    /// Seconds between two rounds of the loops.
    #[serde(default = "default_interval")]
    pub interval_seconds: u64,
}

impl Default for Loops {
    fn default() -> Self {
        Loops {
            interval_seconds: default_interval(),
        }
    }
}

fn default_bind() -> String {
    "0.0.0.0:0".to_owned()
}

fn default_interval() -> u64 {
    30
}

fn yes() -> bool {
    true
}

/// One `[[groups]]` entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GroupEntry {
    /// The group ID: its genesis commit.
    pub id: String,
    /// The folder, or a custodian's bare repository.
    pub path: PathBuf,
}

/// Errors of the configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The file could not be read or written.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// The file is not valid TOML, or a value has the wrong type.
    #[error("config.toml: {0}")]
    Invalid(String),
}

/// Reads `path`; a missing file is the defaults.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml_edit::de::from_str(&text).map_err(|e| ConfigError::Invalid(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config {
            format: 1,
            ..Config::default()
        }),
        Err(e) => Err(e.into()),
    }
}

/// Adds a `[[groups]]` entry to the file at `path`, keeping the rest.
pub fn add_group(path: &Path, entry: &GroupEntry) -> Result<(), ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "format = 1\n".to_owned(),
        Err(e) => return Err(e.into()),
    };
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| ConfigError::Invalid(e.to_string()))?;
    let mut table = toml_edit::Table::new();
    table.insert("id", toml_edit::value(entry.id.clone()));
    table.insert(
        "path",
        toml_edit::value(entry.path.to_string_lossy().into_owned()),
    );
    match doc
        .get_mut("groups")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
    {
        Some(groups) => groups.push(table),
        None => {
            let mut groups = toml_edit::ArrayOfTables::new();
            groups.push(table);
            doc.insert("groups", toml_edit::Item::ArrayOfTables(groups));
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    pigeon_sys::fs::write_atomic(path, doc.to_string().as_bytes())
        .map_err(|e| ConfigError::Io(std::io::Error::other(e.to_string())))
}
