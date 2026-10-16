//! Group-bound invitation redemption (09 §9.6). The issuer records the
//! group and first authenticated device. A retry by that device resumes
//! its welcome; it never grants authority to another device or group.
//!
//! Laws (`tests/invitation.rs`, 1000 cases each): another group and an
//! unbound legacy record refuse; unused scoped records allow a first use;
//! a spent record never grants a first use; only the same device resumes.

/// What the issuer may do after authenticating the proof and checking
/// expiry. A resume still needs current membership or a pending first commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Redemption {
    /// Persist the bound identity before signing its first addition.
    First,
    /// Return the existing welcome or finish its interrupted first commit.
    Resume,
    /// This record grants nothing to this group/device.
    Refused,
}

/// Classifies a record without changing authority or trusting a claimed ID.
pub fn redeem<G: PartialEq + ?Sized, K: PartialEq + ?Sized>(
    issued_for: Option<&G>,
    group: &G,
    used_by: Option<&K>,
    peer: &K,
) -> Redemption {
    if issued_for != Some(group) {
        return Redemption::Refused;
    }
    match used_by {
        None => Redemption::First,
        Some(used) if used == peer => Redemption::Resume,
        Some(_) => Redemption::Refused,
    }
}
