//! Stable and hot-file scheduling (ADR 0037, 15 §15.5). The adapter
//! supplies complete stat fingerprints, monotonic milliseconds and each
//! successful capture's version. No filesystem or wall clock is consulted.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::resources::quiet_ms;

const HOUR: u64 = 3_600_000;
const HOT_INTERVAL: u64 = 600_000;

/// Whether this observed file has settled enough to capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// Two equal observations span the quiet period and any hot-path delay.
    Ready,
    /// Observe again after at least this many milliseconds.
    Wait(u64),
}

#[derive(Debug)]
struct History<S> {
    stamp: S,
    stable_since: u64,
    seen: u64,
    changed: u64,
    last_capture: Option<u64>,
    last_version: Option<String>,
    captures: VecDeque<u64>,
    hot: bool,
    observations: u8,
}

/// Per-path scheduling history. A stat change restarts settling; only a
/// successful distinct capture increments the hot counter. Paths retain at
/// most seven capture timestamps and one fingerprint, independent of bytes.
#[derive(Debug)]
pub struct Schedule<S> {
    paths: BTreeMap<String, History<S>>,
    now: u64,
}

impl<S> Default for Schedule<S> {
    fn default() -> Self {
        Self {
            paths: BTreeMap::new(),
            now: 0,
        }
    }
}

impl<S: Eq> Schedule<S> {
    /// Observe a candidate. A new or changed fingerprint cannot be ready
    /// until a second equal observation spans its size-dependent quiet time.
    /// Backwards input cannot move the schedule's clock backwards.
    pub fn observe(&mut self, path: &str, stamp: S, size: u64, now: u64) -> Readiness {
        self.now = self.now.max(now);
        let now = self.now;
        let state = match self.paths.entry(path.to_owned()) {
            std::collections::btree_map::Entry::Vacant(entry) => entry.insert(History {
                stamp,
                stable_since: now,
                seen: now,
                changed: now,
                last_capture: None,
                last_version: None,
                captures: VecDeque::new(),
                hot: false,
                observations: 1,
            }),
            std::collections::btree_map::Entry::Occupied(entry) => {
                let state = entry.into_mut();
                Self::cool(state, now);
                if state.stamp == stamp {
                    state.observations = state.observations.saturating_add(1).min(2);
                } else {
                    state.stamp = stamp;
                    state.stable_since = now;
                    state.changed = now;
                    state.observations = 1;
                }
                state
            }
        };
        state.seen = now;
        Self::readiness(state, size, now)
    }

    fn readiness(state: &mut History<S>, size: u64, now: u64) -> Readiness {
        Self::cool(state, now);
        let settled = quiet_ms(size).saturating_sub(now.saturating_sub(state.stable_since));
        let limited = if state.hot {
            state
                .last_capture
                .map_or(0, |at| HOT_INTERVAL.saturating_sub(now.saturating_sub(at)))
        } else {
            0
        };
        let wait = settled.max(limited);
        if wait == 0 && state.observations >= 2 {
            Readiness::Ready
        } else {
            Readiness::Wait(wait.max(1))
        }
    }

    fn cool(state: &mut History<S>, now: u64) {
        if state.hot && now.saturating_sub(state.changed) >= HOUR {
            state.hot = false;
            state.captures.clear();
        }
    }

    /// Record a successful version once. Unobserved paths and repeated
    /// notifications of the same version do not count as new captures.
    pub fn captured(&mut self, path: &str, version: &str, now: u64) -> bool {
        self.now = self.now.max(now);
        let Some(state) = self.paths.get_mut(path) else {
            return false;
        };
        if state.last_version.as_deref() == Some(version) {
            return false;
        }
        state.last_version = Some(version.to_owned());
        state.last_capture = Some(self.now);
        state.seen = self.now;
        while state
            .captures
            .front()
            .is_some_and(|at| self.now.saturating_sub(*at) >= HOUR)
        {
            state.captures.pop_front();
        }
        state.captures.push_back(self.now);
        state.hot |= state.captures.len() > 6;
        while state.captures.len() > 7 {
            state.captures.pop_front();
        }
        true
    }

    /// Forget inactive paths only after a whole quiet hour. Reappearing
    /// paths inside that hour retain their actual capture history.
    pub fn prune(&mut self, active: &BTreeSet<String>, now: u64) {
        self.now = self.now.max(now);
        self.paths.retain(|path, state| {
            active.contains(path) || self.now.saturating_sub(state.seen) < HOUR
        });
    }

    /// Number of retained fingerprints, for adapter memory observations.
    pub fn tracked_paths(&self) -> usize {
        self.paths.len()
    }

    /// Oldest observed change first, then literal path for deterministic
    /// ties. Reobserving an unchanged fingerprint never makes it newer;
    /// paths not observed by this scheduler follow its known candidates.
    pub fn order(&self, paths: &mut [String]) {
        let changed = |path: &str| {
            self.paths
                .get(path)
                .map_or((true, 0), |s| (false, s.changed))
        };
        paths.sort_by(|a, b| changed(a).cmp(&changed(b)).then_with(|| a.cmp(b)));
    }
}
