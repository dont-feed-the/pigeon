//! Closed format-1 purge decisions (08 §8.6): an exact historical scope,
//! explicit proposal tips and a UTC retention end. Parsing alone confers
//! no authority: admission also checks signatures, the parent roster,
//! accepted bases and the minimum retention period (P4).

use pigeon_core::authority::Unread;
use pigeon_core::purge::Decision;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Only this namespace carries purge decisions.
pub const PREFIX: &str = ".pigeon/decisions/purge-";

/// The replicated decision. Its identity is its introducing commit plus
/// path, not this filename: cancel/re-add must not reuse an earlier seal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurgeDoc {
    /// Replicated schema version.
    pub format: u32,
    /// Exactly `purge` in this version.
    pub kind: String,
    /// Portable relative paths, with an optional trailing folder slash.
    pub paths: Vec<String>,
    /// The accepted history the decider saw.
    pub base: String,
    /// False erases only a named suggestion's births beyond its accepted
    /// base. Older accepted versions sharing its bytes remain retained.
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub history: bool,
    /// Empty means every version in scope.
    #[serde(default)]
    pub keys: Vec<String>,
    /// Retained proposals explicitly included in this decision.
    #[serde(default)]
    pub proposals: Vec<String>,
    /// The requesting person's roster identity.
    pub requested_by: String,
    /// UTC end; never sufficient without the receiver's own elapsed wait.
    pub retention_end: toml_edit::Datetime,
}

/// A full object identity, never a Git expression or abbreviation.
pub fn commit_id(id: &str) -> bool {
    id.len() == 40
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn yes() -> bool {
    true
}
fn is_yes(value: &bool) -> bool {
    *value
}

fn scope(path: &str) -> Option<String> {
    if path.is_empty() {
        return Some(String::new());
    }
    if path.starts_with('/') {
        return None;
    }
    let path = path.strip_suffix('/').unwrap_or(path);
    if path.is_empty()
        || pigeon_core::names::portable(path).is_err()
        || pigeon_core::authority::is_pigeon_path(path)
        || path == ".git"
    {
        return None;
    }
    Some(path.to_owned())
}

impl PurgeDoc {
    /// Its UTC deadline in Unix seconds.
    pub fn end(&self) -> Option<i64> {
        let text = self.retention_end.to_string();
        if !text.ends_with('Z') && !text.ends_with("+00:00") {
            return None;
        }
        crate::util::parse_rfc3339(&text)
    }

    /// Canonical scope for the pure release algebra, after parsing.
    pub fn decision(&self) -> Decision<String, String> {
        Decision {
            base: self.base.clone(),
            history: self.history,
            paths: self.paths.iter().filter_map(|p| scope(p)).collect(),
            keys: self.keys.iter().cloned().collect(),
            proposals: self.proposals.iter().cloned().collect(),
        }
    }

    /// P4 uses the retention configured in each accepted parent roster,
    /// never a later setting. Arithmetic cannot wrap at timestamp limits.
    pub fn valid_end(&self, recorded_at: i64, retention_seconds: u64) -> bool {
        self.end()
            .is_some_and(|end| pigeon_core::purge::valid_end(recorded_at, retention_seconds, end))
    }
}

/// Parse known fields and values, keeping unknown schemas distinguishable
/// from malformed scopes. No unknown decision authorizes release.
pub fn parse(text: &str) -> Result<PurgeDoc, Unread> {
    let table: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| Unread::Malformed(e.to_string()))?;
    match table.get("format").and_then(toml_edit::Item::as_integer) {
        Some(1) => {}
        Some(n) if n > 1 => return Err(Unread::Unknown("purge format".into())),
        _ => return Err(Unread::Malformed("purge format".into())),
    }
    const FIELDS: &[&str] = &[
        "format",
        "kind",
        "paths",
        "base",
        "history",
        "keys",
        "proposals",
        "requested_by",
        "retention_end",
    ];
    if table.iter().any(|(k, _)| !FIELDS.contains(&k)) {
        return Err(Unread::Unknown("purge field".into()));
    }
    let doc: PurgeDoc =
        toml_edit::de::from_str(text).map_err(|e| Unread::Malformed(e.to_string()))?;
    if doc.kind != "purge" {
        return Err(Unread::Unknown("purge kind".into()));
    }
    let paths: Option<BTreeSet<String>> = doc.paths.iter().map(|p| scope(p)).collect();
    if !commit_id(&doc.base)
        || doc.proposals.iter().any(|p| !commit_id(p))
        || (!doc.history && doc.proposals.len() != 1)
        || doc.keys.iter().any(|k| {
            pigeon_core::capacity::key_size(k).is_none()
                || !pigeon_core::authority::is_pointer(format!("/annex/objects/{k}\n").as_bytes())
        })
        || doc.requested_by.is_empty()
        || doc.end().is_none()
        || paths
            .as_ref()
            .is_none_or(|p| p.is_empty() || p.len() != doc.paths.len())
    {
        return Err(Unread::Malformed("purge scope or time".into()));
    }
    Ok(doc)
}

/// Write exactly this version's closed schema.
pub fn render(doc: &PurgeDoc) -> Result<String, toml_edit::ser::Error> {
    toml_edit::ser::to_string(doc)
}
