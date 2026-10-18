//! A join is served only after its welcome is checked (09 §9.6). Until
//! then its pending group is held busy, so no round reads it. Failure
//! forgets the group and retains the unfinished repository and all files:
//! an initial empty-folder check does not establish ownership of anything
//! another application adds while the exchange waits. The joining marker
//! permits a retry without recursively deleting concurrent additions.
//!
//! The device's annex UUID is kept in `<state>/joins/<group>` (11 §11.2)
//! until the join succeeds: once the inviter signed the welcome, a retry
//! resumes it, and the inviter checks that the UUID is the same.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::GroupEntry;
use crate::daemon::Daemon;
use crate::git::Git;
use crate::group::Group;

#[cfg(test)]
#[path = "join_cleanup_tests.rs"]
mod cleanup_tests;

/// One attempt to join in `path`.
pub(crate) struct Attempt {
    /// The folder, or a backup machine's bare repository, canonical.
    pub path: PathBuf,
}

impl Attempt {
    /// Starts an attempt in `path`, which it makes if it does not exist.
    pub fn begin(path: &Path) -> Result<Attempt, String> {
        std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
        let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        Ok(Attempt { path })
    }

    /// Whether the folder is empty, as a new join needs.
    pub fn empty(&self) -> Result<bool, String> {
        Ok(std::fs::read_dir(&self.path)
            .map_err(|e| e.to_string())?
            .next()
            .is_none())
    }

    /// Runs `join`; a failure retains all files for a safe retry.
    pub async fn run<T>(&self, join: impl Future<Output = Result<T, String>>) -> Result<T, String> {
        let result = join.await;
        if let Err(code) = &result {
            tracing::info!(path = %self.path.display(), %code, "join-kept-for-retry");
        }
        result
    }
}

/// Serves `g`, at `path`, while `verify` checks its welcome, and for good
/// once it passed; a failure forgets it again. Its lock is held all along,
/// so that no round runs on a group not yet checked.
pub(crate) async fn serve_verified<F, Fut>(
    d: &Daemon,
    mut g: Group,
    path: &Path,
    verify: F,
) -> Result<Arc<Group>, String>
where
    F: FnOnce(Arc<Group>) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    crate::shared::configure(&mut g, d.config.bridge_storage.as_ref())?;
    let g = Arc::new(g);
    let busy = g.busy.lock().await;
    d.groups
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .push(g.clone());
    if let Err(code) = verify(g.clone()).await {
        d.groups
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|live| !Arc::ptr_eq(live, &g));
        drop(busy);
        g.stop_remotedaemon().await;
        return Err(code);
    }
    // The new group's roster names this device: its key must outlive a
    // restart even after leaving the last group deleted it (09 §9.7).
    if let Err(code) = d.keep_key() {
        d.groups
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|live| !Arc::ptr_eq(live, &g));
        drop(busy);
        g.stop_remotedaemon().await;
        return Err(code);
    }
    let entry = GroupEntry {
        id: g.id.clone(),
        path: path.to_path_buf(),
        priority: Default::default(),
    };
    if let Err(e) = crate::config::add_group(&d.dirs.config_file(), &entry) {
        d.groups
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|live| !Arc::ptr_eq(live, &g));
        drop(busy);
        g.stop_remotedaemon().await;
        return Err(e.to_string());
    }
    // Bootstrap access serves only the unverified welcome. The persisted
    // group now admits every current roster peer through the usual rule.
    g.lock().bootstrap_peer = None;
    d.serve_group(g.clone(), None).await;
    drop(busy);
    forget_uuid(d, &g.id);
    Ok(g)
}

/// Where the annex UUID of a join of `group` is kept; none for a group
/// ID that is not a plain name.
fn uuid_file(d: &Daemon, group: &str) -> Option<PathBuf> {
    (!group.is_empty() && group.bytes().all(|b| b.is_ascii_alphanumeric()))
        .then(|| d.dirs.state.join("joins").join(group))
}

/// Gives the new repository `git` the annex UUID of an earlier attempt to
/// join `group`, before `git annex init`, and keeps the one it then has.
pub(crate) async fn reuse_uuid(d: &Daemon, group: &str, git: &Git) -> Result<(), String> {
    let Some(file) = uuid_file(d, group) else {
        return Ok(());
    };
    if let Ok(uuid) = std::fs::read_to_string(&file) {
        let uuid = uuid.trim();
        if !uuid.is_empty() {
            git.run(&["config", "annex.uuid", uuid])
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Keeps `uuid` for a retry of the join of `group`.
pub(crate) fn remember_uuid(d: &Daemon, group: &str, uuid: &str) {
    if let Some(file) = uuid_file(d, group) {
        let _ = file.parent().map(std::fs::create_dir_all);
        let _ = pigeon_sys::fs::write_atomic(&file, uuid.as_bytes());
    }
}

fn forget_uuid(d: &Daemon, group: &str) {
    if let Some(file) = uuid_file(d, group) {
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;

    async fn preserves_new_backup_files(existing: bool) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("backup.git");
        if existing {
            std::fs::create_dir(&path).unwrap();
        }
        let attempt = Attempt::begin(&path).unwrap();
        assert!(attempt.empty().unwrap());
        // Another application writes after the initial empty-folder check,
        // before a refused exchange asks the attempt to roll back.
        std::fs::create_dir(path.join("objects")).unwrap();
        std::fs::write(path.join("notes.txt"), b"new personal notes").unwrap();
        std::fs::write(path.join("objects/local-note.txt"), b"new nested notes").unwrap();
        let result = attempt
            .run(async { Err::<(), _>("invite-proof".to_owned()) })
            .await;
        assert_eq!(result, Err("invite-proof".to_owned()));
        assert_eq!(
            std::fs::read(path.join("notes.txt")).unwrap(),
            b"new personal notes"
        );
        assert_eq!(
            std::fs::read(path.join("objects/local-note.txt")).unwrap(),
            b"new nested notes"
        );
    }

    #[tokio::test]
    async fn failure_in_a_new_backup_folder_preserves_concurrent_files() {
        preserves_new_backup_files(false).await;
    }

    #[tokio::test]
    async fn failure_in_an_existing_backup_folder_preserves_concurrent_files() {
        preserves_new_backup_files(true).await;
    }
}
