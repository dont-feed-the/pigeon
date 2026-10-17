//! One content budget per daemon (ADR 0037, 15 §15.3–15.6). Control
//! frames never reserve tokens. Waiting reserves no future credit and
//! callers re-check their content authority before and after every wait.

use std::collections::BTreeMap;
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
    samples: BTreeMap<(usize, String), (u64, Delay)>,
    rate: [u64; 2],
    buckets: [Bucket; 2],
    last_tick: u64,
    pending: usize,
}

/// Shared across every group and content stream, never one bucket per peer.
#[derive(Debug)]
pub struct Controller {
    epoch: Instant,
    inner: Mutex<State>,
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
            inner: Mutex::new(State {
                settings,
                conditions: Conditions::default(),
                disk: Disk::default(),
                samples: BTreeMap::new(),
                rate: [16_384; 2],
                buckets: [Bucket::new(0, CHUNK as u64, 16_384); 2],
                last_tick: 0,
                pending: 0,
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

    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        self.lock().pending
    }

    /// External observations are sampled independently of active streams.
    pub fn observe(&self, conditions: Conditions, io_pressure: bool) {
        let mut s = self.lock();
        s.conditions = conditions;
        let busy = io_pressure && s.pending > 0;
        s.disk.observe(self.now(), busy);
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
    }

    fn content_blocked(&self) -> bool {
        let s = self.lock();
        let now = self.now();
        let share = s.disk.share(now, s.settings.preset, false);
        resources::external_wait(
            s.conditions,
            s.settings.battery_below,
            s.settings.avoid_metered,
        )
        .is_some()
            || now % 1_000 >= u64::from(share) * 10
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
        direction: Direction,
        bytes: usize,
        mut authorized: impl FnMut() -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        struct Pending<'a>(&'a Controller);
        impl Drop for Pending<'_> {
            fn drop(&mut self) {
                self.0.lock().pending -= 1;
            }
        }
        self.lock().pending += 1;
        let _pending = Pending(self);
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

/// Tick feedback separately from slow native probes. A D-Bus timeout cannot
/// delay authorization refresh or the shared stream budget.
pub async fn run(d: std::sync::Arc<crate::daemon::Daemon>) {
    let mut tick = tokio::time::interval(Duration::from_millis(200));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut reload = Instant::now() - Duration::from_secs(2);
    loop {
        tick.tick().await;
        if reload.elapsed() >= Duration::from_secs(2) {
            reload = Instant::now();
            match crate::config::load(&d.dirs.config_file()).and_then(|c| {
                c.resources
                    .effective()
                    .map_err(crate::config::ConfigError::Invalid)
            }) {
                Ok(settings) => d.node.resources.configure(settings),
                Err(e) => tracing::warn!(error = %e, "resource-settings-refused"),
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
    loop {
        let observation = tokio::time::timeout(Duration::from_secs(2), probe.sample())
            .await
            .unwrap_or_default();
        let pressure = pigeon_sys::pressure::io()
            .ok()
            .flatten()
            .is_some_and(|p| p.avg10 > 10.0);
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
        };
        report.headline = crate::api::message(id, &[]);
    }
}
