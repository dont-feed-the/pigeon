//! The join exchange on `pigeon/join/1` (09 §9.6): the new device sends one
//! [`Join`] line and the inviter answers one [`JoinAnswer`] line.
//!
//! The new device proves that it holds the invitation's secret with an HMAC
//! bound to both device IDs, which the QUIC handshake authenticated. The
//! slice adds the direct addresses of the new device, which the inviter
//! writes to its peers' list (13 §13.12).

use serde::{Deserialize, Serialize};

/// The version of [`Join`] and [`JoinAnswer`].
pub const JOIN_VERSION: u32 = 1;

/// What a new device sends to its inviter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Join {
    /// [`JOIN_VERSION`].
    pub v: u32,
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
    },
    /// Refused, with a problem code (10 §10.6).
    Refused {
        /// Such as `invite-expired`.
        code: String,
    },
}
