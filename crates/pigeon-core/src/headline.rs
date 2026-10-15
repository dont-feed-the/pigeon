//! The headline: every item reduced to one of five levels (06 §6.5).
//!
//! Levels are ordered Safe < Working < Not checked recently < Not safe yet <
//! Needs you, and the headline is their join, the maximum.
//!
//! Laws (`tests/headline.rs`): the join is associative, commutative and
//! idempotent, with Safe as identity and Needs you absorbing; the headline
//! of a union is the join of the headlines; adding an item never lowers it;
//! a version only this device holds is *Not safe yet* after the grace
//! period, even when the group is unreachable.

use crate::state::{Missing, Settings, State};
use crate::time::{Span, Time};

/// The oldest safety observation behind a group headline (06 §6.5).
/// A recent conversation with a laptop cannot refresh a custodian's proof.
/// Local and missing versions carry no safety observation; their levels
/// separately keep the headline from saying that everything is safe.
pub fn checked_at<'a, D: 'a>(states: impl IntoIterator<Item = &'a State<D>>) -> Option<Time> {
    states
        .into_iter()
        .filter_map(|state| match state {
            State::Protected(at) | State::Uncertain(Some(at)) => Some(*at),
            _ => None,
        })
        .min()
}

/// A headline level, lowest first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Every version is protected, observed within `stale_after`.
    #[default]
    Safe,
    /// Something is captured, sent or received, younger than the grace period.
    Working,
    /// pigeon cannot confirm: no custodian heard of within `stale_after`.
    NotCheckedRecently,
    /// Some version is *local* or *published* beyond the grace period.
    NotSafeYet,
    /// Only a person can fix an item.
    NeedsYou,
}

impl Level {
    /// The join of two levels: the higher one.
    pub fn join(self, other: Level) -> Level {
        self.max(other)
    }
}

/// The headline of a collection of item levels: their join, Safe if none.
pub fn headline(levels: impl IntoIterator<Item = Level>) -> Level {
    levels.into_iter().fold(Level::Safe, Level::join)
}

/// The causes of *Needs you* (06 §6.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NeedsYou {
    /// A disk is full.
    DiskFull,
    /// A backup machine not heard of for more than 7 days.
    MachineMissing,
    /// A held mass deletion.
    HeldDeletion,
    /// A held non-portable name.
    HeldName,
    /// A conflict waits for this person (ADR 0034).
    Conflict,
    /// A suggestion waits for this person's decision for more than 3 days.
    Suggestion,
    /// This computer was removed from the group.
    Removed,
    /// Custody capacity is short (ADR 0003, spec 08 §8.9).
    CapacityShort,
    /// A backup machine's repository is lost (ADR 0046).
    RepositoryLost,
    /// Erased content came back (spec 08 §8.6).
    ErasedCameBack,
}

/// The level of one version, `age` after it was introduced.
pub fn version_level<D>(
    state: &State<D>,
    age: Span,
    grace: Span,
    settings: &Settings,
    now: Time,
) -> Level {
    match state {
        State::Protected(at) if at.within(settings.stale_after, now) => Level::Safe,
        State::Protected(_) | State::Uncertain(_) => Level::NotCheckedRecently,
        State::Local(_) | State::Published(_) | State::Missing(Missing::ContentMissing) => {
            if age > grace {
                Level::NotSafeYet
            } else {
                Level::Working
            }
        }
        State::Missing(Missing::Erased) => Level::Safe,
    }
}
