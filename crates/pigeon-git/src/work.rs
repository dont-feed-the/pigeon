//! Scheduling of heavy native children (ADR 0037, 15 §15.4). The
//! adapter supplies nonblocking permissions; active deadlines exclude
//! deliberate waits. Metadata calls never ask this controller to run.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pigeon_core::resources::ActiveClock;
use rustix::process::{Pid, Signal, kill_process_group};

/// Maximum ordinary policy-observation interval for an active child.
pub const POLL_MS: u64 = 20;

/// Heavy work with shared cross-group priority and progress policies.
pub use pigeon_core::resources::WorkClass as Class;

/// A nonblocking view of the daemon's resource policy.
pub trait Control: std::fmt::Debug + Send + Sync {
    /// Record an operation while it is waiting, running or suspended.
    fn began(&self, _class: Class) {}

    /// Release that record on success, failure or cancellation.
    fn ended(&self, _class: Class) {}

    /// A completed comparable own read on the opened inode’s device. Duration
    /// excludes deliberate policy waits and hashing; implementations must
    /// retain only bounded measurements, never content or file names.
    fn read_observed(&self, _device: u64, _elapsed: Duration) {}

    /// Permission for capture verification on its actual opened device.
    /// The default retains the ordinary Capture policy.
    fn reading(&self, _device: u64) -> bool {
        self.running(Class::Capture)
    }

    /// Whether that device's deliberate read waits spend active time.
    fn charging_read(&self, _device: u64) -> bool {
        self.charging(Class::Capture)
    }

    /// Whether a new native operation may start or send its next batch key.
    /// By default admission and active scheduling have the same policy.
    fn starting(&self, class: Class) -> bool {
        self.running(class)
    }

    /// Whether the child's process group may run in this scheduling slice.
    fn running(&self, class: Class) -> bool;

    /// Whether time spent running consumes its operation deadline. A local
    /// stream limiter can deliberately wait without stopping metadata peers.
    fn charging(&self, class: Class) -> bool {
        self.running(class)
    }
}

pub(crate) struct Registration<'a>(pub(crate) &'a dyn Control, pub(crate) Class);

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        self.0.ended(self.1);
    }
}

pub(crate) fn class(args: &[&str]) -> Option<Class> {
    let mut args = args;
    while args.first() == Some(&"-c") {
        args = args.get(2..)?;
    }
    match args {
        ["annex", "add" | "calckey", ..] => Some(Class::Capture),
        ["annex", "get" | "copy" | "reinject" | "setkey", ..] => Some(Class::Transfer),
        ["annex", "fsck", ..] => Some(Class::Scrub),
        _ => None,
    }
}

/// Owned only while the leader remains unreaped. Cancellation resumes and
/// kills the whole group, including helpers, before Tokio drops the child.
pub(crate) struct Group {
    pid: Pid,
    armed: AtomicBool,
    stopped: AtomicBool,
}

impl Group {
    pub(crate) fn new(pid: Pid) -> Self {
        Self {
            pid,
            armed: AtomicBool::new(true),
            stopped: AtomicBool::new(false),
        }
    }

    pub(crate) fn running(&self, running: bool) -> std::io::Result<()> {
        if !self.armed.load(Ordering::Acquire) || self.stopped.load(Ordering::Relaxed) == !running {
            return Ok(());
        }
        kill_process_group(self.pid, if running { Signal::CONT } else { Signal::STOP })?;
        self.stopped.store(!running, Ordering::Relaxed);
        Ok(())
    }

    /// Call immediately after wait reaps the leader, before any await.
    pub(crate) fn reaped(&self) {
        self.armed.store(false, Ordering::Release);
    }

    pub(crate) fn terminate(&self) {
        if self.armed.load(Ordering::Acquire) {
            let _ = kill_process_group(self.pid, Signal::CONT);
            let _ = kill_process_group(self.pid, Signal::TERM);
        }
    }

    pub(crate) fn kill(&self) {
        if self.armed.load(Ordering::Acquire) {
            self.terminate();
            let _ = kill_process_group(self.pid, Signal::KILL);
        }
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Poll output and policy together; returning None leaves `work` alive so
/// the caller can resume/TERM, drain and reap during its cleanup grace.
pub(crate) async fn deadline<F: std::future::Future + Unpin>(
    work: &mut F,
    group: &Group,
    control: Option<(&dyn Control, Class)>,
    progress: Option<&crate::progress::Progress>,
    timeout: Duration,
    slow: Duration,
    argv: &str,
) -> std::io::Result<Option<F::Output>> {
    let epoch = Instant::now();
    let ms = |d: Duration| d.as_millis().min(u128::from(u64::MAX)) as u64;
    let mut budget = ActiveClock::new(
        0,
        control.is_none_or(|(c, k)| c.running(k) && c.charging(k)),
    );
    let mut named = false;
    let mut last_progress = progress.map_or(0, crate::progress::Progress::bytes);
    loop {
        let running = control.is_none_or(|(c, k)| c.running(k));
        group.running(running)?;
        budget.observe(
            ms(epoch.elapsed()),
            running && control.is_none_or(|(c, k)| c.charging(k)),
        );
        if let Some(progress) = progress {
            let bytes = progress.bytes();
            if bytes > last_progress {
                last_progress = bytes;
                budget = ActiveClock::new(
                    ms(epoch.elapsed()),
                    running && control.is_none_or(|(c, k)| c.charging(k)),
                );
            }
        }
        if budget.used() >= ms(timeout) {
            return Ok(None);
        }
        if !named && budget.used() >= ms(slow) {
            tracing::info!(argv, secs = slow.as_secs(), "git-slow");
            named = true;
        }
        tokio::select! {
            biased;
            result = &mut *work => return Ok(Some(result)),
            _ = tokio::time::sleep(Duration::from_millis(POLL_MS).min(Duration::from_millis(budget.remaining(ms(timeout))))) => {},
        }
    }
}

/// A scoped kept-folder transfer: admission uses Download priority, while
/// deadline charging retains the actual native operation's class.
#[derive(Debug)]
pub(crate) struct DownloadControl(pub(crate) std::sync::Arc<dyn Control>);

impl DownloadControl {
    fn priority(class: Class) -> Class {
        if class == Class::Transfer {
            Class::Download
        } else {
            class
        }
    }
}

impl Control for DownloadControl {
    fn reading(&self, device: u64) -> bool {
        self.0.reading(device)
    }
    fn charging_read(&self, device: u64) -> bool {
        self.0.charging_read(device)
    }

    fn read_observed(&self, device: u64, elapsed: Duration) {
        self.0.read_observed(device, elapsed);
    }

    fn began(&self, class: Class) {
        self.0.began(Self::priority(class));
    }
    fn ended(&self, class: Class) {
        self.0.ended(Self::priority(class));
    }
    fn starting(&self, class: Class) -> bool {
        self.0.starting(Self::priority(class))
    }
    fn running(&self, class: Class) -> bool {
        // Finish this key before yielding to protection: stopping it here
        // could retain an annex object lock that protection itself needs.
        // Capture and environmental pauses still use the native policy.
        self.0.running(class)
    }
    fn charging(&self, class: Class) -> bool {
        self.0.charging(class)
    }
}
