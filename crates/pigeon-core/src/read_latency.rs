//! Bounded per-device own-read feedback (ADR 0037, 15 §15.4). Monotonic
//! inputs only: sample at most once per 10 ms and assess once per second.
//! A ten-second median above three times the quiet baseline means busy.

use std::collections::VecDeque;

/// A device's bounded observations; no paths or content are retained.
#[derive(Clone, Debug, Default)]
pub struct Window {
    samples: VecDeque<(u64, u64, bool)>,
    first: Option<u64>,
    last: u64,
    assessed: Option<u64>,
    baseline: Option<u64>,
    busy: bool,
}

impl Window {
    /// Record a successful nonempty bounded read, in nanoseconds. Zero clock
    /// resolution is one nanosecond; backwards time cannot grow the window.
    /// `quiet` excludes an independently observed busy disk from training.
    pub fn observe(&mut self, now_ms: u64, latency_ns: u64, quiet: bool) {
        let now = now_ms.max(self.last);
        self.last = now;
        while self
            .samples
            .front()
            .is_some_and(|(at, _, _)| now - at >= 10_000)
        {
            self.samples.pop_front();
        }
        if self
            .samples
            .back()
            .is_some_and(|(at, _, _)| now / 10 == at / 10)
        {
            return;
        }
        let first = *self.first.get_or_insert(now);
        self.samples.push_back((now, latency_ns.max(1), quiet));
        if self.assessed.is_some_and(|at| now - at < 1_000) {
            return;
        }
        self.assessed = Some(now);
        if now - first < 10_000 {
            return;
        }
        let mut values: Vec<_> = self.samples.iter().map(|(_, ns, _)| *ns).collect();
        let middle = values.len() / 2;
        let (_, median, _) = values.select_nth_unstable(middle);
        let median = *median;
        if self.samples.iter().all(|(_, _, quiet)| *quiet)
            && self.baseline.is_none_or(|base| median < base)
        {
            self.baseline = Some(median);
        }
        self.busy = self
            .baseline
            .is_some_and(|base| u128::from(median) > 3 * u128::from(base));
    }

    /// A stale reading cannot indefinitely hold work paused. The adapter
    /// still applies the ordinary five-second quiet and thirty-second ramp.
    pub fn busy(&self, now_ms: u64) -> bool {
        self.busy && now_ms.max(self.last).saturating_sub(self.last) < 10_000
    }

    /// Actual retained samples, bounded by 1000 independent of read rate.
    pub fn samples(&self) -> usize {
        self.samples.len()
    }
}
