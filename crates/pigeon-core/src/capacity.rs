//! Capacity (08 §8.9): the reserve, the need in parts, and admission, which
//! decides what a device takes with the room it has.
//!
//! Admission plans the protect items a device wants and lacks, in queue
//! order (15 §15.2), with its room: free space above the reserve, less the
//! downloads in progress. An endangered key is admitted while it fits; any
//! other key only if the endangered keys not yet admitted still fit after
//! it, so an endangered key's room never goes to another (`Repair.tla`
//! `EndangeredFirst`; sizes generalise its unit keys, `Admission.tla`).
//!
//! Laws (`tests/capacity.rs`): admitted bytes never exceed the room; the
//! endangered keys admitted do not depend on the other keys; the plan does
//! not depend on arrival order and keeps queue order; every item is
//! admitted or deferred, once, and a deferred one says the bytes it misses;
//! `missing_bytes` more room admits everything, one byte less does not; the
//! parts of the need sum to its total, K times each distinct key once; the
//! capacity is one copy per custodian exactly when no limit is below one
//! copy, and those below are the ones warned about.

use std::collections::BTreeMap;

use crate::filter::Role;
use crate::queue::{Work, compare};

/// One gigabyte, as git-annex counts sizes (`annex.diskreserve`).
pub const GB: u64 = 1_000_000_000;

/// The `annex.diskreserve` of a role on a file system of `fs_bytes`:
/// max(1 GB, 1 %) on laptops, max(5 GB, 2 %) on custodians (08 §8.9).
pub fn reserve(role: Role, fs_bytes: u64) -> u64 {
    match role {
        Role::Laptop => GB.max(fs_bytes / 100),
        Role::Custodian => (5 * GB).max(fs_bytes / 50),
    }
}

/// The room for new content: `available` bytes less the reserve and the
/// downloads in progress, which git-annex counts too (`Repair.tla`
/// `Repair_inflight`).
pub fn room(available: u64, reserve: u64, in_progress: u64) -> u64 {
    available
        .saturating_sub(reserve)
        .saturating_sub(in_progress)
}

/// The size a git-annex key names (`SHA256E-s1234--…`), if it names one.
pub fn key_size(key: &str) -> Option<u64> {
    let fields = key.split_once("--").map_or(key, |(f, _)| f);
    fields
        .split('-')
        .skip(1)
        .find_map(|f| f.strip_prefix('s')?.parse().ok())
}

/// A protect item's size and whether it is endangered; other work takes no
/// room here.
fn sized(w: &Work) -> (u64, bool) {
    match w {
        Work::Protect {
            size, endangered, ..
        } => (*size, *endangered),
        _ => (0, false),
    }
}

/// An item admission kept out, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deferred {
    /// The item.
    pub work: Work,
    /// The room it would have needed at its turn beyond what was left.
    pub missing_bytes: u64,
    /// The room kept at its turn for endangered keys not yet admitted: more
    /// than zero only for a key that is not endangered.
    pub held_bytes: u64,
}

/// What a custodian admits (06 §6.3 `admits`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admits {
    /// Every key it wants, as room allows.
    All,
    /// Endangered keys only: it keeps its room for repair.
    Endangered,
}

/// The outcome of admission: what to transfer, in queue order, and what
/// waits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The room admission started with.
    pub room: u64,
    /// The items to transfer, in queue order.
    pub admitted: Vec<Work>,
    /// The items that wait, in queue order.
    pub deferred: Vec<Deferred>,
}

impl Plan {
    /// The bytes admitted.
    pub fn admitted_bytes(&self) -> u64 {
        self.admitted.iter().map(|w| sized(w).0).sum()
    }

    /// The room missing for every item to be admitted: their total size
    /// less the room, or zero.
    pub fn missing_bytes(&self) -> u64 {
        let deferred: u64 = self.deferred.iter().map(|d| sized(&d.work).0).sum();
        (self.admitted_bytes() + deferred).saturating_sub(self.room)
    }

    /// `Endangered` once a key that is not endangered waits so that
    /// endangered keys keep their room.
    pub fn admits(&self) -> Admits {
        if self.deferred.iter().any(|d| d.held_bytes > 0) {
            Admits::Endangered
        } else {
            Admits::All
        }
    }
}

/// Plans `items` with `room` bytes: in queue order, an endangered key is
/// admitted if it fits in what is left; another key if it fits with the
/// endangered keys not yet admitted. Items that are not protect work take no
/// room and are admitted.
pub fn admit(room: u64, items: &[Work]) -> Plan {
    let mut items = items.to_vec();
    items.sort_by(compare);
    let mut held: u64 = items
        .iter()
        .map(sized)
        .filter(|(_, e)| *e)
        .map(|(s, _)| s)
        .sum();
    let mut left = room;
    let mut plan = Plan {
        room,
        admitted: Vec::new(),
        deferred: Vec::new(),
    };
    for work in items {
        let (size, endangered) = sized(&work);
        let keep = if endangered { 0 } else { held };
        let need = size.saturating_add(keep);
        if need <= left {
            left -= size;
            if endangered {
                held -= size;
            }
            plan.admitted.push(work);
        } else {
            plan.deferred.push(Deferred {
                work,
                missing_bytes: need - left,
                held_bytes: keep,
            });
        }
    }
    plan
}

/// A part of the need (08 §8.9), in the order a key shared by several is
/// counted: once, in its first part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    /// The current versions of `main`.
    Current,
    /// Earlier versions not purged.
    History,
    /// Open, declined or withdrawn suggestions, until erased.
    Suggestions,
}

/// The bytes the custodians need together, in parts, each K × the size of
/// its distinct keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Need {
    /// Bytes per part.
    pub parts: BTreeMap<Part, u64>,
}

impl Need {
    /// The sum of the parts.
    pub fn total(&self) -> u64 {
        self.parts.values().sum()
    }
}

/// The need of `k` copies of `versions`, given as (key, size, part); a key
/// listed in several parts counts once, in its first.
pub fn need<'a>(k: u32, versions: impl IntoIterator<Item = (&'a str, u64, Part)>) -> Need {
    let mut first: BTreeMap<&str, (Part, u64)> = BTreeMap::new();
    for (key, size, part) in versions {
        let e = first.entry(key).or_insert((part, size));
        if part < e.0 {
            *e = (part, size);
        }
    }
    let mut need = Need::default();
    for (part, size) in first.into_values() {
        *need.parts.entry(part).or_default() += u64::from(k) * size;
    }
    need
}

/// The capacity of the custodians, given as (maxsize, bytes of one copy of
/// everything): the sum of min(maxsize, one copy), a missing maxsize
/// counting as one copy.
pub fn capacity(custodians: &[(Option<u64>, u64)]) -> u64 {
    custodians
        .iter()
        .map(|(max, all)| max.map_or(*all, |m| m.min(*all)))
        .sum()
}

/// The custodians, by their index in `custodians` given as (maxsize, bytes
/// of one copy of everything), whose storage limit is below one copy: the
/// warning of 08 §8.9. Exactly these make [`capacity`] fall short of one
/// copy per custodian.
pub fn below_one_copy(custodians: &[(Option<u64>, u64)]) -> Vec<usize> {
    custodians
        .iter()
        .enumerate()
        .filter(|(_, (max, all))| max.is_some_and(|m| m < *all))
        .map(|(i, _)| i)
        .collect()
}

/// Whether no custodian is spare: as many custodians as K. Each counts for
/// at most one copy, so losing any leaves versions unprotected until it is
/// replaced, however much disk they have (08 §8.9, information only).
pub fn no_spare(custodians: usize, k: u32) -> bool {
    u32::try_from(custodians).is_ok_and(|n| n == k)
}
