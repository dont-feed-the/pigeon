//! Scheduling of heavy one-shot children (ADR 0037, 15 §15.4). The
//! adapter supplies nonblocking permissions; active deadlines exclude
//! deliberate waits. Metadata calls never ask this controller to run.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pigeon_core::resources::ActiveClock;
use rustix::process::{Pid, Signal, kill_process_group};

/// Maximum ordinary policy-observation interval for an active child.
pub const POLL_MS: u64 = 20;

/// Heavy work with different progress and external-wait policies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Capture keeps a floor even while the disk is busy.
    Capture,
    /// Network contents may wait for power, network or disk conditions.
    Transfer,
    /// Background integrity work may wait indefinitely for a good moment.
    Scrub,
}

/// A nonblocking view of the daemon's resource policy.
pub trait Control: std::fmt::Debug + Send + Sync {
    /// Record an operation while it is waiting, running or suspended.
    fn began(&self, _class: Class) {}

    /// Release that record on success, failure or cancellation.
    fn ended(&self, _class: Class) {}

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
    loop {
        let running = control.is_none_or(|(c, k)| c.running(k));
        group.running(running)?;
        budget.observe(
            ms(epoch.elapsed()),
            running && control.is_none_or(|(c, k)| c.charging(k)),
        );
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
