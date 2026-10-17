//! Erasure preview, explicit preparation/confirmation and waiting state
//! (08 §8.6). A preview is read-only. Its token binds the exact retained
//! history; a changed reading requires another preview and confirmation.

use serde::{Deserialize, Serialize};

/// Erase all observed versions of a path, or one retained suggestion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum Selection {
    /// Absolute local path; deleted paths are allowed.
    Path(String),
    /// Unambiguous retained suggestion commit or prefix.
    Suggestion(String),
}

/// No future readiness change submits an erasure automatically.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Operation {
    /// Read-only history and capacity scope.
    Preview {
        /// The exact path or suggestion to review.
        selection: Selection,
    },
    /// Explicit admin preparation; no erasure is requested yet.
    Prepare {
        /// Identifies the group; does not submit this erasure.
        selection: Selection,
    },
    /// Confirm only the history in the immediately reviewed preview.
    Confirm {
        /// The selection that was reviewed.
        selection: Selection,
        /// Token of that exact preview.
        token: String,
    },
    /// List decisions and missing acknowledgements.
    List,
    /// Read-only cancellation preview.
    PreviewCancel {
        /// Stable decision id or unambiguous prefix.
        decision: String,
    },
    /// Explicitly cancel a still-pending decision with a fresh token.
    Cancel {
        /// Decision from the cancellation preview.
        decision: String,
        /// Token of that exact cancellation preview.
        token: String,
    },
}

/// An actual named machine; absence of its acknowledgement is explicit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waiting {
    /// Immutable device id, for diagnostics and identity disambiguation.
    pub device: String,
    /// Current roster display name.
    pub name: String,
}

/// Capability/membership readiness, independent of a destructive request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Readiness {
    /// The group runs rules 2, with settled nonempty custody.
    pub ready: bool,
    /// This device’s person may explicitly start protocol preparation.
    pub can_prepare: bool,
    /// An admin must explicitly start the one-time upgrade request.
    pub preparation_needed: bool,
    /// Participants or backups still missing the required signed evidence.
    pub waiting: Vec<Waiting>,
    /// Old erasures that need completion or cancellation first.
    pub open_decisions: Vec<String>,
    /// A local checking problem prevents readiness; never raw diagnostics.
    pub problem: Option<crate::Message>,
}

/// Counts describe reference versions; shared content may remain needed
/// by other paths and cannot be promised as recoverable free disk space.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    /// What was requested, in the same form needed for confirmation.
    pub selection: Selection,
    /// Digest of exact group, refs and selected scope.
    pub token: String,
    /// Files removed from the current accepted tree.
    pub current_files: u64,
    /// Covered historical reference versions.
    pub versions: u64,
    /// Distinct content bytes covered, before shared-key exclusions.
    pub bytes: u64,
    /// Covered versions whose bytes also belong to an uncovered reference.
    pub shared_versions: u64,
    /// Other retaining paths, at most twenty examples for the warning.
    pub retained_elsewhere: Vec<String>,
    /// Configured minimum wait after each backup receives the decision.
    pub retention_days: u32,
    /// This device's person needs admin approval for this request.
    pub needs_approval: bool,
    /// Current protocol and custody readiness.
    pub readiness: Readiness,
}

/// One accepted erasure, retaining its stable identity after restarts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    /// Stable introduction/path identity, also accepted as a CLI prefix.
    pub id: String,
    /// Erased paths in the signed decision.
    pub paths: Vec<String>,
    /// Earliest declared UTC deadline; local receipt waits also apply.
    pub not_before: String,
    /// Backups whose immutable seal has not yet been admitted.
    pub waiting: Vec<Waiting>,
    /// Original quorum completion has durable signed evidence.
    pub completed: bool,
    /// An unfinished decision's original quorum no longer matches.
    pub membership_changed: bool,
}

/// Typed result shared by CLI and browser; text is rendered with Fluent.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub enum Answer {
    /// Nothing changed.
    Preview(Preview),
    /// Preparation began; a fresh preview/confirmation is still required.
    Preparing(Readiness),
    /// Request entered main, or awaits an admin as a signed suggestion.
    Submitted {
        /// Decision identity, or suggestion commit when approval is needed.
        id: String,
        /// True when an admin must still decide the signed suggestion.
        needs_approval: bool,
    },
    /// Current decisions, including completed epochs.
    Decisions {
        /// Every active accepted decision.
        decisions: Vec<Decision>,
        /// This device’s person is an admin allowed to cancel pending requests.
        can_cancel: bool,
    },
    /// Confirmation of a still-pending cancellation.
    CancelPreview {
        /// Still-pending decision being reviewed.
        decision: Decision,
        /// Exact current-history cancellation token.
        token: String,
    },
    /// Metadata cancellation cannot reconstruct already erased bytes.
    Cancelled {
        /// Stable identity of the cancelled decision.
        id: String,
    },
}
