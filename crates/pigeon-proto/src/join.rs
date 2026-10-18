//! The group's invitation and the join exchange on `pigeon/join/1` (09 §9.6,
//! ADR 0075).
//!
//! The invitation is written `pgn2-` plus the lowercase unpadded base32 of
//! its JSON, and shared inside the line [`command`] makes. The new device
//! sends one [`Join`] line to a device the invitation lists, which answers
//! one [`JoinAnswer`] line. The new device proves that it holds the
//! invitation's secret with an HMAC bound to both device IDs, which the QUIC
//! handshake authenticated. A recovery device joins a new machine of its
//! person the same way, with no invitation (09 §9.9).

use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};

/// The version of [`Join`] and [`JoinAnswer`].
pub const JOIN_VERSION: u32 = 2;

/// The version of [`Invitation`].
pub const INVITATION_VERSION: u32 = 2;

/// Where the installer of the latest release is (14 §14.5).
pub const INSTALLER: &str =
    "https://github.com/dont-feed-the/pigeon/releases/latest/download/pigeon-install";

/// What the group's invitation carries.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invitation {
    /// [`INVITATION_VERSION`].
    pub v: u32,
    /// The group ID.
    pub group: String,
    /// The group's name.
    pub name: String,
    /// K, for a new custodian's `wanted`.
    pub k: u32,
    /// The devices that admit, when it was written: the admins' laptops
    /// and backup machines.
    pub admitters: Vec<Admitter>,
    /// The roster's invitation: 256 bits, hex.
    pub secret: String,
    /// The group's own relay, through which the new device reaches the
    /// admitters (09 §9.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
    /// For a refounded group, what the joiner accepts (ADR 0047).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refounded: Option<Refounded>,
}

impl std::fmt::Debug for Invitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invitation")
            .field("v", &self.v)
            .field("group", &self.group)
            .field("name", &self.name)
            .field("k", &self.k)
            .field("admitters", &self.admitters)
            .field("secret", &"[redacted]")
            .field("relay", &self.relay)
            .field("refounded", &self.refounded)
            .finish()
    }
}

/// A device that admits new ones.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admitter {
    /// Its device ID.
    pub device: String,
    /// Its person's name, for messages.
    pub person: String,
    /// Its direct addresses, when the invitation was written.
    #[serde(default)]
    pub addresses: Vec<String>,
}

/// What a new device sends to an admitter.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Join {
    /// [`JOIN_VERSION`].
    pub v: u32,
    /// W3C trace context of the joining action (ADR 0025).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traceparent: Option<String>,
    /// SHA-256 of the invitation's secret, hex: which group it joins.
    pub invitation_hash: String,
    /// HMAC-SHA256(secret, "pigeon-join-1" ‖ new device ID ‖ admitter ID), hex.
    pub proof: String,
    /// The name its person gives.
    #[serde(default)]
    pub person: String,
    /// Whether it may join the person of the roster with that name.
    #[serde(default)]
    pub existing: bool,
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

impl std::fmt::Debug for Join {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Join")
            .field("v", &self.v)
            .field("invitation_hash", &self.invitation_hash)
            .field("proof", &"[redacted]")
            .field("person", &self.person)
            .field("device_name", &self.device_name)
            .field("role", &self.role)
            .field("annex_uuid", &self.annex_uuid)
            .finish_non_exhaustive()
    }
}

/// The admitter's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum JoinAnswer {
    /// The device is in the roster from `commit` on.
    Welcome {
        /// The roster commit that adds the device.
        commit: String,
        /// The admitter's git-annex UUID, for the new device's remote.
        annex_uuid: String,
        /// The direct addresses the admitter knows of the group's other
        /// devices, by device ID, so the new one can reach them (09 §9.3).
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        peers: std::collections::BTreeMap<String, Vec<String>>,
    },
    /// Refused, with a problem code (10 §10.6), such as `person-exists`.
    Refused {
        /// The code.
        code: String,
    },
}

/// What an invitation to a refounded group says of it (09 §9.9, ADR 0047):
/// joining means accepting `by` as its admin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refounded {
    /// The old group's ID.
    pub from: String,
    /// The refounder's name.
    pub by: String,
    /// When, RFC 3339.
    pub at: String,
}

/// Writes an invitation as `pgn2-…`.
pub fn encode(inv: &Invitation) -> String {
    let json = serde_json::to_vec(inv).unwrap_or_default();
    format!("pgn2-{}", BASE32_NOPAD.encode(&json).to_ascii_lowercase())
}

/// Reads a `pgn2-…` invitation of this version.
pub fn decode(code: &str) -> Option<Invitation> {
    let body = code.trim().strip_prefix("pgn2-")?;
    let bytes = BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .ok()?;
    serde_json::from_slice(&bytes)
        .ok()
        .filter(|i: &Invitation| i.v == INVITATION_VERSION)
}

/// The group's name and what the invitation `code` says of its refounding,
/// if it is one to a refounded group; for the question asked before joining.
pub fn refounded(code: &str) -> Option<(String, Refounded)> {
    let inv = decode(code)?;
    Some((inv.name, inv.refounded?))
}

/// The line to share: it installs or updates pigeon, then joins with `code`.
pub fn command(code: &str) -> String {
    format!("curl -fsSL {INSTALLER} | sh -s -- --join {code}")
}

/// The `pgn2-` invitation in `pasted`, as a person pastes it from a message:
/// wrapped over lines, or inside the whole line [`command`] makes. The
/// code's parts are the longest run of words after `pgn2-` that decodes to
/// an invitation. Text holding no `pgn2-` is answered trimmed, so that the
/// join refuses it.
pub fn invitation(pasted: &str) -> String {
    let Some(start) = pasted.find("pgn2-") else {
        return pasted.trim().to_owned();
    };
    crate::join_paste::find(&pasted[start + 5..]).unwrap_or_else(|| pasted.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code() -> String {
        encode(&Invitation {
            v: INVITATION_VERSION,
            group: "g".into(),
            name: "Family".into(),
            k: 1,
            admitters: Vec::new(),
            secret: "00".into(),
            relay: None,
            refounded: None,
        })
    }

    #[test]
    fn a_pasted_invitation_is_found_wrapped_or_inside_its_command() {
        let code = code();
        let (a, b) = code.split_at(20);
        assert_eq!(invitation(&format!("  {code}\n")), code);
        assert_eq!(invitation(&format!("{a}\n{b}")), code);
        assert_eq!(invitation(&format!("{a} {b}")), code);
        assert_eq!(invitation(&format!("pigeon join {code} family")), code);
        assert_eq!(invitation(&command(&code)), code);
        assert_eq!(
            invitation(&format!("{} --backup-machine", command(&code))),
            code
        );
    }

    #[test]
    fn a_long_message_after_a_valid_invitation_does_not_hide_it() {
        let code = code();
        let pasted = format!("{code} {}", "a ".repeat(100_000));
        assert_eq!(invitation(&pasted), code);
    }

    #[test]
    fn text_holding_no_invitation_is_only_trimmed() {
        assert_eq!(invitation(" garbage \n"), "garbage");
        assert_eq!(invitation("pgn2-!!"), "pgn2-!!");
    }

    #[test]
    fn an_invitation_round_trips() {
        assert_eq!(decode(&code()).map(|i| encode(&i)), Some(code()));
    }
}

#[cfg(test)]
#[path = "join_private_debug_tests.rs"]
mod private_debug_tests;

#[cfg(test)]
#[path = "join_schema_tests.rs"]
mod schema_tests;
