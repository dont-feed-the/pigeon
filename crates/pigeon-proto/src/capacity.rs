//! Read-only storage accounting (08 §8.9). Need counts a distinct key
//! once, with current/history/suggestions precedence, multiplied by K.
//! Device figures remain timestamped reports; absence is never zero.

use serde::{Deserialize, Serialize};

/// One capacity snapshot, with configured need and dated observations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Group name.
    pub group: String,
    /// Snapshot time.
    pub at: String,
    /// Required distinct backup copies.
    pub k: u32,
    /// K times current keys' bytes.
    pub current: u64,
    /// K times history keys not counted as current.
    pub history: u64,
    /// K times suggestion keys not counted in accepted history.
    pub suggestions: u64,
    /// Actual extra custody copies above K, when location evidence is readable.
    pub overflow: Option<u64>,
    /// Sum of min(offered limit, one copy of all retained content).
    pub configured: u64,
    /// Capacity below 1.25 times required content.
    pub short: bool,
    /// Signed observations, including report times and missing evidence.
    pub machines: Vec<crate::health::MachineHealth>,
    /// Retained suggestions, including declined and withdrawn ones.
    pub proposals: Vec<Proposal>,
}

/// A retained suggestion and its extra storage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    /// Exact signed suggestion id.
    pub id: String,
    /// Commit author time.
    pub saved_at: String,
    /// open, adopted, rejected or withdrawn.
    pub state: String,
    /// Distinct retained bytes beyond accepted histories, before K.
    /// Shared bytes can appear in multiple rows but count once in Report.
    pub additional_bytes: u64,
}
