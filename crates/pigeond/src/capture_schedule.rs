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
    paths: BTreeMap<String, ([i128; 9], [String; 4])>,
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
    verified: BTreeMap<String, ([i128; 9], Entry, String, Instant)>,
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
            .retain(|_, (_, _, _, seen)| seen.elapsed() < Duration::from_secs(3600));
    }

    pub(crate) fn verified(&mut self, path: &str, stat: Stat, entry: &Entry) -> bool {
        let Some((before, version, _, seen)) = self.verified.get_mut(path) else {
            return false;
        };
        if *before != stamp(stat) || version != entry {
            return false;
        }
        *seen = Instant::now();
        true
    }

    pub(crate) fn suggested(&mut self, path: &str, stat: Stat, proposal: &str) -> bool {
        let Some((before, _, captured, seen)) = self.verified.get_mut(path) else {
            return false;
        };
        if captured.is_empty() || *before != stamp(stat) || captured != proposal {
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
        .filter(|file| file.format == 2)
    else {
        return;
    };
    state.verified.extend(file.paths.into_iter().map(
        |(path, (stamp, [mode, kind, oid, proposal]))| {
            (
                path,
                (stamp, Entry { mode, kind, oid }, proposal, Instant::now()),
            )
        },
    ));
}

pub(crate) fn save(g: &Group) {
    let paths = g
        .lock()
        .capture_schedule
        .verified
        .iter()
        .map(|(path, (stamp, entry, proposal, _))| {
            (
                path.clone(),
                (
                    *stamp,
                    [
                        entry.mode.clone(),
                        entry.kind.clone(),
                        entry.oid.clone(),
                        proposal.clone(),
                    ],
                ),
            )
        })
        .collect();
    let result = serde_json::to_vec(&VerifiedFile { format: 2, paths })
        .map_err(std::io::Error::other)
        .and_then(|bytes| {
            crate::util::write_private(&g.git_dir.join("pigeon/capture-verified.json"), &bytes)
        });
    if let Err(error) = result {
        tracing::warn!(group = %g.id, %error, "capture-observation-cache-unavailable");
    }
}

/// Bind only an actual byte/stat observation to the exact signed entry.
/// A crash before this binding leaves the suggestion conservatively dirty.
pub(crate) fn bind_proposal(g: &Group, path: &str, proposal: &str, entry: &Entry) {
    let Some(wt) = &g.worktree else { return };
    let Ok(Some(stat)) = pigeon_sys::fs::lstat(&wt.join(path)) else {
        return;
    };
    let mut known = g.lock();
    let Some((before, captured, bound, _)) = known.capture_schedule.verified.get_mut(path) else {
        return;
    };
    if *before == stamp(stat) && captured == entry {
        *bound = proposal.to_owned();
    }
}

/// Wait briefly for the real kernel timestamp fence, never manufacture
/// an older stat or a newer index time. A future ctime stays unknown; the
/// final per-file comparison also lets other files in that batch progress.
async fn aged_epoch(latest: i64) -> Option<i64> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(50);
    loop {
        let epoch = pigeon_sys::clock::stat_epoch_ns().ok()?;
        if epoch > latest || tokio::time::Instant::now() >= deadline {
            return Some(epoch);
        }
        tokio::time::sleep_until(
            (tokio::time::Instant::now() + Duration::from_millis(1)).min(deadline),
        )
        .await;
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
        // The observed ctime must already be older than a kernel tick
        // before hashing, so a write during the read cannot reuse that tick.
        let Some(latest) = before.iter().flatten().map(|stat| stat.ctime_ns).max() else {
            continue;
        };
        let Some(epoch) = aged_epoch(latest).await else {
            continue;
        };
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
        for ((path, before), key) in chunk.iter().zip(before).zip(keys) {
            let Some(stat) = before else { continue };
            let Some(entry) = index.get(path) else {
                continue;
            };
            if !stat.is_regular()
                || stat.ctime_ns >= epoch
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
                (stamp(stat), entry.clone(), String::new(), Instant::now()),
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "native observation-fence regression")]
    use super::*;

    #[tokio::test]
    async fn a_future_ctime_cannot_make_the_fence_wait_without_a_bound() {
        let epoch = tokio::time::timeout(Duration::from_secs(1), aged_epoch(i64::MAX))
            .await
            .unwrap()
            .unwrap();
        assert!(
            epoch < i64::MAX,
            "a future stamp must still fail the per-file age check"
        );
    }

    #[tokio::test]
    async fn a_verified_observation_does_not_need_a_newer_index_timestamp() {
        let temp = tempfile::tempdir().unwrap();
        let env = pigeon_git::GitEnv::default();
        let git = crate::git::Git::new(temp.path(), env.clone());
        if git.run(&["annex", "version", "--raw"]).await.is_err() {
            assert!(std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none());
            return;
        }
        git.run(&["init", "-q", "-b", "main"]).await.unwrap();
        git.run(&["config", "user.name", "Observation fixture"])
            .await
            .unwrap();
        git.run(&["config", "user.email", "observation@pigeon"])
            .await
            .unwrap();
        git.run(&["annex", "init", "observation"]).await.unwrap();
        let g = Group::open(&"a".repeat(40), temp.path(), &env)
            .await
            .unwrap();
        crate::settings::check(&g).await;
        let file = temp.path().join("notes.bin");
        std::fs::write(&file, b"verified bytes\n").unwrap();
        let trace = crate::trace::Tracer::new(temp.path().join("traces"), "observation");
        crate::capture::capture(&g, "observation", &trace)
            .await
            .unwrap();
        let index = g.git.index().await.unwrap();
        for _ in 0..8 {
            // A fresh native ctime models annex's final hardlink removal.
            // Every observation must finish its proof; this is not a retry
            // until one invocation happens to cross a clock tick.
            std::fs::set_permissions(&file, std::fs::metadata(&file).unwrap().permissions())
                .unwrap();
            let stat = pigeon_sys::fs::lstat(&file).unwrap().unwrap();
            let stamp =
                std::time::UNIX_EPOCH + Duration::from_nanos(u64::try_from(stat.ctime_ns).unwrap());
            let index_file = g.git_dir.join("index");
            std::fs::File::options()
                .write(true)
                .open(&index_file)
                .unwrap()
                .set_modified(stamp)
                .unwrap();
            g.lock().capture_schedule.verified.clear();
            checked(&g, &["notes.bin".to_owned()], &index).await;
            assert!(
                g.lock()
                    .capture_schedule
                    .verified("notes.bin", stat, &index["notes.bin"])
            );
            assert_eq!(
                std::fs::metadata(index_file).unwrap().modified().unwrap(),
                stamp,
                "the observation must not mutate the index to manufacture an age fence"
            );
        }
    }
}
