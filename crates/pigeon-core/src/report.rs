//! Status reports (06 §6.3): which report of each device counts.
//!
//! Each device publishes a signed report of what it has and holds, and
//! every peer relays the reports it fetched. A report counts only if it is
//! signed by the roster device it names, in a known format with known
//! fields, not from the future, and later than the last one that counted
//! for that device ([`judge`]). What a device knows of the others is then
//! [`Latest`]: the latest report of each device, by `at` and then by
//! commit ID, so that reports met in any order, directly or relayed, give
//! the same knowledge.
//!
//! Laws (`tests/report.rs`): `latest_is_a_semilattice`,
//! `the_order_of_arrival_does_not_matter`,
//! `an_older_report_never_replaces_a_newer`,
//! `only_a_report_signed_by_its_roster_device_counts`,
//! `relaying_what_counted_changes_nothing`.

use std::collections::BTreeMap;

use crate::roster::{DeviceId, Roster};
use crate::time::{Span, Time};

/// How far ahead of this device's clock a report may be and still count.
pub const SKEW: Span = Span::minutes(10);

/// One report, as its envelope tells it: ordered by `at`, then by commit.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seen {
    /// When what it says was true.
    pub at: Time,
    /// The commit that carries it.
    pub commit: String,
}

/// A fetched report, before it counts.
#[derive(Clone, Debug)]
pub struct Offer<'a> {
    /// The device whose valid signature it carries, if any.
    pub signer: Option<&'a DeviceId>,
    /// The device it names.
    pub device: &'a DeviceId,
    /// Whether its format and every field are known to this release.
    pub known: bool,
    /// Its time and commit.
    pub seen: Seen,
}

/// Why a report does not count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ignored {
    /// It carries no valid signature.
    Unsigned,
    /// It is signed by another device than the one it names.
    NotItsSigner,
    /// The device it names is not in the roster.
    NotInRoster,
    /// Its format or a field is unknown (ADR 0048).
    Unknown,
    /// Its `at` is more than [`SKEW`] ahead of `now`.
    FromTheFuture,
    /// It is not later than the last report that counted.
    NotNewer,
}

/// Whether `offer` counts, at `now`, after `last`.
pub fn judge(
    roster: &Roster,
    offer: &Offer<'_>,
    last: Option<&Seen>,
    now: Time,
) -> Result<(), Ignored> {
    let Some(signer) = offer.signer else {
        return Err(Ignored::Unsigned);
    };
    if signer != offer.device {
        return Err(Ignored::NotItsSigner);
    }
    if !roster.devices.contains_key(offer.device) {
        return Err(Ignored::NotInRoster);
    }
    if !offer.known {
        return Err(Ignored::Unknown);
    }
    if offer.seen.at.0 > now.0.saturating_add(SKEW.0) {
        return Err(Ignored::FromTheFuture);
    }
    if last.is_some_and(|l| offer.seen <= *l) {
        return Err(Ignored::NotNewer);
    }
    Ok(())
}

/// The latest report that counted, by device.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Latest(pub BTreeMap<DeviceId, Seen>);

impl Latest {
    /// The latest report of `device`.
    pub fn get(&self, device: &DeviceId) -> Option<&Seen> {
        self.0.get(device)
    }

    /// Keeps `seen` for `device` if it is later than the one kept; answers
    /// whether it was.
    pub fn offer(&mut self, device: DeviceId, seen: Seen) -> bool {
        match self.0.get(&device) {
            Some(kept) if *kept >= seen => false,
            _ => {
                self.0.insert(device, seen);
                true
            }
        }
    }

    /// The latest of both, by device.
    #[must_use]
    pub fn join(&self, other: &Latest) -> Latest {
        let mut out = self.clone();
        for (d, s) in &other.0 {
            out.offer(d.clone(), s.clone());
        }
        out
    }
}
