//! Resource decisions (ADR 0037, 15 §15.3–15.6), using monotonic milliseconds
//! supplied by the adapter. Unknown power/network observations do not pause
//! work. Capture retains a progress floor under contention; control traffic
//! never uses the content limiter. Laws live in `tests/resources.rs` and
//! `tests/work_priority.rs`.

/// Priority among heavy one-shot children across all groups. History and
/// status do not enter this controller. Transfer sub-priorities and the full
/// placement queue are separate from these four existing child classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkClass {
    /// Saving local changes takes precedence over content and maintenance.
    Capture,
    /// Content movement and designated repairs precede background integrity checks.
    Transfer,
    /// Optional kept-folder retrieval waits for protection work.
    Download,
    /// Integrity work waits while any higher class has pending work.
    Scrub,
}

impl WorkClass {
    /// Classes in decreasing priority, matching their counter indices.
    pub const ALL: [Self; 4] = [Self::Capture, Self::Transfer, Self::Download, Self::Scrub];

    /// Same-class work may run together; only a higher pending class holds
    /// this class. Counts include work that has not started or is suspended.
    pub fn admitted(self, pending: [u64; 4]) -> bool {
        pending[..self as usize].iter().all(|count| *count == 0)
    }
}

/// A single QUIC path's rolling base RTT. Paths keep separate baselines:
/// joining a distant peer must not turn propagation into queuing delay.
#[derive(Clone, Debug, Default)]
pub struct Delay {
    minima: std::collections::VecDeque<(u64, u64)>,
    last: u64,
}

impl Delay {
    /// Observe smoothed RTT at monotonic milliseconds and return its delay
    /// above this path's minimum over the inclusive last ten minutes.
    pub fn observe(&mut self, now: u64, rtt_ms: u64) -> u64 {
        let now = now.max(self.last);
        self.last = now;
        while self
            .minima
            .front()
            .is_some_and(|(at, _)| now - at > 600_000)
        {
            self.minima.pop_front();
        }
        while self.minima.back().is_some_and(|(_, rtt)| *rtt >= rtt_ms) {
            self.minima.pop_back();
        }
        self.minima.push_back((now, rtt_ms));
        rtt_ms.saturating_sub(self.minima.front().map_or(rtt_ms, |(_, rtt)| *rtt))
    }
}

/// The locally chosen preset; explicit settings override these defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
    /// Background priority and the smallest network queuing target.
    #[default]
    Gentle,
    /// Background priority with a larger network queuing target.
    Balanced,
    /// Foreground priority; automatic network shaping is disabled.
    Fast,
}

impl Preset {
    /// Minimum capture share while the disk is contended, in percent.
    pub fn capture_floor(self) -> u8 {
        if self == Self::Fast { 50 } else { 5 }
    }

    /// Automatic network queuing target in milliseconds, or no shaper.
    pub fn delay_target(self) -> Option<u64> {
        match self {
            Self::Gentle => Some(25),
            Self::Balanced => Some(60),
            Self::Fast => None,
        }
    }

    /// Pause below this battery percentage; zero disables that threshold.
    pub fn battery_threshold(self) -> u8 {
        match self {
            Self::Gentle => 20,
            Self::Balanced => 10,
            Self::Fast => 0,
        }
    }
}

/// Current external conditions, not a claim that any file is safe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Conditions {
    /// Remaining percentage when running on battery; absent while plugged in
    /// or when the platform cannot determine the power source.
    pub battery: Option<u8>,
    /// The platform's low-power mode, when known.
    pub low_power: bool,
    /// The platform's expensive/constrained network observation, when known.
    pub metered: bool,
}

/// One visible reason why content and maintenance wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// Saving local changes currently takes precedence over content work.
    Saving,
    /// Battery is below the configured threshold or low-power mode is on.
    Battery,
    /// The current network is metered and the local policy avoids it.
    Metered,
    /// The disk has not recovered from observed contention yet.
    DiskBusy,
}

/// Power and network policy never applies to capture or history exchange.
pub fn external_wait(
    conditions: Conditions,
    battery_below: u8,
    avoid_metered: bool,
) -> Option<Wait> {
    if battery_below > 0
        && (conditions.low_power || conditions.battery.is_some_and(|p| p < battery_below))
    {
        Some(Wait::Battery)
    } else if avoid_metered && conditions.metered {
        Some(Wait::Metered)
    } else {
        None
    }
}

/// Quiet time before capturing a regular file, in milliseconds. Decimal
/// megabytes follow §15.5: 2 seconds + 1 second per 100 MB, capped at 60 s.
pub fn quiet_ms(size: u64) -> u64 {
    2_000u64
        .saturating_add((size / 100_000_000).saturating_mul(1_000))
        .min(60_000)
}

/// Disk back-off state. A fresh busy observation restarts the quiet window;
/// backwards clocks cannot manufacture quiet time or progress quota.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Disk {
    quiet_since: Option<u64>,
    last: u64,
    contended: bool,
}

impl Disk {
    /// Record measured contention while Pigeon is using the relevant disk.
    pub fn observe(&mut self, now_ms: u64, busy: bool) {
        let now_ms = now_ms.max(self.last);
        self.last = now_ms;
        if busy {
            self.contended = true;
            self.quiet_since = None;
        } else if self.contended && self.quiet_since.is_none() {
            self.quiet_since = Some(now_ms);
        }
    }

    /// Allowed heavy-work share. After five quiet seconds, ramp from the
    /// floor to full speed over thirty seconds. Capture always gets its
    /// configured floor; transfers and scrubs get zero while contended.
    pub fn share(&self, now_ms: u64, preset: Preset, capture: bool) -> u8 {
        let floor = if capture { preset.capture_floor() } else { 0 };
        if !self.contended {
            return 100;
        }
        let quiet = self
            .quiet_since
            .map_or(0, |since| now_ms.saturating_sub(since));
        let ramp = quiet.saturating_sub(5_000).min(30_000);
        floor + ((u64::from(100 - floor) * ramp) / 30_000) as u8
    }
}

/// One delay-based rate update. Capacity and ceilings are bytes per second.
/// Every call represents a 200 ms controller tick. Explicit ceilings apply
/// even in Fast/full-speed mode; a ceiling below the floor wins.
pub fn rate(
    previous: u64,
    capacity: u64,
    queuing_ms: u64,
    target_ms: Option<u64>,
    active_classes: u32,
    ceiling: Option<u64>,
) -> u64 {
    let maximum = ceiling.unwrap_or(u64::MAX).max(1);
    let floor = u64::from(active_classes.max(1))
        .saturating_mul(16_384)
        .min(maximum);
    let Some(target) = target_ms else {
        return maximum;
    };
    let target = target.max(1);
    let step = (capacity / 100).max(1);
    let change =
        i128::from(step) * (i128::from(target) - i128::from(queuing_ms)) / i128::from(target);
    let next = (i128::from(previous) + change).clamp(i128::from(floor), i128::from(maximum));
    next as u64
}

/// A shared token bucket, with byte-millisecond precision and bounded burst.
/// A denied reservation spends nothing; the adapter waits then retries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bucket {
    credit: u128,
    last: u64,
    burst: u64,
    rate: u64,
}

/// Result of reserving bytes in the shared bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permit {
    /// The requested bytes have been charged and may be sent once.
    Granted,
    /// No bytes were charged; wait this many milliseconds then retry.
    Wait(u64),
    /// Split the request into chunks no larger than this burst.
    TooLarge(u64),
}

impl Bucket {
    /// Start empty: changing settings or reconnecting grants no free burst.
    pub fn new(now_ms: u64, burst: u64, rate: u64) -> Self {
        Self {
            credit: 0,
            last: now_ms,
            burst: burst.max(1),
            rate: rate.max(1),
        }
    }

    /// Reserve at most one burst. Credit accrues at the preceding rate;
    /// a settings change cannot retroactively grant a faster past interval.
    /// A backwards clock adds no credit. Denial does not spend any credit.
    pub fn reserve(&mut self, now_ms: u64, bytes: u64, rate: u64) -> Permit {
        let now_ms = now_ms.max(self.last);
        self.credit = self
            .credit
            .saturating_add(u128::from(now_ms - self.last) * u128::from(self.rate))
            .min(u128::from(self.burst) * 1_000);
        self.last = now_ms;
        self.rate = rate.max(1);
        if bytes > self.burst {
            return Permit::TooLarge(self.burst);
        }
        let cost = u128::from(bytes) * 1_000;
        if cost <= self.credit {
            self.credit -= cost;
            return Permit::Granted;
        }
        let missing = cost - self.credit;
        Permit::Wait(
            missing
                .div_ceil(u128::from(self.rate))
                .min(u128::from(u64::MAX)) as u64,
        )
    }
}

/// An operation's active-time budget. Deliberate suspension spends no
/// budget; a backwards observation never refunds time or charges a pause.
/// The adapter records each actual stop/resume transition in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveClock {
    last: u64,
    used: u64,
    running: bool,
}

impl ActiveClock {
    /// Start a fresh operation, which may already be waiting for a permit.
    pub fn new(now_ms: u64, running: bool) -> Self {
        Self {
            last: now_ms,
            used: 0,
            running,
        }
    }

    /// Charge only the preceding running interval, then record the new
    /// state. Saturating arithmetic cannot wrap a spent budget into credit.
    pub fn observe(&mut self, now_ms: u64, running: bool) {
        let now_ms = now_ms.max(self.last);
        if self.running {
            self.used = self.used.saturating_add(now_ms - self.last);
        }
        self.last = now_ms;
        self.running = running;
    }

    /// Active milliseconds spent, independent of the duration of pauses.
    pub fn used(self) -> u64 {
        self.used
    }

    /// Remaining active time; expiration cannot wrap or become negative.
    pub fn remaining(self, budget_ms: u64) -> u64 {
        budget_ms.saturating_sub(self.used)
    }
}
