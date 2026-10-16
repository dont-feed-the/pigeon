//! Cheap immutable purge policy at transport boundaries (08 §8.6).
//! A reference mutation invalidates the reading before it starts and after
//! it finishes, including cancellation. Only a complete matching revision
//! may publish a policy. Metadata remains usable while content waits.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use pigeon_core::filter::ContentPolicy;

#[derive(Debug)]
struct Reading {
    revision: u64,
    policy: Arc<ContentPolicy>,
}

/// Shared by every clone of the live repository driver.
#[derive(Debug)]
pub struct Gate {
    armed: AtomicBool,
    revision: AtomicU64,
    writers: AtomicU64,
    reading: RwLock<Reading>,
    pending: Arc<ContentPolicy>,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            armed: AtomicBool::new(false),
            revision: AtomicU64::new(0),
            writers: AtomicU64::new(0),
            reading: RwLock::new(Reading {
                revision: 0,
                policy: Arc::new(ContentPolicy::default()),
            }),
            pending: Arc::new(ContentPolicy {
                waiting: true,
                released: Default::default(),
            }),
        }
    }
}

impl Gate {
    /// Whether purge metadata has required a complete reading.
    pub fn armed(&self) -> bool {
        self.armed.load(Ordering::SeqCst)
    }

    /// An incomplete or unsupported reading must never restore default
    /// transfers of historical keys that might already have been erased.
    pub fn suspend(&self) {
        self.arm();
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    /// Called as soon as purge metadata or an archive requires evaluation.
    pub fn arm(&self) {
        if !self.armed.swap(true, Ordering::SeqCst) {
            self.revision.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// A stable generation for an incremental scan, or an in-flight writer.
    pub fn revision(&self) -> Option<u64> {
        let revision = self.revision.load(Ordering::SeqCst);
        (self.writers.load(Ordering::SeqCst) == 0).then_some(revision)
    }

    /// Publish only after reading all exact refs and checking them again.
    /// A concurrent writer makes this stale; policy() still returns pending.
    pub fn publish(&self, revision: u64, policy: ContentPolicy) -> bool {
        if self.revision() != Some(revision) {
            return false;
        }
        *self.reading.write().unwrap_or_else(|p| p.into_inner()) = Reading {
            revision,
            policy: Arc::new(policy),
        };
        self.revision() == Some(revision)
    }

    /// Constant work per protocol chunk, with no I/O or set cloning.
    pub fn policy(&self) -> Arc<ContentPolicy> {
        let reading = self.reading.read().unwrap_or_else(|p| p.into_inner());
        if self.armed.load(Ordering::SeqCst) && self.revision() != Some(reading.revision) {
            self.pending.clone()
        } else {
            reading.policy.clone()
        }
    }

    /// Cover every fetch (including archive refs) and direct retained-ref
    /// mutation. Immutable objects and replaceable status refs do not dirty
    /// a completed reference scan.
    pub fn mutation(self: &Arc<Self>, args: &[&str]) -> Option<Mutation> {
        let dirty = match args.first().copied() {
            Some("fetch") => true,
            Some("update-ref") => args.iter().any(|arg| {
                *arg == "--stdin"
                    || *arg == "refs/heads/main"
                    || arg.starts_with(crate::proposal::PREFIX)
                    || arg.starts_with(crate::refound::ARCHIVE)
            }),
            _ => false,
        };
        if !dirty {
            return None;
        }
        self.writers.fetch_add(1, Ordering::SeqCst);
        self.revision.fetch_add(1, Ordering::SeqCst);
        Some(Mutation(self.clone()))
    }
}

/// Cancellation is also a mutation boundary: a killed Git command may
/// already have changed one of the refs before its caller is dropped.
#[derive(Debug)]
pub struct Mutation(Arc<Gate>);

impl Drop for Mutation {
    fn drop(&mut self) {
        self.0.revision.fetch_add(1, Ordering::SeqCst);
        self.0.writers.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scan_cannot_publish_across_a_writer_or_its_cancellation() {
        let gate = Arc::new(Gate::default());
        assert!(!gate.policy().waiting);
        gate.arm();
        let revision = gate.revision().unwrap();
        assert!(gate.policy().waiting);
        assert!(gate.publish(revision, ContentPolicy::default()));
        let writer = gate.mutation(&["fetch", "peer"]);
        assert!(gate.policy().waiting);
        assert!(gate.revision().is_none());
        assert!(!gate.publish(revision, ContentPolicy::default()));
        drop(writer);
        assert!(gate.policy().waiting);
        assert!(!gate.publish(revision, ContentPolicy::default()));
        assert!(gate.publish(gate.revision().unwrap(), ContentPolicy::default()));
        assert!(!gate.policy().waiting);
        assert!(
            gate.mutation(&["update-ref", "refs/pigeon/status/me", "oid"])
                .is_none()
        );
        assert!(!gate.policy().waiting);
        let first = gate.mutation(&["update-ref", "refs/heads/main", "oid"]);
        let second = gate.mutation(&["fetch", "another"]);
        drop(first);
        assert!(gate.revision().is_none());
        drop(second);
        assert!(gate.policy().waiting);
    }
}
