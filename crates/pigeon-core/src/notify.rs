//! When to notify (10 §10.4, ADR 0029): an algebra over what this person
//! must hear about. A [`Situation`] says, for each [`Kind`], whether
//! something of that kind is there and a fingerprint of what it is; a
//! [`Memory`] says what was last notified, and when. [`step`] gives the
//! notifications to show now and the new memory.
//!
//! Laws (`tests/notify.rs`): nothing when notifications are off, nothing
//! for a kind that is absent (never for success), at most one per kind and
//! step (grouped), two of one kind at least a day apart (the rate), never
//! the same fingerprint twice in a row (repeated only if the situation
//! changed), and a changed situation notified once the day has passed.

use crate::time::{Span, Time};

/// What a notification is about (10 §10.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// An item became *Needs you* for this person (06 §6.5).
    NeedsYou,
    /// A version of this device stays *published* more than 24 h.
    NotSafeLong,
    /// A suggestion or a conflict waits for this person.
    Waiting,
}

/// Every kind, in the order they are shown.
pub const KINDS: [Kind; 3] = [Kind::NeedsYou, Kind::NotSafeLong, Kind::Waiting];

/// The rate: one notification per kind and day at most.
pub const RATE: Span = Span(24 * 3600);

/// Now, for each kind: absent, or present with a fingerprint of its items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Situation {
    /// [`Kind::NeedsYou`].
    pub needs_you: Option<u64>,
    /// [`Kind::NotSafeLong`].
    pub not_safe_long: Option<u64>,
    /// [`Kind::Waiting`].
    pub waiting: Option<u64>,
}

impl Situation {
    /// The fingerprint of `kind`, if it is present.
    pub fn get(&self, kind: Kind) -> Option<u64> {
        match kind {
            Kind::NeedsYou => self.needs_you,
            Kind::NotSafeLong => self.not_safe_long,
            Kind::Waiting => self.waiting,
        }
    }
}

/// The last notification of one kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Last {
    /// When it was shown; `None` if never.
    pub at: Option<Time>,
    /// What it was about, while that is still the situation; `None` once
    /// the kind was absent since.
    pub fingerprint: Option<u64>,
}

/// What was notified, per kind in [`KINDS`]' order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory(pub [Last; 3]);

impl Memory {
    /// The last notification of `kind`.
    pub fn last(&self, kind: Kind) -> Last {
        self.0[slot(kind)]
    }
}

fn slot(kind: Kind) -> usize {
    match kind {
        Kind::NeedsYou => 0,
        Kind::NotSafeLong => 1,
        Kind::Waiting => 2,
    }
}

/// The notifications to show at `now` for `situation`, and the memory
/// after them; `enabled` is the `notifications` setting and a desktop.
pub fn step(
    memory: Memory,
    situation: &Situation,
    now: Time,
    enabled: bool,
) -> (Memory, Vec<Kind>) {
    let mut next = memory;
    let mut shown = Vec::new();
    for kind in KINDS {
        let last = memory.last(kind);
        let entry = &mut next.0[slot(kind)];
        match situation.get(kind) {
            None => entry.fingerprint = None,
            Some(fp) => {
                let changed = last.fingerprint != Some(fp);
                let rested = last.at.is_none_or(|at| now.since(at) >= RATE);
                if enabled && changed && rested {
                    shown.push(kind);
                    *entry = Last {
                        at: Some(now),
                        fingerprint: Some(fp),
                    };
                }
            }
        }
    }
    (next, shown)
}
