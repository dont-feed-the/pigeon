//! Native recursive filesystem hints (ADR 0013, 07 §7.2).
//! A constant-size revision wakes the owner; events carry no catalogue or
//! authorization. Access events and pigeon's root metadata are ignored.
//! Failed/overflowed observations force scanning; no paths enter diagnostics.

use std::collections::BTreeSet;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use notify::{Event, EventKind, RecursiveMode, Watcher};

#[derive(Debug)]
struct Hints {
    revision: AtomicU64,
    healthy: AtomicBool,
    rebuild: AtomicBool,
}

/// Current hint revision and whether native event delivery remains available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading {
    /// Saturating generation; saturation always requires another full scan.
    pub revision: u64,
    /// False requires scan-only fallback, even after a successful scan.
    pub healthy: bool,
    /// Linux directory topology changed; rebuild registration before observing.
    pub rebuild: bool,
}

/// Registration failed; callers must retain periodic scan-only observation.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("filesystem event watching is unavailable")]
pub struct Unavailable;

/// One owned recursive registration. Dropping it releases native watches.
pub struct Watch {
    hints: Arc<Hints>,
    _native: notify::RecommendedWatcher,
}

impl std::fmt::Debug for Watch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watch")
            .field("reading", &self.reading())
            .finish()
    }
}

fn internal(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).ok().is_some_and(|relative| {
        matches!(relative.components().next(), Some(Component::Normal(name))
            if name == ".git" || name == ".pigeon" || name == ".gitattributes")
    })
}

fn relevant(root: &Path, event: &Event) -> bool {
    if event.need_rescan() {
        return true;
    }
    if matches!(event.kind, EventKind::Access(kind)
        if !matches!(kind, notify::event::AccessKind::Close(notify::event::AccessMode::Write | notify::event::AccessMode::Any)))
    {
        return false;
    }
    event.paths.is_empty() || event.paths.iter().any(|path| !internal(root, path))
}

fn topology_changed(directories: &Mutex<BTreeSet<std::path::PathBuf>>, event: &Event) -> bool {
    #[cfg(target_os = "linux")]
    {
        event.need_rescan()
            || match event.kind {
                EventKind::Create(notify::event::CreateKind::Folder)
                | EventKind::Remove(notify::event::RemoveKind::Folder) => true,
                EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                    event.paths.iter().any(|path| {
                        directories
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .contains(path)
                            || std::fs::symlink_metadata(path).is_ok_and(|stat| stat.is_dir())
                    })
                }
                _ => false,
            }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (directories, event);
        false
    }
}

#[cfg(target_os = "linux")]
fn register_tree(
    native: &mut notify::RecommendedWatcher,
    root: &Path,
    directories: &Mutex<BTreeSet<std::path::PathBuf>>,
) -> Result<(), Unavailable> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        // Release this lock before waiting for the backend: its callback may
        // inspect the known directory set while handling an earlier event.
        directories
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(path.clone());
        native
            .watch(&path, RecursiveMode::NonRecursive)
            .map_err(|_| Unavailable)?;
        for entry in std::fs::read_dir(&path).map_err(|_| Unavailable)? {
            let entry = entry.map_err(|_| Unavailable)?;
            let child = entry.path();
            if !internal(root, &child) && entry.file_type().map_err(|_| Unavailable)?.is_dir() {
                pending.push(child);
            }
        }
    }
    Ok(())
}

impl Hints {
    fn record(
        &self,
        root: &Path,
        root_id: crate::fs::FileId,
        directories: &Mutex<BTreeSet<std::path::PathBuf>>,
        result: notify::Result<Event>,
    ) -> bool {
        // One fault wakes fallback once; a broken native backend cannot flood
        // the daemon with retries while its registration is being replaced.
        if !self.healthy.load(Ordering::Acquire) {
            return false;
        }
        let changed = match result {
            Ok(event) => {
                let root_changed = (event.need_rescan()
                    || event
                        .paths
                        .iter()
                        .any(|p| p == root || !p.starts_with(root)))
                    && !crate::fs::lstat(root)
                        .ok()
                        .flatten()
                        .is_some_and(|stat| stat.id == root_id);
                if root_changed {
                    self.healthy.store(false, Ordering::Release);
                }
                // A queued access may be the first event handled after root
                // removal. Its invalidation still needs one fallback wake.
                let changed = root_changed || relevant(root, &event);
                if changed && topology_changed(directories, &event) {
                    self.rebuild.store(true, Ordering::Release);
                }
                changed
            }
            Err(_) => {
                self.healthy.store(false, Ordering::Release);
                true
            }
        };
        if changed {
            let _ = self
                .revision
                .try_update(Ordering::AcqRel, Ordering::Acquire, |revision| {
                    Some(revision.saturating_add(1))
                });
        }
        changed
    }
}

impl Watch {
    /// Register without following directory symlinks. The callback only
    /// increments a revision and wakes the caller; it never reads user bytes.
    pub fn new(root: &Path, wake: impl Fn() + Send + Sync + 'static) -> Result<Self, Unavailable> {
        let root = root.canonicalize().map_err(|_| Unavailable)?;
        let watched_root = root.clone();
        let root_id = crate::fs::lstat(&root)
            .map_err(|_| Unavailable)?
            .ok_or(Unavailable)?
            .id;
        let hints = Arc::new(Hints {
            revision: AtomicU64::new(0),
            healthy: AtomicBool::new(true),
            rebuild: AtomicBool::new(false),
        });
        let observed = hints.clone();
        let directories = Arc::new(Mutex::new(BTreeSet::new()));
        let observed_directories = directories.clone();
        let mut native = notify::RecommendedWatcher::new(
            move |result: notify::Result<Event>| {
                if observed.record(&watched_root, root_id, &observed_directories, result) {
                    wake();
                }
            },
            notify::Config::default().with_follow_symlinks(false),
        )
        .map_err(|_| Unavailable)?;
        #[cfg(target_os = "linux")]
        register_tree(&mut native, &root, &directories)?;
        #[cfg(not(target_os = "linux"))]
        native
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|_| Unavailable)?;
        Ok(Self {
            hints,
            _native: native,
        })
    }

    /// Cheap immutable hint state, independent of any filesystem scan result.
    pub fn reading(&self) -> Reading {
        Reading {
            revision: self.hints.revision.load(Ordering::Acquire),
            healthy: self.hints.healthy.load(Ordering::Acquire),
            rebuild: self.hints.rebuild.load(Ordering::Acquire),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "isolated native hint fault fixtures")]
    use super::*;
    use notify::event::{AccessKind, AccessMode, DataChange, Flag};

    fn hints() -> Hints {
        Hints {
            revision: AtomicU64::new(0),
            healthy: AtomicBool::new(true),
            rebuild: AtomicBool::new(false),
        }
    }

    #[test]
    fn a_native_error_admits_fallback_once_instead_of_a_retry_storm() {
        let hints = hints();
        let root = Path::new("/fixture");
        let id = crate::fs::FileId { dev: 0, ino: 0 };
        let directories = Mutex::new(BTreeSet::new());
        assert!(hints.record(
            root,
            id,
            &directories,
            Err(notify::Error::generic("private native detail"))
        ));
        assert!(!hints.healthy.load(Ordering::Acquire));
        assert_eq!(hints.revision.load(Ordering::Acquire), 1);
        assert!(!hints.record(
            root,
            id,
            &directories,
            Err(notify::Error::generic("another detail"))
        ));
        assert_eq!(hints.revision.load(Ordering::Acquire), 1);
    }

    #[test]
    fn root_loss_on_an_ignored_access_still_wakes_fallback() {
        let container = tempfile::tempdir().unwrap();
        let root = container.path().join("gone");
        let hints = hints();
        let event =
            Event::new(EventKind::Access(AccessKind::Open(AccessMode::Any))).add_path(root.clone());
        assert!(hints.record(
            &root,
            crate::fs::FileId { dev: 0, ino: 0 },
            &Mutex::new(BTreeSet::new()),
            Ok(event)
        ));
        assert!(!hints.healthy.load(Ordering::Acquire));
        assert_eq!(hints.revision.load(Ordering::Acquire), 1);
    }

    #[test]
    fn overflow_forces_observation_even_on_an_ignored_access_event() {
        let event = Event::new(EventKind::Access(AccessKind::Read))
            .add_path(Path::new("/fixture/.git/index").into())
            .set_flag(Flag::Rescan);
        assert!(relevant(Path::new("/fixture"), &event));
    }

    #[test]
    fn metadata_chatter_is_ignored_but_closing_a_user_writer_is_a_hint() {
        let root = Path::new("/fixture");
        let metadata = Event::new(EventKind::Modify(notify::event::ModifyKind::Data(
            DataChange::Any,
        )))
        .add_path(root.join(".git/index"));
        assert!(!relevant(root, &metadata));
        let closed = Event::new(EventKind::Access(AccessKind::Close(AccessMode::Write)))
            .add_path(root.join("saved.txt"));
        assert!(relevant(root, &closed));
        let read = Event::new(EventKind::Access(AccessKind::Read)).add_path(root.join("saved.txt"));
        assert!(!relevant(root, &read));
    }
}
