//! Incremental scrub scheduling (08 §8.8, 15 §15.1). A local wall-clock
//! window permits work, never proves elapsed retention. A pass acknowledges
//! only keys in its pending snapshot; failed checks stay in its findings.
//! Laws check exact window width, idempotent progress and complete coverage.

use std::collections::BTreeSet;

/// A half-open local-time window, including those crossing midnight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    start: u16,
    end: u16,
}

impl Window {
    /// Strict `HH:MM-HH:MM`; 24:00 is permitted only as the end.
    /// Equal times disable scrubbing; 00:00-24:00 permits the whole day.
    pub fn parse(text: &str) -> Option<Self> {
        fn minute(text: &str, end: bool) -> Option<u16> {
            if text.len() != 5 || text.as_bytes()[2] != b':' {
                return None;
            }
            let (hours, minutes) = text.split_once(':')?;
            if !hours
                .bytes()
                .chain(minutes.bytes())
                .all(|b| b.is_ascii_digit())
            {
                return None;
            }
            let hours: u16 = hours.parse().ok()?;
            let minutes: u16 = minutes.parse().ok()?;
            if minutes >= 60 || hours > 24 || (hours == 24 && (!end || minutes != 0)) {
                return None;
            }
            Some(hours * 60 + minutes)
        }
        let (start, end) = text.split_once('-')?;
        Some(Self {
            start: minute(start, false)?,
            end: minute(end, true)?,
        })
    }

    /// Whether a minute of the local day is within this window.
    pub fn contains(self, minute: u16) -> bool {
        if minute >= 1440 {
            return false;
        }
        if self.start <= self.end {
            self.start <= minute && minute < self.end
        } else {
            self.start <= minute || minute < self.end
        }
    }
}

/// A snapshot's remaining checks and immutable findings. A failed hash
/// counts as checked, with a finding; a command/I/O failure is not an ack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass<K> {
    /// Keys not yet successfully examined (not necessarily healthy).
    pub pending: BTreeSet<K>,
    /// Keys found bad during this pass, without duplicate counting.
    pub bad: BTreeSet<K>,
}

impl<K: Ord> Pass<K> {
    /// Begin one complete inventory snapshot.
    pub fn new(keys: BTreeSet<K>) -> Self {
        Self {
            pending: keys,
            bad: BTreeSet::new(),
        }
    }

    /// Acknowledging an unrequested or repeated key cannot change progress.
    pub fn checked(&mut self, key: K, bad: bool) {
        if self.pending.remove(&key) && bad {
            self.bad.insert(key);
        }
    }

    /// Completion requires every snapshot key to have an acknowledgement.
    pub fn complete(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Start a new full pass only when the last pass is at least this old.
/// A backward clock jump waits; subtraction cannot wrap at timestamp ends.
pub fn due(now: i64, last: Option<i64>, period_seconds: u64) -> bool {
    last.is_none_or(|last| i128::from(now) - i128::from(last) >= i128::from(period_seconds))
}
