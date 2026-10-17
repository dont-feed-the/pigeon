//! `config.toml` (11 §11.2): read with serde, changed in place with
//! `toml_edit`, so that comments, order and keys this build does not know
//! survive (13 §13.12).
//!
//! The slice reads `format`, `language`, `notifications`, `[[groups]]`
//! and two tables the spec did not have: `[network]` (where the endpoint
//! binds, whether mDNS runs) and `[loops]` (how often the loops run). A group is a laptop's if
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
    /// Desktop notifications (ADR 0029); on unless `false`.
    #[serde(default)]
    pub notifications: Option<bool>,
    /// The endpoint.
    #[serde(default)]
    pub network: Network,
    /// The loops.
    #[serde(default)]
    pub loops: Loops,
    /// Optional backend on this device; no default endpoint (ADR 0025).
    #[serde(default)]
    pub telemetry: Telemetry,
    /// Local maintenance/resource choices (15 §15.6).
    #[serde(default)]
    pub resources: Resources,
    /// The groups of this device.
    #[serde(default)]
    pub groups: Vec<GroupEntry>,
    /// Shared content and sparse client overlays, only for managed bridge peers.
    #[serde(default)]
    pub bridge_storage: Option<BridgeStorage>,
}

/// A managed bridge peer's immutable content pool and existing storage node.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeStorage {
    /// Shared pool, on the same filesystem as the client's repository.
    pub store: PathBuf,
    /// A laptop or custodian daemon serving this group on a private local socket.
    pub source_socket: PathBuf,
}

pub use crate::resource_config::Resources;

/// `[telemetry]`: configured locally, never learned from another member.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Telemetry {
    /// OTLP HTTP base URL, such as `http://127.0.0.1:4318`.
    #[serde(default)]
    pub endpoint: Option<String>,
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
    /// `n0` (n0's relays and address lookup), `off` (direct addresses and
    /// mDNS only) or a relay's URL (09 §9.5); `PIGEON_RELAYS` overrides it.
    #[serde(default = "default_relays")]
    pub relays: String,
}

impl Default for Network {
    fn default() -> Self {
        Network {
            bind: default_bind(),
            mdns: true,
            relays: default_relays(),
        }
    }
}

impl Network {
    /// How the endpoint reaches peers: `relays`, or `PIGEON_RELAYS` when
    /// it is set.
    pub fn reach(&self) -> Result<crate::net::Reach, ConfigError> {
        let env = std::env::var("PIGEON_RELAYS")
            .ok()
            .filter(|v| !v.is_empty());
        reach_of(env.as_deref().unwrap_or(&self.relays))
    }
}

/// The reach a `relays` value names.
pub fn reach_of(value: &str) -> Result<crate::net::Reach, ConfigError> {
    use crate::net::Reach;
    match value.trim() {
        "n0" => Ok(Reach::N0),
        "off" => Ok(Reach::Local),
        url if url.starts_with("https://") || url.starts_with("http://") => {
            Ok(Reach::Relays(vec![url.to_owned()]))
        }
        other => Err(ConfigError::Invalid(format!(
            "[network] relays: {other:?} is not n0, off or a relay URL"
        ))),
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

fn default_relays() -> String {
    "n0".to_owned()
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
    /// Local kept-folder download order, never custody or retention authority.
    #[serde(default)]
    pub priority: crate::download_queue::Priorities,
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
        Ok(text) => {
            let config: Config =
                toml_edit::de::from_str(&text).map_err(|e| ConfigError::Invalid(e.to_string()))?;
            config.resources.effective().map_err(ConfigError::Invalid)?;
            for group in &config.groups {
                crate::download_queue::validate(&group.priority).map_err(ConfigError::Invalid)?;
            }
            Ok(config)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config {
            format: 1,
            ..Config::default()
        }),
        Err(e) => Err(e.into()),
    }
}

/// The document at `path`; a missing file is `format = 1`.
pub(crate) fn read_doc(path: &Path) -> Result<toml_edit::DocumentMut, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "format = 1\n".to_owned(),
        Err(e) => return Err(e.into()),
    };
    text.parse()
        .map_err(|e: toml_edit::TomlError| ConfigError::Invalid(e.to_string()))
}

/// Writes `doc` to `path` at once.
fn write_doc(path: &Path, doc: &toml_edit::DocumentMut) -> Result<(), ConfigError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    pigeon_sys::fs::write_atomic(path, doc.to_string().as_bytes())
        .map_err(|e| ConfigError::Io(std::io::Error::other(e.to_string())))
}

/// Sets `key` of `[resources]` in the file at `path` to `value`, or removes
/// it for `None`, keeping the rest (15 §15.6).
pub fn set_resource(
    path: &Path,
    key: &str,
    value: Option<toml_edit::Value>,
) -> Result<(), ConfigError> {
    let mut doc = read_doc(path)?;
    let table = doc
        .entry("resources")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()
        .ok_or_else(|| ConfigError::Invalid("[resources] is not a table".to_owned()))?;
    match value {
        Some(v) => {
            table.insert(key, toml_edit::Item::Value(v));
        }
        None => {
            table.remove(key);
        }
    }
    write_doc(path, &doc)
}

/// Sets the top-level `key` of the file at `path` to `value`, keeping the
/// rest.
pub fn set_value(path: &Path, key: &str, value: toml_edit::Value) -> Result<(), ConfigError> {
    let mut doc = read_doc(path)?;
    match doc.get_mut(key).and_then(toml_edit::Item::as_value_mut) {
        // In place, so that the comments around it stay.
        Some(old) => {
            let decor = old.decor().clone();
            *old = value;
            *old.decor_mut() = decor;
        }
        None => {
            doc.insert(key, toml_edit::Item::Value(value));
        }
    }
    write_doc(path, &doc)
}

/// Adds a `[[groups]]` entry to the file at `path`, keeping the rest.
pub fn add_group(path: &Path, entry: &GroupEntry) -> Result<(), ConfigError> {
    crate::download_queue::validate(&entry.priority).map_err(ConfigError::Invalid)?;
    let mut doc = read_doc(path)?;
    let mut table = toml_edit::Table::new();
    table.insert("id", toml_edit::value(entry.id.clone()));
    table.insert(
        "path",
        toml_edit::value(entry.path.to_string_lossy().into_owned()),
    );
    if !entry.priority.is_empty() {
        let mut priorities = toml_edit::Table::new();
        for (folder, priority) in &entry.priority {
            let value = match priority {
                crate::download_queue::Priority::High => "high",
                crate::download_queue::Priority::Normal => "normal",
                crate::download_queue::Priority::Low => "low",
            };
            priorities.insert(folder, toml_edit::value(value));
        }
        table.insert("priority", toml_edit::Item::Table(priorities));
    }
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
    write_doc(path, &doc)
}

/// Atomically replaces an in-place join's old entry, preserving position
/// and other settings. Repeating it removes duplicate old/new entries.
pub fn replace_group(path: &Path, old: &str, entry: &GroupEntry) -> Result<(), ConfigError> {
    let mut doc = read_doc(path)?;
    let replacement_path =
        std::fs::canonicalize(&entry.path).unwrap_or_else(|_| entry.path.clone());
    let mut replacement = toml_edit::Table::new();
    replacement.insert("id", toml_edit::value(entry.id.clone()));
    replacement.insert(
        "path",
        toml_edit::value(entry.path.to_string_lossy().into_owned()),
    );
    let mut changed = toml_edit::ArrayOfTables::new();
    let mut inserted = false;
    if let Some(groups) = doc
        .get("groups")
        .and_then(toml_edit::Item::as_array_of_tables)
    {
        for table in groups {
            let id = table.get("id").and_then(toml_edit::Item::as_str);
            let same_path = table
                .get("path")
                .and_then(toml_edit::Item::as_str)
                .is_some_and(|p| {
                    std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p))
                        == replacement_path
                });
            if (id == Some(old) && same_path) || id == Some(entry.id.as_str()) {
                if !inserted {
                    let mut preserved = table.clone();
                    preserved.insert("id", replacement["id"].clone());
                    preserved.insert("path", replacement["path"].clone());
                    changed.push(preserved);
                    inserted = true;
                }
            } else {
                changed.push(table.clone());
            }
        }
    }
    if !inserted {
        changed.push(replacement);
    }
    doc.insert("groups", toml_edit::Item::ArrayOfTables(changed));
    write_doc(path, &doc)
}

/// Removes the `[[groups]]` entry of the group `id` (`pigeon leave`, 09
/// §9.7), keeping the rest of the file as it is.
pub fn remove_group(path: &Path, id: &str) -> Result<(), ConfigError> {
    let mut doc = read_doc(path)?;
    if let Some(groups) = doc
        .get_mut("groups")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
    {
        groups.retain(|t| t.get("id").and_then(toml_edit::Item::as_str) != Some(id));
    }
    write_doc(path, &doc)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]

    use super::*;

    #[test]
    fn a_setting_is_set_and_the_rest_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "format = 1\n# mine\nlanguage = \"en\"\n\n[network]\nmdns = false\n",
        )
        .unwrap();
        set_value(&path, "language", "fr".into()).unwrap();
        set_value(&path, "notifications", false.into()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# mine"), "{text}");
        let c = load(&path).unwrap();
        assert_eq!(c.language.as_deref(), Some("fr"));
        assert_eq!(c.notifications, Some(false));
        assert!(!c.network.mdns);
    }

    #[test]
    fn a_group_left_leaves_the_file_and_the_others_stay() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("config.toml");
        std::fs::write(&path, "# mine\nlanguage = \"fr\"\n").unwrap();
        for id in ["a", "b"] {
            let entry = GroupEntry {
                id: id.into(),
                path: t.path().join(id),
                priority: Default::default(),
            };
            add_group(&path, &entry).unwrap();
        }
        remove_group(&path, "a").unwrap();
        let c = load(&path).unwrap();
        assert_eq!(
            c.groups.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(),
            ["b"]
        );
        assert_eq!(c.language.as_deref(), Some("fr"));
        assert!(std::fs::read_to_string(&path).unwrap().contains("# mine"));
    }

    #[test]
    fn an_in_place_replacement_is_idempotent_and_preserves_other_groups() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("config.toml");
        std::fs::write(&path, "# mine\nlanguage = \"fr\"\nfuture_option = 7\n").unwrap();
        // macOS's /var -> /private/var alias exposed this naturally.
        // Exercise an equivalent real directory alias on every runner.
        let target = t.path().join("repositories");
        std::fs::create_dir(&target).unwrap();
        let alias = t.path().join("alias");
        std::os::unix::fs::symlink(&target, &alias).unwrap();
        let old_path = alias.join("old");
        std::fs::create_dir(&old_path).unwrap();
        let entries = [
            GroupEntry {
                id: "first".into(),
                path: t.path().join("first"),
                priority: Default::default(),
            },
            GroupEntry {
                id: "old".into(),
                path: old_path.clone(),
                priority: Default::default(),
            },
            GroupEntry {
                id: "old".into(),
                path: t.path().join("another-repository"),
                priority: Default::default(),
            },
            GroupEntry {
                id: "last".into(),
                path: t.path().join("last"),
                priority: Default::default(),
            },
        ];
        for entry in &entries {
            add_group(&path, entry).unwrap();
        }
        let new = GroupEntry {
            id: "new".into(),
            path: old_path,
            priority: Default::default(),
        };
        // A duplicate left by an older implementation is removed too.
        add_group(&path, &new).unwrap();
        replace_group(&path, "old", &new).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        replace_group(&path, "old", &new).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
        assert_eq!(
            load(&path).unwrap().groups,
            [
                entries[0].clone(),
                new,
                entries[2].clone(),
                entries[3].clone()
            ]
        );
        assert!(first.contains("# mine") && first.contains("future_option = 7"));
        assert_eq!(load(&path).unwrap().language.as_deref(), Some("fr"));
    }
}
