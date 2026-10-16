//! Bounded diagnostic milestone observations (12 §12.2). Each version's
//! acceptance and protection are measured once per ledger lifetime.
//! Clock skew is explicit; a future capture never becomes a zero lag.

use std::collections::BTreeMap;

/// An independently observed milestone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Milestone {
    /// The captured version reached accepted history on this computer.
    Accepted,
    /// This computer first observed the version protected.
    Protected,
}

/// A wall-clock observation, never used for a safety decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    /// Elapsed whole seconds, saturating at the largest representable value.
    Seconds(u64),
    /// The capture's clock is ahead of the observer's.
    ClockSkew,
}

/// Fixed-capacity duplicate suppression. New identities beyond capacity
/// are omitted; old identities are never evicted and measured twice.
#[derive(Clone, Debug, Default)]
pub struct Ledger<K> {
    seen: BTreeMap<K, u8>,
}

impl<K: Ord> Ledger<K> {
    /// Observe a milestone once. `None` means repeated or beyond capacity.
    /// Laws in `tests/latency.rs` run 1000 cases each.
    pub fn observe(
        &mut self,
        capacity: usize,
        id: K,
        milestone: Milestone,
        captured: i64,
        now: i64,
    ) -> Option<Sample> {
        if !self.seen.contains_key(&id) && self.seen.len() >= capacity {
            return None;
        }
        let bit = match milestone {
            Milestone::Accepted => 1,
            Milestone::Protected => 2,
        };
        let seen = self.seen.entry(id).or_default();
        if *seen & bit != 0 {
            return None;
        }
        *seen |= bit;
        Some(if now < captured {
            Sample::ClockSkew
        } else {
            Sample::Seconds(
                u64::try_from(i128::from(now) - i128::from(captured)).unwrap_or(u64::MAX),
            )
        })
    }

    /// The number of tracked identities, independent of their byte sizes.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Whether there are no tracked identities.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}
