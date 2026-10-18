//! A join is served only after its welcome is checked (09 §9.6). Until
//! then its pending group is held busy, so no round reads it. Failure
//! forgets the group. A laptop takes back unchanged files it wrote. A
//! backup retains its unfinished bare repository: an initially empty
//! directory does not establish ownership of files another application
//! adds while the exchange waits. Its existing joining marker permits a
//! retry without recursively deleting concurrent additions.
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

/// One attempt to join in `path`.
pub(crate) struct Attempt {
    /// The folder, or a backup machine's bare repository, canonical.
    pub path: PathBuf,
    /// The attempt made the folder: it goes on failure if it is empty.
    made: bool,
    /// A bare backup repository, retained on failure for a safe retry.
    bare: bool,
}

impl Attempt {
    /// Starts an attempt in `path`, which it makes if it does not exist.
    pub fn begin(path: &Path, bare: bool) -> Result<Attempt, String> {
        let made = !path.exists();
        std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
        let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        Ok(Attempt { path, made, bare })
    }

    /// Whether the folder is empty, as a new join needs.
    pub fn empty(&self) -> Result<bool, String> {
        Ok(std::fs::read_dir(&self.path)
            .map_err(|e| e.to_string())?
            .next()
            .is_none())
    }

    /// Runs `join`; failure rolls back a laptop or retains an unfinished backup.
    pub async fn run<T>(
        &self,
        d: &Daemon,
        join: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        let result = join.await;
        if let Err(code) = &result {
            if self.bare {
                tracing::info!(path = %self.path.display(), %code, "join-backup-kept-for-retry");
            } else {
                tracing::info!(path = %self.path.display(), %code, "join-undone");
            }
            self.undo(d).await;
        }
        result
    }

    fn undo_bare(&self) {
        // Git/annex and unrelated writers may have added files after the
        // empty-folder check. There is no ownership manifest, so retain
        // the complete repository, its joining marker and its annex UUID.
        tracing::debug!(path = %self.path.display(), "incomplete-backup-repository-retained");
    }

    /// Removes what the attempt wrote. A file of the group's tree goes
    /// only while it still matches the index, so one changed since stays.
    async fn undo(&self, d: &Daemon) {
        if self.bare {
            self.undo_bare();
            return;
        }
        let git = Git::new(&self.path, d.env.clone());
        if self.path.join(".git").exists() {
            let _ = git.run(&["update-index", "-q", "--refresh"]).await;
            let changed: Vec<String> = git
                .run(&["diff-files", "--name-only", "-z"])
                .await
                .unwrap_or_default()
                .split('\0')
                .map(str::to_owned)
                .collect();
            let written = git.run(&["ls-files", "-z", "-t"]).await.unwrap_or_default();
            for entry in written.split('\0') {
                // `H` is a file in the folder; `S` one the join hid.
                let Some(file) = entry.strip_prefix("H ") else {
                    continue;
                };
                if !changed.iter().any(|c| c == file) {
                    let _ = std::fs::remove_file(self.path.join(file));
                    remove_empty_parents(&self.path, &self.path.join(file));
                }
            }
        }
        for own in [".git", ".gitattributes", ".pigeon"] {
            remove_tree(&self.path.join(own));
        }
        if self.made && self.empty().unwrap_or(false) {
            let _ = std::fs::remove_dir(&self.path);
        }
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

/// Removes `path`, a file or a tree, even one whose directories git-annex
/// made read-only.
fn remove_tree(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    if !meta.is_dir() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let mut dirs = vec![path.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        if let Ok(meta) = std::fs::symlink_metadata(&dir)
            && meta.is_dir()
        {
            let mode = meta.permissions().mode() | 0o700;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode));
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    dirs.push(entry.path());
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(path);
}

/// Removes the directories between `file` and `root` that are now empty.
fn remove_empty_parents(root: &Path, file: &Path) {
    let mut dir = file.parent();
    while let Some(d) = dir.filter(|d| *d != root && d.starts_with(root)) {
        if std::fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;

    fn preserves_new_backup_files(existing: bool) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("backup.git");
        if existing {
            std::fs::create_dir(&path).unwrap();
        }
        let attempt = Attempt::begin(&path, true).unwrap();
        assert!(attempt.empty().unwrap());
        // Another application writes after the initial empty-folder check,
        // before a refused exchange asks the attempt to roll back.
        std::fs::create_dir(path.join("objects")).unwrap();
        std::fs::write(path.join("notes.txt"), b"new personal notes").unwrap();
        std::fs::write(path.join("objects/local-note.txt"), b"new nested notes").unwrap();
        attempt.undo_bare();
        assert_eq!(
            std::fs::read(path.join("notes.txt")).unwrap(),
            b"new personal notes"
        );
        assert_eq!(
            std::fs::read(path.join("objects/local-note.txt")).unwrap(),
            b"new nested notes"
        );
    }

    #[test]
    fn failure_in_a_new_backup_folder_preserves_concurrent_files() {
        preserves_new_backup_files(false);
    }

    #[test]
    fn failure_in_an_existing_backup_folder_preserves_concurrent_files() {
        preserves_new_backup_files(true);
    }

    #[test]
    fn a_tree_made_read_only_is_removed_and_only_empty_parents_go() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let objects = tmp.path().join(".git/annex/objects/ab");
        std::fs::create_dir_all(&objects).unwrap();
        std::fs::write(objects.join("k"), b"x").unwrap();
        std::fs::set_permissions(&objects, std::fs::Permissions::from_mode(0o555)).unwrap();
        remove_tree(&tmp.path().join(".git"));
        assert!(!tmp.path().join(".git").exists());

        let kept = tmp.path().join("a/kept.txt");
        let gone = tmp.path().join("a/b/c/gone.txt");
        std::fs::create_dir_all(gone.parent().unwrap()).unwrap();
        std::fs::write(&kept, b"mine").unwrap();
        std::fs::write(&gone, b"group").unwrap();
        std::fs::remove_file(&gone).unwrap();
        remove_empty_parents(tmp.path(), &gone);
        assert!(!tmp.path().join("a/b").exists());
        assert!(kept.exists());
    }
}
