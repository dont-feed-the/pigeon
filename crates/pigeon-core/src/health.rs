//! Group-health counts from observed file state (12 §12.2, ADR 0025).
//! Counts combine as a commutative monoid with saturating natural numbers.
//! Pending saves replace their path's older state and count once as local.
//! Uncertain checks never become protected counts. Four laws, 1000 cases
//! each, live in `tests/health.rs`; four repair-epoch laws are in
//! `tests/health_repair.rs`. Telemetry is not an input.

use crate::roster::{DeviceId, History, apply};
use crate::state::{Missing, State};

/// The accepted roster change that began replacing a computer's copies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepairEpoch<C> {
    /// Causal identity of the removal.
    pub commit: C,
    /// Removed computer, never a paper key.
    pub device: DeviceId,
    /// Its name immediately before removal.
    pub name: String,
    /// The accepted commit's author time.
    pub at: i64,
}

/// Projects effective removals using the same canonical fold as authority.
/// Skipped edits and paper keys do not initiate content repair. Laws are
/// in `tests/health_repair.rs`, with 1000 cases each.
pub fn repair_epochs<C: Ord + Clone>(history: &History<C>, tip: &C) -> Vec<RepairEpoch<C>> {
    let mut roster = history.roster.clone();
    let mut epochs = Vec::new();
    for commit in history.canonical_order(tip) {
        let Some(record) = history.commits.get(&commit) else {
            continue;
        };
        if record.change.is_empty() {
            continue;
        }
        let next = apply(&roster, &record.change).roster;
        for (device, previous) in &roster.devices {
            if previous.role != crate::roster::Role::Recovery
                && previous.annex_uuid.is_some()
                && !next.devices.contains_key(device)
            {
                epochs.push(RepairEpoch {
                    commit: commit.clone(),
                    device: device.clone(),
                    name: previous.name.clone(),
                    at: record.time,
                });
            }
        }
        roster = next;
    }
    epochs
}
use std::collections::BTreeSet;

/// A partition of observed versions, kept separate from headline policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// No copy is known.
    pub missing: u64,
    /// Released by a purge.
    pub erased: u64,
    /// Only this computer has the version.
    pub local: u64,
    /// Another computer knows it, without enough safe backup copies.
    pub published: u64,
    /// Enough copies were seen but their check is no longer recent.
    pub uncertain: u64,
    /// Enough distinct, recently checked backup copies.
    pub protected: u64,
}

impl Counts {
    /// The contribution of one version; uncertainty is preserved.
    pub fn one<D>(state: &State<D>) -> Self {
        let mut counts = Self::default();
        match state {
            State::Missing(Missing::ContentMissing) => counts.missing = 1,
            State::Missing(Missing::Erased) => counts.erased = 1,
            State::Local(_) => counts.local = 1,
            State::Published(_) => counts.published = 1,
            State::Uncertain(_) => counts.uncertain = 1,
            State::Protected(_) => counts.protected = 1,
        }
        counts
    }

    /// Combines disjoint inventories without overflow or loss of category.
    pub fn combine(self, other: Self) -> Self {
        Self {
            missing: self.missing.saturating_add(other.missing),
            erased: self.erased.saturating_add(other.erased),
            local: self.local.saturating_add(other.local),
            published: self.published.saturating_add(other.published),
            uncertain: self.uncertain.saturating_add(other.uncertain),
            protected: self.protected.saturating_add(other.protected),
        }
    }

    /// The number observed, saturating if an inventory cannot fit in u64.
    pub fn total(self) -> u64 {
        [
            self.missing,
            self.erased,
            self.local,
            self.published,
            self.uncertain,
            self.protected,
        ]
        .into_iter()
        .fold(0, u64::saturating_add)
    }
}

/// Current files: pending paths take precedence over their previous version.
/// The caller supplies each observed path once, as a catalogue does.
pub fn current<'a, P: Ord + 'a, D: 'a>(
    observed: impl IntoIterator<Item = (&'a P, &'a State<D>)>,
    pending: &BTreeSet<P>,
) -> Counts {
    observed
        .into_iter()
        .filter(|(path, _)| !pending.contains(*path))
        .map(|(_, state)| Counts::one(state))
        .fold(
            Counts {
                local: pending.len().try_into().unwrap_or(u64::MAX),
                ..Counts::default()
            },
            Counts::combine,
        )
}
