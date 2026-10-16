//! One-use local consent for diagnostic disclosure (12 §12.2). A grant
//! names its complete recipient/scope key, has a monotonic deadline, and
//! is consumed only by a matching recipient before that deadline.
//!
//! Laws: no use after expiry; no use by a different scope or recipient;
//! a successful use cannot repeat; a refused use does not spend consent.
//! `tests/consent.rs` checks each law with at least 1000 cases.

/// One explicit local permission; absence of a grant permits nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant<K> {
    recipient: K,
    until: u64,
    spent: bool,
}

impl<K: PartialEq> Grant<K> {
    /// An explicit permission, in the caller's monotonic time units.
    pub fn new(recipient: K, now: u64, lifetime: u64) -> Self {
        Self {
            recipient,
            until: now.saturating_add(lifetime),
            spent: false,
        }
    }

    /// Consumes consent exactly once if recipient and deadline match.
    pub fn take(&mut self, recipient: &K, now: u64) -> bool {
        if !self.spent && now < self.until && recipient == &self.recipient {
            self.spent = true;
            true
        } else {
            false
        }
    }

    /// Whether the grant remains unspent and unexpired.
    pub fn live(&self, now: u64) -> bool {
        !self.spent && now < self.until
    }
}
