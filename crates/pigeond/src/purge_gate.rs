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
    // Metadata writers share leases so parallel fetches stay parallel.
    // Physical erasure takes the exclusive lease through its child exit.
    updates: Arc<tokio::sync::RwLock<()>>,
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
            updates: Default::default(),
        }
    }
}

impl Gate {
    /// Wrapped retained-ref writers may run together, but cannot cross an
    /// actual forced drop. Acquire this before invalidating or spawning Git.
    pub async fn before_mutation(
        &self,
        args: &[&str],
    ) -> Option<tokio::sync::OwnedRwLockReadGuard<()>> {
        if dirty(args) {
            Some(self.updates.clone().read_owned().await)
        } else {
            None
        }
    }

    /// Keep through the drop child's exit. The caller must recheck the
    /// published policy after acquiring this lease, including its key.
    pub async fn erasure(&self) -> tokio::sync::OwnedRwLockWriteGuard<()> {
        self.updates.clone().write_owned().await
    }

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

    /// Cover direct retained-ref mutation and unconstrained fetches.
    /// Quarantined peer/incoming refs are not retained until separately
    /// validated and adopted through a guarded update-ref.
    pub fn mutation(self: &Arc<Self>, args: &[&str]) -> Option<Mutation> {
        if !dirty(args) {
            return None;
        }
        self.writers.fetch_add(1, Ordering::SeqCst);
        self.revision.fetch_add(1, Ordering::SeqCst);
        Some(Mutation(self.clone()))
    }
}

fn dirty(args: &[&str]) -> bool {
    match args.first().copied() {
        Some("fetch") => !quarantined_fetch(args),
        Some("update-ref") => args.iter().any(|arg| {
            *arg == "--stdin"
                || *arg == "refs/heads/main"
                || arg.starts_with(crate::proposal::PREFIX)
                || arg.starts_with(crate::refound::ARCHIVE)
        }),
        _ => false,
    }
}

fn quarantined_fetch(args: &[&str]) -> bool {
    let [
        "fetch",
        "-q",
        "--no-tags",
        "--no-write-fetch-head",
        "--refmap=",
        remote,
        specs @ ..,
    ] = args
    else {
        return false;
    };
    !remote.is_empty()
        && !remote.starts_with('-')
        && !specs.is_empty()
        && specs.iter().all(|spec| {
            spec.strip_prefix('+')
                .and_then(|spec| spec.split_once(':'))
                .is_some_and(|(source, destination)| {
                    source.starts_with("refs/")
                        && (destination.starts_with("refs/pigeon/peers/")
                            || destination.starts_with("refs/pigeon/incoming/")
                            || destination.starts_with("refs/pigeon/reports/"))
                })
        })
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

    #[tokio::test]
    async fn parallel_metadata_writers_and_physical_erasure_cannot_overlap() {
        use std::future::{Future, poll_fn};
        use std::task::Poll;
        let gate = Arc::new(Gate::default());
        let args = ["fetch", "peer"];
        let first = gate.before_mutation(&args).await;
        let second = gate.before_mutation(&args).await;
        let mut erase = Box::pin(gate.erasure());
        assert!(poll_fn(|cx| Poll::Ready(erase.as_mut().poll(cx).is_pending())).await);
        drop(first);
        assert!(poll_fn(|cx| Poll::Ready(erase.as_mut().poll(cx).is_pending())).await);
        drop(second);
        let erase = erase.await;
        let mut writer = Box::pin(gate.before_mutation(&args));
        assert!(poll_fn(|cx| Poll::Ready(writer.as_mut().poll(cx).is_pending())).await);
        // Object reads and status publication need no retained-ref lease.
        assert!(
            gate.before_mutation(&["cat-file", "--batch"])
                .await
                .is_none()
        );
        drop(erase);
        assert!(writer.await.is_some());
    }

    #[tokio::test]
    async fn quarantined_fetches_keep_content_usable_until_validated_history_is_adopted() {
        let gate = Arc::new(Gate::default());
        gate.arm();
        assert!(gate.publish(gate.revision().unwrap(), ContentPolicy::default()));
        let args = [
            "fetch",
            "-q",
            "--no-tags",
            "--no-write-fetch-head",
            "--refmap=",
            "peer",
            "+refs/heads/*:refs/pigeon/peers/peer/*",
            "+refs/pigeon/archive/*:refs/pigeon/peers/peer/archive/*",
            "+refs/pigeon/status/*:refs/pigeon/reports/peer/*",
        ];
        // A physical erasure need not block a quarantined metadata fetch;
        // neither the retained reading nor the content policy can change.
        let erase = gate.erasure().await;
        assert!(gate.before_mutation(&args).await.is_none());
        assert!(gate.mutation(&args).is_none());
        assert!(!gate.policy().waiting);
        drop(erase);
        let adopted = gate.mutation(&["update-ref", "refs/heads/main", "new", "old"]);
        assert!(gate.policy().waiting);
        drop(adopted);
        assert!(gate.policy().waiting);
        // Extra or unbounded refspecs must still take the conservative path.
        for spec in [
            "+refs/heads/*:refs/heads/*",
            "+refs/*:refs/pigeon/archive/*",
            "refs/heads/main",
        ] {
            let mut unsafe_args = args.to_vec();
            unsafe_args.push(spec);
            assert!(gate.mutation(&unsafe_args).is_some());
        }
        assert!(gate.mutation(&["fetch", "peer"]).is_some());
    }

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
