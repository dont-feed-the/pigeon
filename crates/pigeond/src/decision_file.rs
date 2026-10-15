//! `.pigeon/decisions/<commit>.toml` (04 §4.5): a decline or a withdrawal
//! of the proposal `<commit>`, committed on `main`.
//!
//! ```toml
//! format = 1
//! proposal = "<commit id>"
//! verdict = "rejected"        # or "withdrawn"
//! note = "On garde la recette de Mamie."
//! ```
//!
//! The schema is closed (ADR 0048): another `format`, a key it does not
//! define or a verdict this release does not know is [`Unread::Unknown`];
//! anything else that does not fit is [`Unread::Malformed`]. Law
//! (`tests/codecs.rs`): parsing a rendered decision gives it back.

use pigeon_core::authority::{Decided, Unread};
use serde::{Deserialize, Serialize};

/// The format this release writes.
pub const FORMAT: u32 = 1;

/// A decision as the file says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionDoc {
    /// The proposal's commit.
    pub proposal: String,
    /// What is decided.
    pub verdict: Decided,
    /// Why, for its author.
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileT {
    format: u32,
    proposal: String,
    verdict: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

const KEYS: &[&str] = &["format", "proposal", "verdict", "note"];

/// Reads a decision file.
pub fn parse(text: &str) -> Result<DecisionDoc, Unread> {
    let doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| Unread::Malformed(e.to_string()))?;
    match doc.get("format").and_then(toml_edit::Item::as_integer) {
        Some(f) if f == i64::from(FORMAT) => {}
        Some(f) if f > i64::from(FORMAT) => return Err(Unread::Unknown(format!("format {f}"))),
        _ => return Err(Unread::Malformed("format".to_owned())),
    }
    if let Some((k, _)) = doc.iter().find(|(k, _)| !KEYS.contains(k)) {
        return Err(Unread::Unknown(format!("key {k}")));
    }
    let f: FileT = toml_edit::de::from_str(text).map_err(|e| Unread::Malformed(e.to_string()))?;
    let verdict = match f.verdict.as_str() {
        "rejected" => Decided::Rejected,
        "withdrawn" => Decided::Withdrawn,
        other => return Err(Unread::Unknown(format!("verdict {other}"))),
    };
    if f.proposal.len() != 40 || !f.proposal.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Unread::Malformed("proposal".to_owned()));
    }
    Ok(DecisionDoc {
        proposal: f.proposal,
        verdict,
        note: f.note,
    })
}

/// Writes a decision file.
pub fn render(d: &DecisionDoc) -> String {
    let f = FileT {
        format: FORMAT,
        proposal: d.proposal.clone(),
        verdict: match d.verdict {
            Decided::Rejected => "rejected",
            Decided::Withdrawn => "withdrawn",
        }
        .to_owned(),
        note: d.note.clone(),
    };
    toml_edit::ser::to_string(&f).unwrap_or_default()
}
