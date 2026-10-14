//! What opens each iroh stream between devices (09 §9.3, §9.8; ADR 0066).
//!
//! A stream starts with one header line naming the group, the peer answers
//! with one line, and then the stream carries git-annex's protocol
//! unchanged. A device offers every version of each ALPN it supports
//! (ADR 0048).

use serde::{Deserialize, Serialize};

/// One git-annex P2P session per stream, git and content.
pub const ALPN_ANNEX: &[u8] = b"pigeon/annex/1";
/// The join exchange (09 §9.6).
pub const ALPN_JOIN: &[u8] = b"pigeon/join/1";
/// OTLP to the group's telemetry device (ADR 0025).
pub const ALPN_TELEMETRY: &[u8] = b"pigeon/telemetry/1";
/// `pigeon debug bundle` requests (12).
pub const ALPN_DEBUG: &[u8] = b"pigeon/debug/1";

/// The version of the header and answer below.
pub const HEADER_VERSION: u32 = 1;

/// The longest header or answer line, its newline excluded.
pub const MAX_HEADER: usize = 4096;

/// The first line of a stream, from the device that opens it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamHeader {
    /// [`HEADER_VERSION`].
    pub v: u32,
    /// The group whose repository serves the stream; checked against the
    /// roster of that group for admission.
    pub group: String,
    /// W3C trace context, so one action is one trace across devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traceparent: Option<String>,
}

/// The peer's one-line answer to a [`StreamHeader`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamAnswer {
    /// [`HEADER_VERSION`].
    pub v: u32,
    /// Whether git-annex's protocol follows.
    pub ok: bool,
    /// Why not, as a stable problem code (10 §10.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Refusal>,
}

/// Why a peer refuses a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refusal {
    /// The device is not in the roster of that group.
    NotInRoster,
    /// The peer serves no repository of that group.
    UnknownGroup,
    /// The peer's repository is not listening (its remotedaemon is down).
    NotListening,
    /// The header could not be read.
    BadHeader,
    /// The header's version is newer than the peer's (ADR 0048).
    PeerTooOld,
}

impl StreamHeader {
    /// The header as one line, newline included.
    pub fn to_line(&self) -> Vec<u8> {
        line(self)
    }
}

impl StreamAnswer {
    /// An acceptance.
    pub fn accept() -> Self {
        StreamAnswer {
            v: HEADER_VERSION,
            ok: true,
            refusal: None,
        }
    }

    /// A refusal with its reason.
    pub fn refuse(refusal: Refusal) -> Self {
        StreamAnswer {
            v: HEADER_VERSION,
            ok: false,
            refusal: Some(refusal),
        }
    }

    /// The answer as one line, newline included.
    pub fn to_line(&self) -> Vec<u8> {
        line(self)
    }
}

fn line<T: Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}
