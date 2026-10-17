//! Content work outlives a round without holding its worktree lock
//! (ADR 0037, 15 §15.2). Reference snapshots and fresh validation hold
//! `busy`; cold location probes and key-only transfers do not. Capture,
//! admission and erasure keep running.

use std::sync::{Arc, Weak};

use crate::{daemon::Daemon, group::Group};

/// A group explicitly aborts and joins this job before stopping or moving
/// its repository. Dropping an unjoined handle must not detach the worker.
#[derive(Debug)]
pub(crate) struct Task(Option<tokio::task::JoinHandle<usize>>);

impl Task {
    pub(crate) async fn stop(mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
            let _ = task.await;
        }
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }
}

/// Called with the round's lock held. At most one job per group runs;
/// completed copies are observed by the normal report/status path.
pub(crate) async fn queue(d: &Arc<Daemon>, g: &Arc<Group>) -> usize {
    let previous = {
        let mut job = g.placement.lock().await;
        if job
            .as_ref()
            .is_some_and(|t| t.0.as_ref().is_some_and(|h| !h.is_finished()))
        {
            return 0;
        }
        job.take()
    };
    let made = if let Some(mut old) = previous {
        match old.0.take() {
            Some(handle) => handle.await.unwrap_or(0),
            None => 0,
        }
    } else {
        0
    };
    let group = g.clone();
    let daemon = Arc::downgrade(d);
    let me = d.device.clone();
    let wake = d.wake.clone();
    let task = tokio::spawn(async move {
        let made = crate::custody::place_worker(&group, &me, &daemon).await;
        let received = if authorized(&daemon, &group).await {
            downloads(&daemon, &group).await
        } else {
            0
        };
        if made > 0 || received > 0 {
            wake.notify_one();
        }
        made
    });
    *g.placement.lock().await = Some(Task(Some(task)));
    made
}

/// Revalidate identity and retained authority between keys. The actual
/// transport filter remains authoritative throughout each active transfer.
pub(crate) async fn authorized(d: &Weak<Daemon>, g: &Group) -> bool {
    let _busy = g.busy.lock().await;
    current(d, g).await
}

async fn current(d: &Weak<Daemon>, g: &Group) -> bool {
    let Some(d) = d.upgrade() else { return false };
    if !d.current_repository(g).await
        || crate::pause::now(&d).is_some()
        || crate::leave::leaving(g).is_some()
        || g.lock().admission_error.is_some()
        || crate::membership::pending(g, &d.device)
    {
        return false;
    }
    true
}

/// Share one retained-key reading between a worker's keys until any
/// guarded reference mutation invalidates it. Never cache an in-flight
/// or incomplete reading, and never scan all history once per file.
#[derive(Default)]
pub(crate) struct Validation {
    revision: Option<u64>,
    keys: std::collections::BTreeSet<String>,
}

impl Validation {
    pub(crate) async fn allows(&mut self, d: &Weak<Daemon>, g: &Group, key: &str) -> bool {
        if !current(d, g).await {
            return false;
        }
        let Some(revision) = g.git.purge_gate.revision() else {
            return false;
        };
        // A complete unchanged reading needs no worktree lock. Taking it
        // once per small file queues every transfer behind a full round.
        // Reference writers invalidate this revision before they mutate.
        if self.revision == Some(revision) {
            return self.keys.contains(key) && g.git.purge_gate.revision() == Some(revision);
        }
        let _busy = g.busy.lock().await;
        if !current(d, g).await {
            return false;
        }
        let Some(revision) = g.git.purge_gate.revision() else {
            return false;
        };
        if self.revision != Some(revision) {
            let Ok(keys) = crate::retention::keys(g).await else {
                return false;
            };
            if g.git.purge_gate.revision() != Some(revision) {
                return false;
            }
            self.keys = keys;
            self.revision = Some(revision);
        }
        self.keys.contains(key)
    }
}

/// Kept current versions are fetched after protection work. Install and keep
/// only use locally available objects while holding the worktree lock.
async fn downloads(d: &Weak<Daemon>, g: &Group) -> usize {
    if g.worktree.is_none() {
        return 0;
    }
    let keys = {
        let _busy = g.busy.lock().await;
        let Some(main) = g.git.rev("refs/heads/main").await else {
            return 0;
        };
        let Ok(keys) = g.git.keys(&main).await else {
            return 0;
        };
        let targets = crate::shared_overlay::targets(g);
        keys.into_iter()
            .filter(|(path, _)| !targets.is_freed(path))
            .map(|(_, key)| key)
            .collect::<std::collections::BTreeSet<_>>()
    };
    let mut made = 0;
    let mut validation = Validation::default();
    for key in keys {
        if crate::custody::present(g, &key).await.is_some() {
            continue;
        }
        if !validation.allows(d, g, &key).await {
            break;
        }
        if let Err(code) = crate::restore::bring(g, &key).await {
            tracing::debug!(group = %g.id, key, code, "kept-content-waits");
        } else {
            made += 1;
        }
    }
    made
}
