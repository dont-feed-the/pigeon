//! One bounded, stream-owned native lookup session, with no result cache.
//! Repository identity changes invalidate it; two idle seconds close it.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pigeon_git::Repo;
use pigeon_sys::fs::FileId;

const IDLE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) struct Lookup(tokio::sync::Mutex<State>);

impl std::fmt::Debug for Lookup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Lookup { private native session }")
    }
}

struct State {
    pool: crate::custody_transfer::Pool,
    identity: Option<(PathBuf, FileId)>,
    last: Instant,
    timer: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pool: Default::default(),
            identity: None,
            last: Instant::now(),
            timer: false,
        }
    }
}

fn identity(path: &Path) -> io::Result<FileId> {
    pigeon_sys::fs::lstat(path)
        .map_err(io::Error::other)?
        .map(|stat| stat.id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "repository disappeared"))
}

impl Lookup {
    pub(super) async fn object(
        self: &Arc<Self>,
        repo: Repo,
        git_dir: &Path,
        key: &str,
    ) -> io::Result<PathBuf> {
        let observed = identity(git_dir)?;
        let mut state = self.0.lock().await;
        if state.identity.as_ref() != Some(&(git_dir.to_owned(), observed)) {
            state.pool.stop().await;
            state.identity = Some((git_dir.to_owned(), observed));
        }
        if !state.timer {
            state.timer = true;
            let weak = Arc::downgrade(self);
            tokio::spawn(async move {
                let mut delay = IDLE;
                loop {
                    tokio::time::sleep(delay).await;
                    let Some(lookup) = weak.upgrade() else { return };
                    let mut state = lookup.0.lock().await;
                    delay = IDLE.saturating_sub(state.last.elapsed());
                    if delay.is_zero() {
                        state.pool.stop().await;
                        state.timer = false;
                        return;
                    }
                }
            });
        }
        let mut git = crate::git::Git::new(repo.dir.clone(), repo.env.clone());
        git.repo = repo;
        state.last = Instant::now();
        let object = state
            .pool
            .local_object_for(&git, key, super::CALL_TIMEOUT)
            .await;
        state.last = Instant::now();
        if identity(git_dir)? != observed {
            state.pool.stop().await;
            state.identity = None;
            return Err(io::Error::other("repository changed during lookup"));
        }
        object.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "object unavailable"))
    }
}
