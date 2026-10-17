//! Native stable/hot-path observations for background capture (15 §15.5).
//! One monotonic clock and one earliest deadline cover all dirty paths;
//! requests can capture explicitly, without spawning per-file sleepers.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use pigeon_core::authority::Entry;
use pigeon_core::capture_schedule::{Readiness, Schedule};
use pigeon_sys::fs::Stat;

use crate::group::Group;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifiedFile {
    format: u32,
    paths: BTreeMap<String, ([i128; 9], [String; 3])>,
}

fn stamp(stat: Stat) -> [i128; 9] {
    [
        stat.id.dev.into(),
        stat.id.ino.into(),
        stat.size.into(),
        stat.mtime_ns.into(),
        stat.ctime_ns.into(),
        stat.nlink.into(),
        stat.mode.into(),
        stat.uid.into(),
        stat.gid.into(),
    ]
}

/// Volatile scheduling observations. Exact byte/stat/index observations are
/// also cached on disk; they authorize no transfer, overwrite or deletion.
#[derive(Debug, Default)]
pub struct State {
    paths: Schedule<Stat>,
    due: Option<Instant>,
    near_install: BTreeMap<String, (Stat, Instant)>,
    verified: BTreeMap<String, ([i128; 9], Entry, Instant)>,
    verified_loaded: bool,
}

impl State {
    /// Start a new scan; inactive history expires after one hour.
    pub(crate) fn begin(&mut self) {
        if let Ok(elapsed) = pigeon_sys::clock::continuous_millis() {
            self.paths.prune(&BTreeSet::new(), elapsed);
        }
        self.near_install
            .retain(|_, (_, at)| at.elapsed() < Duration::from_secs(3600));
        self.due = None;
        self.verified
            .retain(|_, (_, _, seen)| seen.elapsed() < Duration::from_secs(3600));
    }

    pub(crate) fn verified(&mut self, path: &str, stat: Stat, entry: &Entry) -> bool {
        let Some((before, version, seen)) = self.verified.get_mut(path) else {
            return false;
        };
        if *before != stamp(stat) || version != entry {
            return false;
        }
        *seen = Instant::now();
        true
    }

    pub(crate) fn ready(&mut self, path: &str, stat: Stat) -> bool {
        let now = Instant::now();
        let Ok(elapsed) = pigeon_sys::clock::continuous_millis() else {
            return false;
        };
        match self.paths.observe(path, stat, stat.size, elapsed) {
            Readiness::Ready => true,
            Readiness::Wait(ms) => {
                let due = now + Duration::from_millis(ms);
                self.due = Some(self.due.map_or(due, |previous| previous.min(due)));
                false
            }
        }
    }

    pub(crate) fn explicit(&mut self, path: &str, stat: Stat) {
        if let Ok(elapsed) = pigeon_sys::clock::continuous_millis() {
            self.paths.observe(path, stat, stat.size, elapsed);
        }
    }

    pub(crate) fn order(&self, paths: &mut [String]) {
        self.paths.order(paths);
    }

    /// Preserve the original save's concurrent-install classification
    /// through settling and hot-path waits. A later fingerprint replaces it.
    pub(crate) fn near_install(&mut self, path: &str, stat: Stat, recent: bool) -> bool {
        if recent {
            self.near_install
                .insert(path.to_owned(), (stat, Instant::now()));
        } else if self
            .near_install
            .get(path)
            .is_some_and(|(old, _)| *old != stat)
        {
            self.near_install.remove(path);
        }
        self.near_install.contains_key(path)
    }
}

/// Missing, future or unreadable caches mean unknown, never clean. The full
/// stat includes ctime, so a same-size write preserving mtime invalidates it.
pub(crate) fn load(g: &Group) {
    let mut known = g.lock();
    let state = &mut known.capture_schedule;
    if state.verified_loaded {
        return;
    }
    state.verified_loaded = true;
    let path = g.git_dir.join("pigeon/capture-verified.json");
    let Some(file) = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<VerifiedFile>(&bytes).ok())
        .filter(|file| file.format == 1)
    else {
        return;
    };
    state.verified.extend(
        file.paths
            .into_iter()
            .map(|(path, (stamp, [mode, kind, oid]))| {
                (path, (stamp, Entry { mode, kind, oid }, Instant::now()))
            }),
    );
}

fn save(g: &Group) {
    let paths = g
        .lock()
        .capture_schedule
        .verified
        .iter()
        .map(|(path, (stamp, entry, _))| {
            (
                path.clone(),
                (
                    *stamp,
                    [entry.mode.clone(), entry.kind.clone(), entry.oid.clone()],
                ),
            )
        })
        .collect();
    let result = serde_json::to_vec(&VerifiedFile { format: 1, paths })
        .map_err(std::io::Error::other)
        .and_then(|bytes| {
            crate::util::write_private(&g.git_dir.join("pigeon/capture-verified.json"), &bytes)
        });
    if let Err(error) = result {
        tracing::warn!(group = %g.id, %error, "capture-observation-cache-unavailable");
    }
}

/// Bind the actual bytes after ingestion to their exact staged pointer.
/// Annex may change ctime while adding/removing its temporary hardlink, so
/// an unchanged pre-ingestion stat cannot prove this observation. CalcKey
/// hashes the working file afresh (Command/CalcKey.hs run -> Backend.genKey),
/// under capture's native resource control, never during a status request.
pub(crate) async fn checked(g: &Group, paths: &[String], index: &pigeon_core::authority::Tree) {
    if paths.is_empty() {
        return;
    }
    load(g);
    let Some(wt) = &g.worktree else { return };
    for chunk in paths.chunks(64) {
        let before: Vec<_> = chunk
            .iter()
            .map(|path| pigeon_sys::fs::lstat(&wt.join(path)).ok().flatten())
            .collect();
        let files: Vec<_> = chunk.iter().map(|path| wt.join(path)).collect();
        let mut args = vec!["annex", "calckey", "--"];
        let Some(names) = files
            .iter()
            .map(|file| file.to_str())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        args.extend(names);
        let Ok(keys) = g.git.run(&args).await else {
            continue;
        };
        let keys: Vec<_> = keys.lines().collect();
        if keys.len() != chunk.len() {
            continue;
        }
        let Ok(Some(index_stat)) = pigeon_sys::fs::lstat(&g.git_dir.join("index")) else {
            continue;
        };
        for ((path, before), key) in chunk.iter().zip(before).zip(keys) {
            let Some(stat) = before else { continue };
            let Some(entry) = index.get(path) else {
                continue;
            };
            if !stat.is_regular()
                || stat.ctime_ns >= index_stat.mtime_ns
                || (stat.mode & 0o111 != 0) != (entry.mode == "100755")
                || pigeon_sys::fs::lstat(&wt.join(path)).ok().flatten() != Some(stat)
            {
                continue;
            }
            let Ok(pointer) = g.git.object(&entry.oid).await else {
                continue;
            };
            if crate::git::pointer_key(&pointer).as_deref() != Some(key) {
                continue;
            }
            g.lock().capture_schedule.verified.insert(
                path.to_owned(),
                (stamp(stat), entry.clone(), Instant::now()),
            );
        }
    }
    save(g);
}

/// Record only an actual signed capture or suggestion, using its entry
/// rather than a batch's attempted paths. Duplicate entries do not heat it.
pub(crate) fn recorded(g: &Group, path: &str, entry: &Entry) {
    let mut known = g.lock();
    let state = &mut known.capture_schedule;
    let Ok(elapsed) = pigeon_sys::clock::continuous_millis() else {
        return;
    };
    let version = format!("{}:{}", entry.mode, entry.oid);
    state.paths.captured(path, &version, elapsed);
    state.near_install.remove(path);
}

/// Wake in time for the earliest deferred path even if normal rounds are
/// thirty seconds apart. One expired deadline cannot create a busy loop.
pub(crate) fn delay(groups: &[std::sync::Arc<Group>], interval: Duration) -> Duration {
    let now = Instant::now();
    groups
        .iter()
        .filter_map(|g| g.lock().capture_schedule.due)
        .map(|due| {
            due.saturating_duration_since(now)
                .max(Duration::from_millis(20))
        })
        .fold(interval, Duration::min)
}
