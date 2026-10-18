//! Admission of authoritative full observations (ADR 0013, 07 §7.2).
//! Hints choose when to scan, never which paths exist or what bytes mean.
//! A successful scan acknowledges only its starting revision, so concurrent
//! hints survive. Failed scans acknowledge nothing. Saturation stays dirty.

/// Default maximum time between complete observations, in milliseconds.
pub const PERIOD_MS: u64 = 600_000;

/// Volatile observation history; no catalogue, paths or authority are retained.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schedule {
    observed: Option<(u64, u64)>,
}

impl Schedule {
    /// Whether a background round needs a full authoritative observation.
    /// Scan-only fallback preserves ordinary round polling. Deferred stable/
    /// hot-file work supplies its own deadline independently of event hints.
    pub fn due(&self, now_ms: u64, revision: u64, watched: bool, settling: bool) -> bool {
        let Some((observed_at, seen)) = self.observed else {
            return true;
        };
        !watched
            || settling
            || revision == u64::MAX
            || revision != seen
            || now_ms.saturating_sub(observed_at) >= PERIOD_MS
    }

    /// Milliseconds until a full observation is due, independent of hints.
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        self.observed.map_or(0, |(at, _)| {
            PERIOD_MS.saturating_sub(now_ms.saturating_sub(at))
        })
    }

    /// Record success using the time and revision sampled before starting the scan.
    /// A backwards input cannot manufacture time since a later observation.
    pub fn completed(&mut self, now_ms: u64, starting_revision: u64) {
        let previous = self.observed.map_or(0, |(at, _)| at);
        self.observed = Some((now_ms.max(previous), starting_revision));
    }
}
