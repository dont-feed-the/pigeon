//! Local group-health view (12 §12.2, ADR 0025). These values come from
//! accepted state and signed device reports, without a telemetry backend.
//! Missing evidence stays explicit; stale observations are never safe.

use crate::api::{Machine, StatusReport};
use serde::{Deserialize, Serialize};

/// A partition of current files or retained older versions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    /// No copy is known.
    pub missing: u64,
    /// Released by a purge.
    pub erased: u64,
    /// Only this computer has it.
    pub local: u64,
    /// Another computer knows it; backup copies are still needed.
    pub published: u64,
    /// Enough copies were seen but the check is stale.
    pub uncertain: u64,
    /// Enough recently checked backup copies.
    pub protected: u64,
}

/// A complete local view for the Home page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthReport {
    /// The group's genesis; identifies changes of scope in the local view.
    pub group_id: String,
    /// The group's human name.
    pub group: String,
    /// When this view was read, RFC 3339.
    pub at: String,
    /// The normal headline and explanations, including retained history.
    pub status: StatusReport,
    /// Additional current-file explanations beyond the displayed examples.
    pub omitted_paths: u64,
    /// Current files; absent if their inventory could not be read.
    pub current: Option<Counts>,
    /// Retained versions outside the current tree; absent on read failure.
    pub retained: Option<Counts>,
    /// Suggestions awaiting a decision; absent on read failure.
    pub suggestions: Option<u64>,
    /// Retained keys known to need replacement copies.
    pub endangered: u64,
    /// Every current computer and paper key, in roster order.
    pub machines: Vec<MachineHealth>,
    /// Accepted removals that initiated replacement work.
    pub repair_epochs: Option<Vec<RepairEpoch>>,
}

/// A computer's latest admitted report, shown with its time and freshness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineHealth {
    /// Its identity, role and last contact.
    pub machine: Machine,
    /// Time of its latest admitted report, RFC 3339.
    pub reported_at: Option<String>,
    /// Whether that report is within the group's check window.
    pub report_fresh: bool,
    /// Disk figures from that report, never inferred from telemetry.
    pub disk: Option<Disk>,
    /// The backup is checking its restored repository.
    pub checking: bool,
    /// Its report says the repository is lost.
    pub repository_lost: bool,
    /// Work reported by that computer, if known.
    pub queue: Option<Queue>,
    /// Last integrity pass from its signed report; local progress when known.
    #[serde(default)]
    pub scrub: Option<Scrub>,
}

/// Disk capacity in bytes, from one signed report snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disk {
    /// Annex content present.
    pub used: u64,
    /// Space available to the writer.
    pub free: u64,
    /// Group storage limit, zero if unset.
    pub maxsize: u64,
    /// Reserved free space.
    pub reserve: u64,
    /// Free space is below the reserve.
    pub full: bool,
}

/// Work queues from one signed report snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queue {
    /// Content waiting to send.
    pub send_bytes: u64,
    /// Content waiting to receive.
    pub receive_bytes: u64,
    /// Keys deferred for lack of capacity.
    pub deferred_keys: u64,
    /// Space missing for that deferred work.
    pub missing_bytes: u64,
}

/// A roster removal starts a causally identified repair epoch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairEpoch {
    /// Accepted roster commit introducing the removal.
    pub commit: String,
    /// Removed device's identity.
    pub device: String,
    /// Its last accepted name, if available.
    pub name: String,
    /// Commit author time, RFC 3339.
    pub started_at: String,
}

/// Integrity evidence. Pending counts describe this local daemon only;
/// absent remote progress never means that a pass is complete.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scrub {
    /// Last durably completed pass, not the start of the current pass.
    pub last_pass_completed: Option<String>,
    /// Findings in that completed pass, not a claim about current corruption.
    pub keys_failed: u64,
    /// Keys left in an active local pass, when locally observed.
    pub remaining: Option<u64>,
    /// Findings in the active local pass, when locally observed.
    pub current_failures: Option<u64>,
    /// A local progress-reading problem, without raw filesystem diagnostics.
    pub problem: Option<crate::Message>,
}
