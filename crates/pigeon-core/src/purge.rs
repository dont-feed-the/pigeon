//! Reference-aware purge (08 §8.6): a decision covers only births it saw.
//! Release is recomputed from active decisions, all known references,
//! current keys and every active custodian's immutable seal. It is never
//! git-annex metadata. Seal histories must be merged before any drop.
//! Laws in `tests/purge.rs` cover waiting, current/shared/new references,
//! missing seals, unseen sealer histories, held proposals and cancellation.

use std::collections::{BTreeMap, BTreeSet};

/// A key's birth: this commit has it at path, and no parent does.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reference<C, K> {
    /// The commit introducing this reference.
    pub birth: C,
    /// Its exact portable path.
    pub path: String,
    /// The retained content key.
    pub key: K,
}

/// The verified scope of one active signed decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision<C, K> {
    /// Accepted history the decider saw.
    pub base: C,
    /// Include accepted history. False scopes only new births of the named
    /// proposal beyond its accepted base, preserving identical older bytes.
    pub history: bool,
    /// Exact path/subtree scopes; an empty scope means the group's root.
    pub paths: BTreeSet<String>,
    /// Empty means all keys in scope.
    pub keys: BTreeSet<K>,
    /// Explicitly named retained suggestion tips outside accepted history.
    pub proposals: BTreeSet<C>,
}

impl<C: Ord, K: Ord> Decision<C, K> {
    /// Whether this reference existed in a history explicitly seen by the
    /// decider. A later copy/restore has a new birth, even for the same key.
    pub fn covers(&self, reference: &Reference<C, K>, contains: &impl Fn(&C, &C) -> bool) -> bool {
        self.paths
            .iter()
            .any(|path| crate::restore::under(&reference.path, path))
            && (self.keys.is_empty() || self.keys.contains(&reference.key))
            && (self.history || !contains(&self.base, &reference.birth))
            && (contains(&self.base, &reference.birth)
                || self
                    .proposals
                    .iter()
                    .any(|tip| contains(tip, &reference.birth)))
    }
}

/// One custodian's immutable signed seal of a decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seal<C, K> {
    /// Its accepted head when it sealed.
    pub main: C,
    /// Covered keys held for an uncovered reference outside that main.
    pub held: BTreeSet<K>,
}

/// A fully verified, replicated completion witness, introduced only after
/// this accepted head contained every original custodian's signed seal.
/// Membership admission must observe this witness before changing that
/// decision's custodian set; it closes an epoch, never a content key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Epoch<C, D> {
    /// The accepted head containing all original seal histories.
    pub main: C,
    /// Exactly its custodian set; the verifier requires a nonempty set.
    pub custodians: BTreeSet<D>,
}

/// A verified reading; decision ids are distinct from file names and must
/// change when a cancelled decision is explicitly introduced again.
pub struct View<'a, C, K, D> {
    /// This device's accepted head.
    pub main: &'a C,
    /// Every known retained birth, including archived and proposal history.
    pub references: &'a BTreeSet<Reference<C, K>>,
    /// Keys in the accepted current tree, which never release.
    pub current: &'a BTreeSet<K>,
    /// Only decisions still present in accepted main.
    pub decisions: &'a BTreeMap<C, Decision<C, K>>,
    /// Every current custodian, excluding dead/removed identities.
    pub custodians: &'a BTreeSet<D>,
    /// Admitted seals, indexed by (decision id, signer).
    pub seals: &'a BTreeMap<(C, D), Seal<C, K>>,
    /// Verified completion epochs in this accepted history. Later births
    /// and cancellations still control coverage exactly as before.
    pub epochs: &'a BTreeMap<C, Epoch<C, D>>,
}

impl<C: Ord + Clone, K: Ord + Clone, D: Ord + Clone> View<'_, C, K, D> {
    /// Released keys. An empty custodian roster waits: otherwise the
    /// universal seal check would release before anybody waited at all.
    pub fn released(&self, contains: impl Fn(&C, &C) -> bool) -> BTreeSet<K> {
        if self.custodians.is_empty() && self.epochs.is_empty() {
            return BTreeSet::new();
        }
        let mut by_key: BTreeMap<&K, Vec<&Reference<C, K>>> = BTreeMap::new();
        for reference in self.references {
            by_key.entry(&reference.key).or_default().push(reference);
        }
        by_key
            .into_iter()
            .filter_map(|(key, references)| {
                if self.current.contains(key) {
                    return None;
                }
                let ready: Vec<_> = self
                    .decisions
                    .iter()
                    .filter(|(id, _)| {
                        let custodians = match self.epochs.get(*id) {
                            Some(epoch)
                                if !epoch.custodians.is_empty()
                                    && contains(self.main, &epoch.main) =>
                            {
                                &epoch.custodians
                            }
                            Some(_) => return false,
                            None => self.custodians,
                        };
                        !custodians.is_empty()
                            && custodians.iter().all(|device| {
                                self.seals
                                    .get(&((*id).clone(), device.clone()))
                                    .is_some_and(|seal| {
                                        !seal.held.contains(key)
                                            && contains(self.main, &seal.main)
                                            && self.epochs.get(*id).is_none_or(|epoch| {
                                                contains(&epoch.main, &seal.main)
                                            })
                                    })
                            })
                    })
                    .map(|(_, decision)| decision)
                    .collect();
                references
                    .iter()
                    .all(|reference| {
                        ready
                            .iter()
                            .any(|decision| decision.covers(reference, &contains))
                    })
                    .then(|| key.clone())
            })
            .collect()
    }

    /// Holds recorded at sealing: an uncovered reference outside accepted
    /// main survives even when that proposal has not reached other devices.
    pub fn held(
        &self,
        decision: &Decision<C, K>,
        contains: impl Fn(&C, &C) -> bool,
    ) -> BTreeSet<K> {
        let covered: BTreeSet<_> = self
            .references
            .iter()
            .filter(|r| decision.covers(r, &contains))
            .map(|r| &r.key)
            .collect();
        self.references
            .iter()
            .filter(|r| {
                covered.contains(&r.key)
                    && !contains(self.main, &r.birth)
                    && !self.decisions.values().any(|d| d.covers(r, &contains))
            })
            .map(|r| r.key.clone())
            .collect()
    }
}

/// P4: the recorded retention end cannot shorten the decision's configured
/// period, including timestamps close to the signed integer boundaries.
pub fn valid_end(recorded_at: i64, retention_seconds: u64, end: i64) -> bool {
    i128::from(end) >= i128::from(recorded_at) + i128::from(retention_seconds)
}

/// A wall-clock jump alone cannot seal: both the declared UTC end and a
/// durable monotonic elapsed period since local receipt must have passed.
pub fn can_seal(now: i64, end: i64, elapsed_seconds: u64, retention_seconds: u64) -> bool {
    now >= end && elapsed_seconds >= retention_seconds
}
