//! The order of the one work queue (15 §15.2).
//!
//! Classes run highest first: capture, history, protect, download, scrub,
//! rebalance. Inside a class the order of 15 §15.2 applies, and every tie is
//! broken by the remaining fields, so equal items are identical.
//!
//! Laws (`tests/queue.rs`): [`compare`] is a total order; a higher class
//! always goes first; sorting gives one sequence whatever the arrival order;
//! a protect item held by fewer custodians never waits behind one held by
//! more (spec 08 §8.9).

use std::cmp::Ordering;

use crate::time::Time;

/// A kept folder's priority on this device (11 §11.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// Before the others.
    High,
    /// The default.
    Normal,
    /// After the others.
    Low,
}

/// One item of work.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Work {
    /// Hash and commit what a person saved: oldest change first.
    Capture {
        /// When the change was seen.
        changed: Time,
        /// The path.
        path: String,
    },
    /// Fetch, validate and merge a peer's history: most recent notice first.
    History {
        /// When the notice arrived.
        notice: Time,
        /// The peer.
        peer: String,
    },
    /// Bring a version toward *protected*, or repair it.
    Protect {
        /// The git-annex key.
        key: String,
        /// Custodians holding a custody copy.
        custody_copies: u32,
        /// Laptops holding a copy: they order, they do not protect.
        laptop_copies: u32,
        /// A holder is failing or leaving.
        endangered: bool,
        /// When the version was introduced: newest first.
        introduced: Time,
        /// Its size: smallest first.
        size: u64,
    },
    /// Content of a kept folder, and its install.
    Download {
        /// The git-annex key.
        key: String,
        /// The folder's priority.
        priority: Priority,
        /// When it changed: most recent first.
        changed: Time,
    },
    /// One `fsck --incremental` slice, in git-annex's order.
    Scrub {
        /// The slice's sequence number.
        slice: u64,
    },
    /// A verified drop of a key a custodian no longer wants.
    Rebalance {
        /// The git-annex key.
        key: String,
    },
}

impl Work {
    /// The class, 1 (capture) to 6 (rebalance).
    pub fn class(&self) -> u8 {
        match self {
            Work::Capture { .. } => 1,
            Work::History { .. } => 2,
            Work::Protect { .. } => 3,
            Work::Download { .. } => 4,
            Work::Scrub { .. } => 5,
            Work::Rebalance { .. } => 6,
        }
    }
}

/// The queue order: `Less` means `a` runs first.
pub fn compare(a: &Work, b: &Work) -> Ordering {
    use Work::*;
    match (a, b) {
        (
            Capture {
                changed: t1,
                path: p1,
            },
            Capture {
                changed: t2,
                path: p2,
            },
        ) => t1.cmp(t2).then_with(|| p1.cmp(p2)),
        (
            History {
                notice: t1,
                peer: p1,
            },
            History {
                notice: t2,
                peer: p2,
            },
        ) => t2.cmp(t1).then_with(|| p1.cmp(p2)),
        (
            Protect {
                key: k1,
                custody_copies: d1,
                laptop_copies: l1,
                endangered: e1,
                introduced: i1,
                size: s1,
            },
            Protect {
                key: k2,
                custody_copies: d2,
                laptop_copies: l2,
                endangered: e2,
                introduced: i2,
                size: s2,
            },
        ) => d1
            .cmp(d2)
            .then_with(|| l1.cmp(l2))
            .then_with(|| e2.cmp(e1))
            .then_with(|| i2.cmp(i1))
            .then_with(|| s1.cmp(s2))
            .then_with(|| k1.cmp(k2)),
        (
            Download {
                key: k1,
                priority: p1,
                changed: c1,
            },
            Download {
                key: k2,
                priority: p2,
                changed: c2,
            },
        ) => p1.cmp(p2).then_with(|| c2.cmp(c1)).then_with(|| k1.cmp(k2)),
        (Scrub { slice: s1 }, Scrub { slice: s2 }) => s1.cmp(s2),
        (Rebalance { key: k1 }, Rebalance { key: k2 }) => k1.cmp(k2),
        _ => a.class().cmp(&b.class()),
    }
}

/// Sort `items` into queue order.
pub fn order(items: &mut [Work]) {
    items.sort_by(compare);
}
