//! Version states: local, published, protected, uncertain, missing (06 §6.2).
//!
//! A version's state is a function of its key's observations, the spread of
//! the commit that names it and the present time. H is the set of custodians
//! that count for it, each counted once by its device (ADR 0046, 13 §13.13).
//!
//! Laws (`tests/state.rs`): more presence observations never lower the state
//! (its [`rank`]); a second entry for one device never changes it; the order
//! of the custodians does not matter; a version is protected exactly when K
//! distinct custodians count and one was heard within `stale_after`; a
//! version only this device holds is *local* whatever the network says.

use std::collections::{BTreeMap, BTreeSet};

use crate::time::{Span, Time};

/// The thresholds of 06 §6.7 and the group's custody settings (04 §4.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// K: custody copies, on distinct custodians.
    pub k: usize,
    /// A custodian silent for longer stops counting (24 hours).
    pub offline_after: Span,
    /// No custodian heard of for longer makes a version *uncertain* (1 hour).
    pub stale_after: Span,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            k: 2,
            offline_after: Span::hours(24),
            stale_after: Span::hours(1),
        }
    }
}

/// One custodian of the roster, as this device observes it for one version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Custodian<D> {
    /// The custodian's device.
    pub device: D,
    /// The latest news from it (`heard`).
    pub heard: Option<Time>,
    /// `holds(x, k)`: its location log says it holds the key.
    pub holds: bool,
    /// `has_commit(x, c)`: it has the commit that names the version.
    pub has_commit: bool,
    /// Its report says it is checking its repository.
    pub checking: bool,
    /// Its report says its repository is lost.
    pub repository_lost: bool,
}

/// What this device knows of one version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version<D> {
    /// Every custodian of the roster.
    pub custodians: Vec<Custodian<D>>,
    /// Some other device has the commit.
    pub other_device_has_commit: bool,
    /// This device holds the content.
    pub this_device_holds: bool,
    /// A purge released the content (08 §8.6).
    pub erased: bool,
}

/// Why a version is not protected (06 §6.4), for the states that need one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason<D> {
    /// Copies are on the way, or nothing more specific is known.
    Sending,
    /// Custodians holding it have been silent too long; the latest news.
    CustodianOffline(BTreeSet<D>, Option<Time>),
    /// A custodian holding it reports its repository lost.
    RepositoryLost(D),
    /// The roster has fewer custodians than K.
    TooFewCustodians {
        /// Custodians in the roster.
        have: usize,
        /// The group's K.
        k: usize,
    },
    /// No custodian heard of within `stale_after`; the latest news.
    GroupUnreachable(Option<Time>),
}

/// Why no device is known to hold a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    /// No device is known to hold the content.
    ContentMissing,
    /// A purge released it.
    Erased,
}

/// The state of a version (06 §6.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State<D> {
    /// No device is known to hold it.
    Missing(Missing),
    /// Only this device holds it.
    Local(Reason<D>),
    /// Another device has the commit, or some custodian holds the key.
    Published(Reason<D>),
    /// It would be protected, but no custodian was heard within `stale_after`.
    Uncertain(Option<Time>),
    /// K distinct custodians hold it, observed at the given time.
    Protected(Time),
}

/// The order of states, for the monotonicity law: missing, local,
/// published, uncertain, protected.
pub fn rank<D>(state: &State<D>) -> u8 {
    match state {
        State::Missing(_) => 0,
        State::Local(_) => 1,
        State::Published(_) => 2,
        State::Uncertain(_) => 3,
        State::Protected(_) => 4,
    }
}

fn counts<D>(c: &Custodian<D>, s: &Settings, now: Time) -> bool {
    c.holds
        && c.has_commit
        && !c.checking
        && !c.repository_lost
        && c.heard.is_some_and(|h| h.within(s.offline_after, now))
}

/// The state of `version` at `now` under `settings`.
pub fn state<D: Ord + Clone>(version: &Version<D>, settings: &Settings, now: Time) -> State<D> {
    let k = settings.k.max(1);
    // H, each device once, with the latest news from it.
    let mut h: BTreeMap<&D, Option<Time>> = BTreeMap::new();
    for c in version
        .custodians
        .iter()
        .filter(|c| counts(c, settings, now))
    {
        let at = h.entry(&c.device).or_default();
        *at = (*at).max(c.heard);
    }
    let latest = version.custodians.iter().filter_map(|c| c.heard).max();
    let reachable = latest.is_some_and(|t| t.within(settings.stale_after, now));
    if h.len() >= k {
        if !reachable {
            return State::Uncertain(latest);
        }
        let mut heard: Vec<Time> = h.values().flatten().copied().collect();
        heard.sort_by(|a, b| b.cmp(a));
        return State::Protected(heard.get(k - 1).copied().unwrap_or_default());
    }
    let reason = reason(version, settings, now, reachable, latest);
    if version.other_device_has_commit || version.custodians.iter().any(|c| c.holds) {
        State::Published(reason)
    } else if version.this_device_holds {
        State::Local(reason)
    } else if version.erased {
        State::Missing(Missing::Erased)
    } else {
        State::Missing(Missing::ContentMissing)
    }
}

fn reason<D: Ord + Clone>(
    version: &Version<D>,
    settings: &Settings,
    now: Time,
    reachable: bool,
    latest: Option<Time>,
) -> Reason<D> {
    let k = settings.k.max(1);
    let named: Vec<&Custodian<D>> = version
        .custodians
        .iter()
        .filter(|c| c.holds && c.has_commit)
        .collect();
    if let Some(lost) = named
        .iter()
        .filter(|c| c.repository_lost)
        .map(|c| &c.device)
        .min()
    {
        return Reason::RepositoryLost(lost.clone());
    }
    let offline: Vec<&&Custodian<D>> = named
        .iter()
        .filter(|c| {
            !c.heard
                .is_some_and(|t| t.within(settings.offline_after, now))
        })
        .collect();
    if !offline.is_empty() {
        let since = offline.iter().filter_map(|c| c.heard).max();
        return Reason::CustodianOffline(offline.iter().map(|c| c.device.clone()).collect(), since);
    }
    let roster: BTreeSet<&D> = version.custodians.iter().map(|c| &c.device).collect();
    if roster.len() < k {
        return Reason::TooFewCustodians {
            have: roster.len(),
            k,
        };
    }
    if !reachable {
        return Reason::GroupUnreachable(latest);
    }
    Reason::Sending
}
