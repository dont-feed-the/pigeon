//! The join exchange on `pigeon/join/1` (09 §9.6): the new device sends one
//! [`Join`] line and the inviter answers one [`JoinAnswer`] line.
//!
//! The new device proves that it holds the invitation's secret with an HMAC
//! bound to both device IDs, which the QUIC handshake authenticated. The
//! slice adds the direct addresses of the new device, which the inviter
//! writes to its peers' list (13 §13.12). A recovery device joins a new
//! machine of its person the same way, with no invitation (09 §9.9).

use serde::{Deserialize, Serialize};

/// The version of [`Join`] and [`JoinAnswer`].
pub const JOIN_VERSION: u32 = 1;

/// What a new device sends to its inviter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Join {
    /// [`JOIN_VERSION`].
    pub v: u32,
    /// W3C trace context of the joining action (ADR 0025).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traceparent: Option<String>,
    /// SHA-256 of the invitation's secret, hex.
    pub invitation_hash: String,
    /// HMAC-SHA256(secret, "pigeon-join-1" ‖ new device ID ‖ inviter device ID), hex.
    pub proof: String,
    /// The new device's name in the roster.
    pub device_name: String,
    /// `custodian` or `laptop`.
    pub role: String,
    /// Its git-annex UUID in the group.
    pub annex_uuid: String,
    /// The free space it offers, in bytes, for a custodian.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<u64>,
    /// Its direct addresses.
    #[serde(default)]
    pub addresses: Vec<String>,
    /// Set when a recovery device sends the join (09 §9.9, ADR 0047): the
    /// device ID of the new machine it adds for its person. The invitation
    /// fields are then empty: the recovery device's own key, which the
    /// QUIC handshake authenticated, is the authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_device: Option<String>,
    /// A sheet-signed roster commit, pulled from the authenticated recovery
    /// endpoint before the custodian accepts the new computer (ADR 0047).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_commit: Option<String>,
    /// Direct addresses of the temporary endpoint using the sheet key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovery_addresses: Vec<String>,
    /// Recovery exchange version; older custodian-signed recovery is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_version: Option<u32>,
}

/// The inviter's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JoinAnswer {
    /// The device is in the roster from `commit` on.
    Welcome {
        /// The roster commit that adds the device.
        commit: String,
        /// The inviter's git-annex UUID, for the new device's remote.
        annex_uuid: String,
        /// The direct addresses the inviter knows of the group's other
        /// devices, by device ID, so the new one can reach them (09 §9.3).
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        peers: std::collections::BTreeMap<String, Vec<String>>,
    },
    /// Refused, with a problem code (10 §10.6).
    Refused {
        /// Such as `invite-expired`.
        code: String,
    },
}

/// What an invitation to a refounded group says of it (09 §9.9, ADR 0047):
/// joining means accepting `by` as its admin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refounded {
    /// The old group's ID.
    pub from: String,
    /// The refounder's name.
    pub by: String,
    /// When, RFC 3339.
    pub at: String,
}

/// The group's name and what the `pgn1-` invitation `code` says of its
/// refounding, if it is one to a refounded group; for the question the
/// CLI asks before joining.
pub fn refounded(code: &str) -> Option<(String, Refounded)> {
    let body = code.trim().strip_prefix("pgn1-")?;
    let bytes = data_encoding::BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let r = serde_json::from_value(v.get("refounded")?.clone()).ok()?;
    Some((v.get("name")?.as_str()?.to_owned(), r))
}
