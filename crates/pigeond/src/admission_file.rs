//! Closed, content-addressed admission metadata (proposed ADR 0081).
//! An upgrade request freezes its starting membership; a custody request
//! names the actual accepted device introduction. Full signed reports are
//! retained separately, since hashes written in TOML are not Git edges.

use std::collections::BTreeMap;

use pigeon_core::authority::Unread;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::report_file::AdmissionKind;

/// Requests are made by admins. Witnesses grant no discretionary right.
pub const PREFIX: &str = ".pigeon/admissions/";
/// Only the initial upgrade needs a replicated request document.
pub const REQUESTS: &str = ".pigeon/admissions/requests/";
/// Verified immutable activation evidence.
pub const WITNESSES: &str = ".pigeon/admissions/witnesses/";

/// Exact operation. Its canonical bytes determine the acknowledgement id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Exactly one.
    pub format: u32,
    /// Upgrade, or activation of one configured custodian.
    pub kind: AdmissionKind,
    /// Upgrade snapshot, or actual accepted device introduction.
    pub base: String,
    /// Present only for custody activation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

/// A full causal certificate, checked independently by every reader.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Witness {
    /// Exactly one.
    pub format: u32,
    /// The exact operation acknowledged by every signer.
    pub request: Request,
    /// Accepted head containing every acknowledgement's main.
    pub main: String,
    /// Device id to full signed report commit id.
    pub reports: BTreeMap<String, String>,
}

fn identity(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn hash(text: &str) -> String {
    crate::util::hex(&Sha256::digest(text.as_bytes()))
}

fn schema(text: &str, fields: &[&str]) -> Result<(), Unread> {
    let doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| Unread::Malformed(e.to_string()))?;
    match doc.get("format").and_then(toml_edit::Item::as_integer) {
        Some(1) => {}
        Some(n) if n > 1 => return Err(Unread::Unknown("admission format".into())),
        _ => return Err(Unread::Malformed("admission format".into())),
    }
    if doc.iter().any(|(key, _)| !fields.contains(&key)) {
        return Err(Unread::Unknown("admission field".into()));
    }
    Ok(())
}

impl Request {
    /// Canonical bytes: unknown fields can never enter a signature claim.
    pub fn render(&self) -> Result<String, String> {
        toml_edit::ser::to_string(self).map_err(|e| e.to_string())
    }

    /// Stable opaque identity of the exact operation and snapshot.
    pub fn id(&self) -> Result<String, String> {
        Ok(hash(&self.render()?))
    }

    /// Content-addressed upgrade request path.
    pub fn path(&self) -> Result<String, String> {
        Ok(format!("{REQUESTS}{}.toml", self.id()?))
    }

    /// Validate even when embedded inside a certificate.
    pub fn validate(&self) -> Result<(), Unread> {
        if self.format != 1 {
            return Err(Unread::Unknown("admission request format".into()));
        }
        if !crate::purge_file::commit_id(&self.base)
            || match self.kind {
                AdmissionKind::Upgrade => self.device.is_some(),
                AdmissionKind::Custodian => self.device.as_ref().is_none_or(|d| !identity(d)),
            }
        {
            return Err(Unread::Malformed("admission identity".into()));
        }
        Ok(())
    }

    /// Future fields/kinds wait for an update. Wrong names are malformed.
    pub fn parse(path: &str, text: &str) -> Result<Self, Unread> {
        schema(text, &["format", "kind", "base", "device"])?;
        let table: toml_edit::DocumentMut = text
            .parse()
            .map_err(|e: toml_edit::TomlError| Unread::Malformed(e.to_string()))?;
        if !matches!(
            table.get("kind").and_then(toml_edit::Item::as_str),
            Some("upgrade" | "custodian")
        ) {
            return Err(Unread::Unknown("admission kind".into()));
        }
        let request: Self =
            toml_edit::de::from_str(text).map_err(|e| Unread::Malformed(e.to_string()))?;
        request.validate()?;
        if request.kind != AdmissionKind::Upgrade || request.path().as_deref() != Ok(path) {
            return Err(Unread::Malformed("admission request name".into()));
        }
        Ok(request)
    }
}

impl Witness {
    /// Exact bytes and their collision-free witness path.
    pub fn render(&self) -> Result<(String, String), String> {
        let text = toml_edit::ser::to_string(self).map_err(|e| e.to_string())?;
        Ok((format!("{WITNESSES}{}.toml", hash(&text)), text))
    }

    /// Closed schema, including the nested request. No unknown capability
    /// can be interpreted as agreement with this protocol.
    pub fn parse(path: &str, text: &str) -> Result<Self, Unread> {
        schema(text, &["format", "request", "main", "reports"])?;
        let table: toml_edit::DocumentMut = text
            .parse()
            .map_err(|e: toml_edit::TomlError| Unread::Malformed(e.to_string()))?;
        let nested = table
            .get("request")
            .and_then(toml_edit::Item::as_table_like)
            .ok_or_else(|| Unread::Malformed("missing admission operation".into()))?;
        if nested
            .iter()
            .any(|(key, _)| !["format", "kind", "base", "device"].contains(&key))
            || nested.get("format").and_then(toml_edit::Item::as_integer) != Some(1)
            || !matches!(
                nested.get("kind").and_then(toml_edit::Item::as_str),
                Some("upgrade" | "custodian")
            )
        {
            return Err(Unread::Unknown("admission operation".into()));
        }
        let witness: Self =
            toml_edit::de::from_str(text).map_err(|e| Unread::Malformed(e.to_string()))?;
        witness.request.validate()?;
        if !crate::purge_file::commit_id(&witness.main)
            || witness.reports.is_empty()
            || witness
                .reports
                .iter()
                .any(|(d, c)| !identity(d) || !crate::purge_file::commit_id(c))
            || path != format!("{WITNESSES}{}.toml", hash(text))
        {
            return Err(Unread::Malformed(
                "admission witness identity or name".into(),
            ));
        }
        Ok(witness)
    }
}
