//! Native hints admit full background capture observations (ADR 0013).
//! Explicit API observations stay fresh. Metadata, contents and authority
//! are still decided by the existing scan/capture implementation alone.

use std::sync::Arc;
use std::time::{Duration, Instant};

use pigeon_core::change_scan::Schedule;
use pigeon_sys::change_hints::{Reading, Watch};

use crate::capture::Captured;
use crate::git::Error;
use crate::group::Group;
use crate::trace::Tracer;

#[derive(Debug, Default)]
pub(crate) struct State {
    watch: Option<Watch>,
    attempted: bool,
    register_after: Option<Instant>,
    retry_after: Option<Instant>,
    poll_after: Option<Instant>,
    clock: Schedule,
    ordinary_timer: Option<Instant>,
    observed_round: bool,
}

impl State {
    fn reading(&self) -> Reading {
        self.watch.as_ref().map_or(
            Reading {
                revision: 0,
                healthy: false,
                rebuild: false,
            },
            Watch::reading,
        )
    }

    pub(crate) fn bound_expired(&self, delay: Duration, now: Instant) -> Duration {
        if self.attempted && !self.observed_round && delay.is_zero() {
            self.ordinary_timer
                .map_or(delay, |at| at.saturating_duration_since(now))
        } else {
            delay
        }
    }

    pub(crate) fn delay(&self, now: Instant) -> Option<Duration> {
        if let Some(retry) = self.retry_after.filter(|at| *at > now) {
            return Some(retry.saturating_duration_since(now));
        }
        if !self.attempted {
            return None;
        }
        let periodic = pigeon_sys::clock::continuous_millis().ok().map(|time| {
            self.bound_expired(Duration::from_millis(self.clock.remaining_ms(time)), now)
        });
        periodic
            .into_iter()
            .chain(
                self.poll_after
                    .map(|at| self.bound_expired(at.saturating_duration_since(now), now)),
            )
            .min()
    }
}

async fn register(g: &Group, wake: &Arc<tokio::sync::Notify>) {
    let now = Instant::now();
    let (register, previous) = {
        let mut known = g.lock();
        let state = &mut known.capture_watch;
        let reading = state.reading();
        let register = reading.rebuild
            || (!reading.healthy && state.register_after.is_none_or(|at| at <= now));
        let previous = if register {
            state.register_after = Some(now + Duration::from_secs(60));
            state.clock = Schedule::default();
            state.watch.take()
        } else {
            None
        };
        (register, previous)
    };
    if !register {
        return;
    }
    let Some(root) = g.worktree.clone() else {
        return;
    };
    let wake = wake.clone();
    // Recursive registration can enumerate many directories. No group state
    // mutex or async runtime worker is retained while the native call waits.
    let watch = tokio::task::spawn_blocking(move || {
        // Free old kernel slots before registering the replacement. The full
        // observation after registration covers every change during this gap.
        drop(previous);
        Watch::new(&root, move || wake.notify_one())
    })
    .await
    .ok()
    .and_then(Result::ok);
    let mut known = g.lock();
    let state = &mut known.capture_watch;
    state.attempted = true;
    state.watch = watch;
    state.clock = Schedule::default(); // Registration gaps always require a full observation.
    tracing::info!(group = %g.id, native = state.reading().healthy, "capture-watch-registered");
}

pub(crate) async fn background(
    g: &Group,
    me: &str,
    tracer: &Tracer,
    wake: &Arc<tokio::sync::Notify>,
) -> Option<Result<Captured, Error>> {
    if g.worktree.is_none() || crate::evict::unsettled(g) {
        return Some(crate::capture::background(g, me, tracer).await);
    }
    register(g, wake).await;
    let now = Instant::now();
    let time = pigeon_sys::clock::continuous_millis().ok();
    let reading = {
        let known = g.lock();
        let state = &known.capture_watch;
        if state.retry_after.is_some_and(|at| at > now) {
            return None;
        }
        let reading = state.reading();
        let settling = known
            .capture_schedule
            .next_due()
            .is_some_and(|at| at <= now)
            || state.poll_after.is_some_and(|at| at <= now);
        let due = state.clock.due(
            time.unwrap_or(0),
            reading.revision,
            reading.healthy && time.is_some(),
            settling,
        );
        // Counts and monotonic clocks only: diagnose missing native hints
        // independently of candidate scans without logging user paths.
        tracing::debug!(
            group = %g.id,
            watch_revision = reading.revision,
            native = reading.healthy,
            settling,
            due,
            clock = ?state.clock,
            "capture-observation-admission"
        );
        if !due {
            return None;
        }
        reading
    };
    let result = crate::capture::background(g, me, tracer).await;
    let mut known = g.lock();
    known.capture_watch.observed_round = true;
    if let Ok(done) = &result {
        let poll = (done.busy > 0 && known.capture_schedule.next_due().is_none())
            .then(|| Instant::now() + Duration::from_secs(30));
        let state = &mut known.capture_watch;
        state.clock.completed(time.unwrap_or(0), reading.revision);
        state.retry_after = None;
        state.poll_after = poll;
    } else {
        let retry = Instant::now() + Duration::from_secs(5);
        known.capture_schedule.retry_at(retry);
        let state = &mut known.capture_watch;
        state.clock = Schedule::default();
        state.retry_after = Some(retry);
        state.poll_after = None;
    }
    Some(result)
}

/// Failed/missing-root rounds use their ordinary cadence instead of spinning
/// on an expired periodic timer. New native hints can still wake immediately.
pub(crate) fn round_started(g: &Group, interval: Duration) {
    let mut known = g.lock();
    let state = &mut known.capture_watch;
    state.ordinary_timer = Some(Instant::now() + interval);
    state.observed_round = false;
}

/// An explicit capture decision can change eligibility without changing the
/// working tree (for example sharing an already-held deletion). Its next
/// round must observe again even when no new native event can arrive.
pub(crate) fn invalidate(g: &Group) {
    let mut known = g.lock();
    known.capture_watch.clock = Schedule::default();
    known.capture_watch.retry_after = None;
}

/// A native registration failure is informational: scans continue normally.
pub(crate) fn scan_only(g: &Group) -> bool {
    let known = g.lock();
    known.capture_watch.attempted && !known.capture_watch.reading().healthy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unobserved_expired_timer_keeps_the_ordinary_round_cadence() {
        let now = Instant::now();
        let mut state = State {
            attempted: true,
            ordinary_timer: Some(now + Duration::from_secs(30)),
            ..State::default()
        };
        assert_eq!(state.delay(now), Some(Duration::from_secs(30)));
        state.observed_round = true;
        assert_eq!(state.delay(now), Some(Duration::ZERO));
    }

    #[test]
    fn a_failed_observation_retains_its_bounded_retry_before_periodic_work() {
        let now = Instant::now();
        let state = State {
            attempted: true,
            retry_after: Some(now + Duration::from_secs(5)),
            ordinary_timer: Some(now + Duration::from_secs(30)),
            ..State::default()
        };
        assert_eq!(state.delay(now), Some(Duration::from_secs(5)));
    }
}
