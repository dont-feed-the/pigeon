//! One content budget per daemon (ADR 0037, 15 §15.3–15.6). Control
//! frames never reserve tokens. Waiting reserves no future credit and
//! callers re-check their content authority before and after every wait.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use pigeon_core::resources::{self, Bucket, Conditions, Delay, Disk, Permit, Wait};

use crate::resource_config::Effective;

/// Maximum payload reservation; long DATA frames are split at this boundary.
pub const CHUNK: usize = 65_536;

/// An authenticated selected path, scoped to its connection and address.
#[derive(Debug)]
pub(crate) struct PathSample {
    pub id: (usize, String),
    pub rtt_ms: u64,
    pub capacity: u64,
}

/// Content direction relative to this computer, independent of initiator.
#[derive(Clone, Copy, Debug)]
pub enum Direction {
    /// Bytes leaving this computer.
    Upload,
    /// Bytes entering this computer.
    Download,
}

#[derive(Debug)]
struct State {
    settings: Effective,
    conditions: Conditions,
    disk: Disk,
    io_pressure: bool,
    reads: BTreeMap<u64, (u64, pigeon_core::read_latency::Window)>,
    samples: BTreeMap<(usize, String), (u64, Delay)>,
    rate: [u64; 2],
    buckets: [Bucket; 2],
    last_tick: u64,
    pending: usize,
    group_waits: BTreeMap<String, usize>,
}

/// Shared across every group and content stream, never one bucket per peer.
#[derive(Debug)]
pub struct Controller {
    epoch: Instant,
    inner: Mutex<State>,
    children: [AtomicUsize; 4],
    person_paused: AtomicBool,
}

impl Default for Controller {
    fn default() -> Self {
        Self::new(Effective {
            preset: resources::Preset::Gentle,
            upload: None,
            download: None,
            battery_below: 20,
            avoid_metered: true,
            full_speed: Vec::new(),
        })
    }
}

impl Controller {
    /// A valid configuration starts with empty token buckets.
    pub fn new(settings: Effective) -> Self {
        Self {
            epoch: Instant::now(),
            children: std::array::from_fn(|_| AtomicUsize::new(0)),
            person_paused: AtomicBool::new(false),
            inner: Mutex::new(State {
                settings,
                conditions: Conditions::default(),
                disk: Disk::default(),
                io_pressure: false,
                reads: BTreeMap::new(),
                samples: BTreeMap::new(),
                rate: [16_384; 2],
                buckets: [Bucket::new(0, CHUNK as u64, 16_384); 2],
                last_tick: 0,
                pending: 0,
                group_waits: BTreeMap::new(),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn now(&self) -> u64 {
        self.epoch.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
    }

    /// Apply an entire validated reading; invalid readings never reach here.
    pub fn configure(&self, settings: Effective) {
        self.lock().settings = settings;
    }

    /// A person's pause suspends initiated heavy work while the serving
    /// side remains available to peers, as in 15 §15.6.
    pub fn set_person_paused(&self, paused: bool) {
        self.person_paused.store(paused, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        self.lock().pending
    }

    /// External observations are sampled independently of active streams.
    pub fn observe(&self, conditions: Conditions, io_pressure: bool) {
        let mut s = self.lock();
        s.conditions = conditions;
        let now = self.now();
        s.io_pressure = io_pressure
            && (s.pending > 0 || self.children.iter().any(|n| n.load(Ordering::Relaxed) > 0));
        let busy = s.io_pressure || s.reads.values().any(|(_, w)| w.busy(now));
        s.disk.observe(now, busy);
    }

    fn read_observed(&self, device: u64, elapsed: Duration) {
        self.read_at(self.now(), device, elapsed);
    }

    fn read_at(&self, now: u64, device: u64, elapsed: Duration) {
        let mut s = self.lock();
        s.reads
            .retain(|_, (seen, _)| now.saturating_sub(*seen) < 600_000);
        // A bound on devices as well as samples; overflow falls back to
        // native pressure. Do not evict an active device's learned baseline.
        if !s.reads.contains_key(&device) && s.reads.len() >= 32 {
            return;
        }
        let quiet = !s.io_pressure;
        let (seen, window) = s.reads.entry(device).or_default();
        *seen = now.max(*seen);
        window.observe(
            now,
            elapsed.as_nanos().min(u128::from(u64::MAX)) as u64,
            quiet,
        );
        let busy = s.io_pressure || s.reads.values().any(|(_, w)| w.busy(now));
        s.disk.observe(now, busy);
    }

    /// Visible reason content or maintenance cannot run now.
    pub fn waiting(&self) -> Option<Wait> {
        let s = self.lock();
        resources::external_wait(
            s.conditions,
            s.settings.battery_below,
            s.settings.avoid_metered,
        )
        .or_else(|| {
            (s.disk.share(self.now(), s.settings.preset, false) < 100).then_some(Wait::DiskBusy)
        })
        .or_else(|| self.capturing().then_some(Wait::Saving))
    }

    fn capturing(&self) -> bool {
        self.children[resources::WorkClass::Capture as usize].load(Ordering::Relaxed) > 0
    }

    fn content_blocked(&self) -> bool {
        let s = self.lock();
        let now = self.now();
        let share = s.disk.share(now, s.settings.preset, false);
        self.capturing()
            || resources::external_wait(
                s.conditions,
                s.settings.battery_below,
                s.settings.avoid_metered,
            )
            .is_some()
            || now % 1_000 >= u64::from(share) * 10
    }

    /// A repository shares the controller but excludes only its own stream
    /// waits from child deadlines; another group's shaper cannot extend it.
    pub fn for_group(
        self: &std::sync::Arc<Self>,
        group: String,
    ) -> std::sync::Arc<dyn pigeon_git::work::Control> {
        std::sync::Arc::new(ChildControl {
            controller: self.clone(),
            group,
            repair: false,
        })
    }

    /// A designated repair protects contents alongside transfers (15 §15.2).
    /// Its fsck budget remains independent of content-stream shaping waits.
    pub fn for_repair_group(
        self: &std::sync::Arc<Self>,
        group: String,
    ) -> std::sync::Arc<dyn pigeon_git::work::Control> {
        std::sync::Arc::new(ChildControl {
            controller: self.clone(),
            group,
            repair: true,
        })
    }

    /// Update delay feedback at most once per 200 ms, with a rolling ten
    /// minute minimum. Capacity is a selected QUIC path's cwnd/RTT estimate.
    pub(crate) fn tick(&self, paths: Vec<PathSample>, minute: u16) {
        let now = self.now();
        let mut s = self.lock();
        if now.saturating_sub(s.last_tick) < 200 {
            return;
        }
        s.last_tick = now;
        s.samples
            .retain(|_, (at, _)| now.saturating_sub(*at) <= 600_000);
        let mut delay = 0;
        let mut capacity: u64 = 0;
        for path in paths.into_iter().filter(|p| p.capacity > 0) {
            let (at, baseline) = s.samples.entry(path.id).or_default();
            *at = now;
            delay = delay.max(baseline.observe(now, path.rtt_ms));
            capacity = capacity.saturating_add(path.capacity);
        }
        let target = if s.settings.full_speed.iter().any(|w| w.contains(minute)) {
            None
        } else {
            s.settings.preset.delay_target()
        };
        for (i, ceiling) in [s.settings.upload, s.settings.download]
            .into_iter()
            .enumerate()
        {
            s.rate[i] = resources::rate(s.rate[i], capacity, delay, target, 1, ceiling);
        }
    }

    fn reserve(&self, direction: Direction, bytes: usize) -> Permit {
        let mut s = self.lock();
        let index = match direction {
            Direction::Upload => 0,
            Direction::Download => 1,
        };
        let ceiling = match direction {
            Direction::Upload => s.settings.upload,
            Direction::Download => s.settings.download,
        };
        // A newly lowered ceiling applies immediately, even before a tick.
        let rate = s.rate[index].min(ceiling.unwrap_or(u64::MAX));
        s.buckets[index].reserve(self.now(), bytes as u64, rate)
    }

    /// Wait without spending tokens on cancellation. The callback must use
    /// the filter's existing active-key check, not a second authorization rule.
    pub async fn acquire(
        &self,
        group: &str,
        direction: Direction,
        bytes: usize,
        mut authorized: impl FnMut() -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        struct Pending<'a>(&'a Controller, &'a str);
        impl Drop for Pending<'_> {
            fn drop(&mut self) {
                let mut state = self.0.lock();
                state.pending -= 1;
                if let Some(count) = state.group_waits.get_mut(self.1) {
                    *count -= 1;
                    if *count == 0 {
                        state.group_waits.remove(self.1);
                    }
                }
            }
        }
        {
            let mut state = self.lock();
            state.pending += 1;
            *state.group_waits.entry(group.to_owned()).or_default() += 1;
        }
        let _pending = Pending(self, group);
        loop {
            authorized()?;
            let delay = if self.content_blocked() {
                200
            } else {
                match self.reserve(direction, bytes) {
                    Permit::Granted => {
                        authorized()?;
                        return Ok(());
                    }
                    Permit::Wait(ms) => ms.clamp(1, 200),
                    Permit::TooLarge(_) => {
                        return Err(std::io::Error::other("content reservation exceeds chunk"));
                    }
                }
            };
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    }
}

#[cfg(test)]
#[path = "resource_read_tests.rs"]
mod read_tests;

#[derive(Debug)]
struct ChildControl {
    controller: std::sync::Arc<Controller>,
    group: String,
    repair: bool,
}

impl ChildControl {
    fn priority(&self, class: pigeon_git::work::Class) -> pigeon_git::work::Class {
        if self.repair && class == pigeon_git::work::Class::Scrub {
            pigeon_git::work::Class::Transfer
        } else {
            class
        }
    }
}

impl pigeon_git::work::Control for ChildControl {
    fn read_observed(&self, device: u64, elapsed: Duration) {
        self.controller.read_observed(device, elapsed);
    }

    fn began(&self, class: pigeon_git::work::Class) {
        self.controller.children[self.priority(class) as usize].fetch_add(1, Ordering::Relaxed);
    }

    fn ended(&self, class: pigeon_git::work::Class) {
        self.controller.children[self.priority(class) as usize].fetch_sub(1, Ordering::Relaxed);
    }

    fn running(&self, class: pigeon_git::work::Class) -> bool {
        let pending = self
            .controller
            .children
            .each_ref()
            .map(|n| n.load(Ordering::Relaxed) as u64);
        if !self.priority(class).admitted(pending) {
            return false;
        }
        let capture = class == pigeon_git::work::Class::Capture;
        let state = self.controller.lock();
        if !capture
            && (self.controller.person_paused.load(Ordering::Relaxed)
                || resources::external_wait(
                    state.conditions,
                    state.settings.battery_below,
                    state.settings.avoid_metered,
                )
                .is_some())
        {
            return false;
        }
        let now = self.controller.now();
        let share = state.disk.share(now, state.settings.preset, capture);
        let window = u64::from(share) * 10;
        // Leave one observation's margin so discretizing a 50 ms capture
        // floor into child polls cannot reduce it to two 20 ms slices.
        let window = if capture {
            (window + pigeon_git::work::POLL_MS).min(1_000)
        } else {
            window
        };
        now % 1_000 < window
    }

    fn charging(&self, class: pigeon_git::work::Class) -> bool {
        // Keep the actual operation class: a repair fsck never waits on
        // the content bucket, even though it shares protection priority.
        class != pigeon_git::work::Class::Transfer
            || !self.controller.lock().group_waits.contains_key(&self.group)
    }
}

/// Tick feedback separately from slow native probes. A D-Bus timeout cannot
/// delay authorization refresh or the shared stream budget.
pub async fn run(d: std::sync::Arc<crate::daemon::Daemon>) {
    let mut tick = tokio::time::interval(Duration::from_millis(200));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut reload = Instant::now() - Duration::from_secs(2);
    let mut native_priority = crate::resource_supervisor::Requester::new(d.dirs.state.clone());
    loop {
        tick.tick().await;
        if reload.elapsed() >= Duration::from_secs(2) {
            reload = Instant::now();
            d.node
                .resources
                .set_person_paused(crate::pause::now(&d).is_some());
            match crate::config::load(&d.dirs.config_file()) {
                Ok(config) => match config.resources.effective() {
                    Ok(settings) => {
                        native_priority.request(settings.preset);
                        d.node.resources.configure(settings);
                        crate::download_queue::configure(&d, &config);
                    }
                    Err(error) => tracing::warn!(%error, "resource-settings-refused"),
                },
                Err(error) => tracing::warn!(%error, "resource-settings-refused"),
            }
        }
        let now = crate::util::now();
        let offset = i64::from(pigeon_sys::clock::local_offset_minutes(now)) * 60;
        let minute = (i128::from(now) + i128::from(offset)).rem_euclid(86_400) / 60;
        d.node
            .resources
            .tick(d.node.resource_sample(), minute as u16);
    }
}

/// Refresh platform state at a modest rate, with a bounded wait. Unknown
/// native readings follow ADR 0037; a previous low battery is not permanent.
pub async fn probe(d: std::sync::Arc<crate::daemon::Daemon>) {
    let mut probe = pigeon_sys::resources::Probe::default();
    let mut previous_pressure = None;
    loop {
        let observation = tokio::time::timeout(Duration::from_secs(2), probe.sample())
            .await
            .unwrap_or_default();
        let reading = pigeon_sys::pressure::io().ok().flatten();
        let pressure = reading.is_some_and(|p| p.avg10 > 10.0);
        let pending = d.node.resources.lock().pending;
        let children = d
            .node
            .resources
            .children
            .each_ref()
            .map(|n| n.load(Ordering::Relaxed));
        let outstanding = pending > 0 || children.iter().any(|n| *n > 0);
        let sample = (pressure, outstanding);
        if previous_pressure != Some(sample)
            && (pressure || previous_pressure.is_some_and(|(high, _)| high))
        {
            tracing::info!(
                pressure_high = pressure,
                avg10 = ?reading.map(|p| p.avg10),
                pending_content = pending,
                pending_children = ?children,
                "resource-io-pressure"
            );
        }
        previous_pressure = Some(sample);
        d.node.resources.observe(
            Conditions {
                battery: observation.battery,
                low_power: observation.low_power.unwrap_or(false),
                metered: observation.metered.unwrap_or(false),
            },
            pressure,
        );
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Resource waits explain pending protection without replacing missing
/// contents, a person's pause, or a problem needing attention.
pub fn tell(d: &crate::daemon::Daemon, report: &mut pigeon_proto::api::StatusReport) {
    use pigeon_proto::api::{Level, State};
    if !matches!(report.level, Level::Working | Level::NotSafe)
        || report.state == State::Missing
        || crate::pause::now(d).is_some()
    {
        return;
    }
    if let Some(wait) = d.node.resources.waiting() {
        let id = match wait {
            Wait::Battery => "headline-resource-battery",
            Wait::Metered => "headline-resource-metered",
            Wait::DiskBusy => "headline-resource-disk",
            Wait::Saving => "headline-resource-saving",
        };
        report.headline = crate::api::message(id, &[]);
    }
}

#[cfg(test)]
#[path = "resource_priority_tests.rs"]
mod priority_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use pigeon_git::work::Class;

    #[test]
    fn power_and_network_waits_leave_capture_running_and_heavy_reads_observe_pressure() {
        let controller = std::sync::Arc::new(Controller::default());
        let child = controller.for_group("a".into());
        controller.observe(
            Conditions {
                battery: Some(1),
                low_power: true,
                metered: true,
            },
            false,
        );
        assert!(child.running(Class::Capture));
        assert!(!child.running(Class::Transfer));
        assert!(!child.running(Class::Scrub));
        child.began(Class::Capture);
        controller.observe(Conditions::default(), true);
        assert_eq!(controller.waiting(), Some(Wait::DiskBusy));
        assert!(!child.running(Class::Transfer));
        child.ended(Class::Capture);
        assert!(
            controller
                .children
                .iter()
                .all(|n| n.load(Ordering::Relaxed) == 0)
        );
    }

    #[test]
    fn a_persons_pause_suspends_initiated_children_but_never_capture() {
        let controller = std::sync::Arc::new(Controller::default());
        let child = controller.for_group("a".into());
        controller.set_person_paused(true);
        assert!(child.running(Class::Capture));
        assert!(!child.running(Class::Transfer));
        assert!(!child.running(Class::Scrub));
        controller.set_person_paused(false);
        assert!(child.running(Class::Transfer));
        assert!(child.running(Class::Scrub));
    }

    #[tokio::test]
    #[allow(clippy::unwrap_used, reason = "bounded live stream-wait fixture")]
    async fn a_stream_wait_extends_only_its_own_groups_transfer_budget() {
        let controller = std::sync::Arc::new(Controller::default());
        let a = controller.for_group("a".into());
        let repair = controller.for_repair_group("a".into());
        let b = controller.for_group("b".into());
        let queued = controller.clone();
        let task = tokio::spawn(async move {
            queued
                .acquire("a", Direction::Upload, CHUNK, || Ok(()))
                .await
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while controller.pending() == 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        assert!(!a.charging(Class::Transfer));
        assert!(b.charging(Class::Transfer));
        assert!(a.charging(Class::Capture));
        assert!(a.charging(Class::Scrub));
        assert!(repair.charging(Class::Scrub));
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(controller.pending(), 0);
        assert!(controller.lock().group_waits.is_empty());
        assert!(a.charging(Class::Transfer));
    }
}
