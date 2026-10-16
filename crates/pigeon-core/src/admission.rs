//! Purge-capable membership admission (proposed ADR 0076). A pending backup
//! is configured but not yet eligible for custody. Signed acknowledgements
//! bind its introduction; activation first merges their histories. The
//! initial protocol upgrade requires every non-recovery participant and a
//! nonempty custody seed. A later empty-seed bootstrap also uses all peers.
//! Laws cover missing acknowledgements, protocol support, causal boundaries,
//! bootstrap quorums and the pending fence on old-roster purge sealing.

use std::collections::{BTreeMap, BTreeSet};

/// The admission operation whose independently verified reports are read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind<D> {
    /// Initial rules-2 enablement, after every participant acknowledges it.
    Upgrade,
    /// A configured backup's activation, including its own capability ack.
    Custodian(D),
}

/// An admitted signed report's protocol assertion and exact accepted main.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ack<C> {
    /// Main containing the exact request/pending introduction.
    pub main: C,
    /// Exactly 2 implements this admission and purge protocol.
    pub protocol: u32,
}

/// The complete current reading. Signature/identity verification and the
/// immutable first-upgrade acknowledgement choice belong to the adapter.
pub struct View<'a, C, D> {
    /// Prospective accepted parent containing all acknowledgement histories.
    pub main: &'a C,
    /// The exact request or pending-device introduction.
    pub introduced: &'a C,
    /// Operation to activate.
    pub kind: &'a Kind<D>,
    /// Currently eligible custodians, excluding other pending devices.
    pub custodians: &'a BTreeSet<D>,
    /// All current non-recovery devices, including pending participants.
    pub participants: &'a BTreeSet<D>,
    /// Independently verified acknowledgements of this exact request.
    pub acknowledgements: &'a BTreeMap<D, Ack<C>>,
    /// Active old decisions not cancelled or closed by valid witnesses.
    pub unclosed_purges: bool,
}

impl<C: Ord, D: Ord + Clone> View<'_, C, D> {
    /// Every identity required for this transition. Empty current custody
    /// never becomes a vacuous admission quorum.
    pub fn required(&self) -> BTreeSet<D> {
        let mut required = if matches!(self.kind, Kind::Upgrade) || self.custodians.is_empty() {
            self.participants.clone()
        } else {
            self.custodians.clone()
        };
        if let Kind::Custodian(device) = self.kind {
            required.insert(device.clone());
        }
        required
    }

    /// Missing, unsupported or not-yet-merged participants, for the UI.
    pub fn waiting(&self, contains: impl Fn(&C, &C) -> bool) -> BTreeSet<D> {
        self.required()
            .into_iter()
            .filter(|device| {
                self.acknowledgements.get(device).is_none_or(|ack| {
                    ack.protocol != 2
                        || !contains(&ack.main, self.introduced)
                        || !contains(self.main, &ack.main)
                })
            })
            .collect()
    }

    /// Capability, exact history and completed/cancelled old purge epochs
    /// are all required. A new backup is never a fresh unsealed custodian
    /// for an old decision merely because its roster entry was added.
    pub fn ready(&self, contains: impl Fn(&C, &C) -> bool) -> bool {
        !self.unclosed_purges
            && !self.required().is_empty()
            && (!matches!(self.kind, Kind::Upgrade) || !self.custodians.is_empty())
            && self.waiting(contains).is_empty()
    }
}

/// Learning a pending admission fences sealing old-roster decisions. An
/// unknown concurrent decision must be cancelled/reissued after membership
/// settles, rather than silently excluding a newly eligible custodian.
pub fn may_seal<D: Ord>(
    original: &BTreeSet<D>,
    eligible: &BTreeSet<D>,
    pending_admission: bool,
) -> bool {
    !original.is_empty() && original == eligible && !pending_admission
}
