//! Native stable/hot-path observations for background capture (15 §15.5).
//! One monotonic clock and one earliest deadline cover all dirty paths;
//! requests can capture explicitly, without spawning per-file sleepers.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use pigeon_core::authority::Entry;
use pigeon_core::capture_schedule::{Readiness, Schedule};
use pigeon_sys::fs::Stat;

use crate::group::Group;

/// Volatile observations, rebuilt conservatively after restart.
#[derive(Debug, Default)]
pub struct State {
    start: Option<Instant>,
    paths: Schedule<Stat>,
    due: Option<Instant>,
    near_install: BTreeMap<String, (Stat, Instant)>,
}

impl State {
    fn elapsed(&mut self, now: Instant) -> u64 {
        let start = *self.start.get_or_insert(now);
        u64::try_from(now.saturating_duration_since(start).as_millis()).unwrap_or(u64::MAX)
    }

    /// Start a new scan; inactive history expires after one hour.
    pub(crate) fn begin(&mut self) {
        let elapsed = self.elapsed(Instant::now());
        self.paths.prune(&BTreeSet::new(), elapsed);
        self.near_install
            .retain(|_, (_, at)| at.elapsed() < Duration::from_secs(3600));
        self.due = None;
    }

    pub(crate) fn ready(&mut self, path: &str, stat: Stat) -> bool {
        let now = Instant::now();
        let elapsed = self.elapsed(now);
        match self.paths.observe(path, stat, stat.size, elapsed) {
            Readiness::Ready => true,
            Readiness::Wait(ms) => {
                let due = now + Duration::from_millis(ms);
                self.due = Some(self.due.map_or(due, |previous| previous.min(due)));
                false
            }
        }
    }

    /// Preserve the original save's concurrent-install classification
    /// through settling and hot-path waits. A later fingerprint replaces it.
    pub(crate) fn near_install(&mut self, path: &str, stat: Stat, recent: bool) -> bool {
        if recent {
            self.near_install
                .insert(path.to_owned(), (stat, Instant::now()));
        } else if self
            .near_install
            .get(path)
            .is_some_and(|(old, _)| *old != stat)
        {
            self.near_install.remove(path);
        }
        self.near_install.contains_key(path)
    }
}

/// Record only an actual signed capture or suggestion, using its entry
/// rather than a batch's attempted paths. Duplicate entries do not heat it.
pub(crate) fn recorded(g: &Group, path: &str, entry: &Entry) {
    let mut known = g.lock();
    let state = &mut known.capture_schedule;
    let elapsed = state.elapsed(Instant::now());
    let version = format!("{}:{}", entry.mode, entry.oid);
    state.paths.captured(path, &version, elapsed);
    state.near_install.remove(path);
}

/// Wake in time for the earliest deferred path even if normal rounds are
/// thirty seconds apart. One expired deadline cannot create a busy loop.
pub(crate) fn delay(groups: &[std::sync::Arc<Group>], interval: Duration) -> Duration {
    let now = Instant::now();
    groups
        .iter()
        .filter_map(|g| g.lock().capture_schedule.due)
        .map(|due| {
            due.saturating_duration_since(now)
                .max(Duration::from_millis(20))
        })
        .fold(interval, Duration::min)
}
