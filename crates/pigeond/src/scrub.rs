//! Background integrity passes (08 §8.8, ADR 0037). A durable inventory
//! advances through idle-priority `fsck --incremental/--more --key` calls.
//! No worktree lock is held. Unknown ledgers are preserved. Only a fully
//! acknowledged inventory publishes a pass time; normal placement repairs
//! content git-annex moved out after a failed integrity check.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pigeon_core::filter::Role;
use pigeon_core::scrub::{Pass, Window};
use serde::{Deserialize, Serialize};

use crate::daemon::Daemon;
use crate::group::Group;

const PERIOD: u64 = 30 * 86_400;
const SLICE: Duration = Duration::from_secs(5);
const MAX_KEYS: usize = 16;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cycle {
    started: i64,
    pending: BTreeSet<String>,
    bad: BTreeSet<String>,
    incremental_started: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileT {
    format: u32,
    last_pass_completed: Option<i64>,
    keys_failed: u64,
    cycle: Option<Cycle>,
}

impl Default for FileT {
    fn default() -> Self {
        Self {
            format: 1,
            last_pass_completed: None,
            keys_failed: 0,
            cycle: None,
        }
    }
}

impl FileT {
    fn read(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let file: Self = toml_edit::de::from_str(&text).map_err(|e| e.to_string())?;
                if file.format != 1
                    || file.cycle.as_ref().is_some_and(|c| {
                        !c.pending.is_disjoint(&c.bad)
                            || c.pending
                                .iter()
                                .chain(&c.bad)
                                .any(|k| !crate::net::sync::plausible_key(k))
                    })
                {
                    return Err("unknown or inconsistent scrub ledger".into());
                }
                Ok(file)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml_edit::ser::to_string(self).map_err(|e| e.to_string())?;
        pigeon_sys::fs::write_atomic(path, text.as_bytes()).map_err(|e| e.to_string())
    }

    fn report(&self) -> Option<crate::report_file::Scrub> {
        self.last_pass_completed
            .map(|at| crate::report_file::Scrub {
                last_pass_completed: crate::report_file::datetime(at),
                keys_failed: self.keys_failed,
            })
    }
}

/// Restore only the last completed pass, never a half-finished inventory.
pub(crate) fn restore(g: &Group) {
    if let Ok(file) = FileT::read(&g.root().join("scrub.toml")) {
        g.lock().scrub_report = file.report();
    }
}

/// Read only the durable inventory. Socket and health reads do not start a
/// pass, advance it, replace an unknown ledger or invent a completion time.
pub(crate) fn status(g: &Group) -> Result<pigeon_proto::health::Scrub, String> {
    let file = FileT::read(&g.root().join("scrub.toml"))?;
    Ok(pigeon_proto::health::Scrub {
        last_pass_completed: file.last_pass_completed.map(crate::util::rfc3339),
        keys_failed: file.keys_failed,
        remaining: file.cycle.as_ref().map(|c| c.pending.len() as u64),
        current_failures: file.cycle.as_ref().map(|c| c.bad.len() as u64),
        problem: None,
    })
}

fn children(path: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    match std::fs::read_dir(path) {
        Ok(entries) => entries
            .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}

/// git-annex 10.20261006 Locations.hs uses objects/hash/hash/key/key.
/// Enumerating this inventory avoids one child process per retained key.
fn local_keys(g: &Group) -> Result<BTreeSet<String>, String> {
    let mut keys = BTreeSet::new();
    for a in children(&g.git_dir.join("annex/objects"))? {
        for b in children(&a)? {
            for dir in children(&b)? {
                let Some(key) = dir.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if crate::net::sync::plausible_key(key) && dir.join(key).is_file() {
                    keys.insert(key.to_owned());
                }
            }
        }
    }
    Ok(keys)
}

fn in_window(window: &str, now: i64) -> Result<bool, String> {
    let window = Window::parse(window).ok_or("resources.scrub_window must be HH:MM-HH:MM")?;
    let offset = i64::from(pigeon_sys::clock::local_offset_minutes(now)) * 60;
    let local = (i128::from(now) + i128::from(offset)).rem_euclid(86_400) / 60;
    Ok(window.contains(u16::try_from(local).unwrap_or(0)))
}

/// One maintenance slice. The caller serializes slices, independently of
/// worktree requests. A single active object's hash may outlast the slice;
/// the idle child has a separate hard deadline and never holds `busy`.
pub async fn slice(g: &Group, now: i64) -> Result<(), String> {
    if g.role != Role::Custodian || g.git.purge_gate.policy().waiting {
        return Ok(());
    }
    let space = pigeon_sys::fs::space(&g.git_dir).map_err(|e| e.to_string())?;
    if space.available < crate::settings::reserve_bytes(g.role, &g.git_dir) {
        return Ok(());
    }
    let path = g.root().join("scrub.toml");
    let mut file = FileT::read(&path)?;
    if file.cycle.is_none() {
        if !pigeon_core::scrub::due(now, file.last_pass_completed, PERIOD) {
            return Ok(());
        }
        let retained = crate::retention::keys(g).await.map_err(|e| e.to_string())?;
        let pending = local_keys(g)?.intersection(&retained).cloned().collect();
        file.cycle = Some(Cycle {
            started: now,
            pending,
            bad: BTreeSet::new(),
            incremental_started: false,
        });
        file.save(&path)?;
    }
    let started = Instant::now();
    for _ in 0..MAX_KEYS {
        let cycle = file.cycle.as_mut().ok_or("missing scrub cycle")?;
        let Some(key) = cycle.pending.first().cloned() else {
            break;
        };
        let policy = g.git.purge_gate.policy();
        if policy.waiting || started.elapsed() >= SLICE {
            break;
        }
        let mut pass = Pass {
            pending: std::mem::take(&mut cycle.pending),
            bad: std::mem::take(&mut cycle.bad),
        };
        if !policy.released.contains(&key) && crate::custody::present(g, &key).await.is_some() {
            let incremental = if cycle.incremental_started {
                "--more"
            } else {
                "--incremental"
            };
            // Fsck.hs: prepIncremental starts/continues its durable checked-
            // key database; runFsck records only successful checks. The
            // time limit stops starting more work, not an active hash.
            let (status, _) = g
                .git
                .repo
                .run_idle(
                    &[
                        "annex",
                        "fsck",
                        "--json",
                        incremental,
                        "--time-limit=5s",
                        "--key",
                        &key,
                    ],
                    Duration::from_secs(600),
                )
                .await
                .map_err(|e| e.to_string())?;
            if status != Some(0) {
                if g.git.purge_gate.policy().released.contains(&key) {
                    pass.checked(key.clone(), false);
                } else if crate::custody::present(g, &key).await.is_none() {
                    pass.checked(key.clone(), true);
                } else {
                    // A lock, unavailable proof or I/O error is not evidence
                    // that this object was checked. Keep it pending.
                    return Err("scrub key check did not complete".into());
                }
            } else {
                pass.checked(key.clone(), false);
            }
            cycle.incremental_started = true;
        } else {
            // An inventory object disappearing unexpectedly is a finding,
            // including a crash after fsck moved it but before our save.
            // A qualified purge deliberately leaves the inventory instead.
            pass.checked(key.clone(), !policy.released.contains(&key));
        }
        cycle.pending = pass.pending;
        cycle.bad = pass.bad;
        file.save(&path)?;
    }
    if file.cycle.as_ref().is_some_and(|c| c.pending.is_empty()) {
        let cycle = file.cycle.take().ok_or("missing completed cycle")?;
        file.last_pass_completed = Some(crate::util::now().max(cycle.started));
        file.keys_failed = u64::try_from(cycle.bad.len()).unwrap_or(u64::MAX);
        file.save(&path)?;
        g.lock().scrub_report = file.report();
    }
    Ok(())
}

/// Separate from capture rounds so an active large-object hash cannot
/// delay capture or the socket. Configuration is read again for each slice.
pub async fn run(d: Arc<Daemon>) {
    loop {
        match crate::config::load(&d.dirs.config_file()) {
            Ok(config) => match in_window(&config.resources.scrub_window, crate::util::now()) {
                Ok(true) if crate::pause::now(&d).is_none() => {
                    for g in d.groups() {
                        if crate::pause::now(&d).is_some() {
                            break;
                        }
                        if crate::membership::pending(&g, &d.device)
                            || g.lock().admission_error.is_some()
                        {
                            continue;
                        }
                        if let Err(e) = slice(&g, crate::util::now()).await {
                            tracing::warn!(group = %g.id, error = e, "scrub-waits");
                        }
                    }
                }
                Err(e) => tracing::warn!(error = e, "scrub-settings-refused"),
                _ => {}
            },
            Err(e) => tracing::warn!(error = %e, "scrub-settings-refused"),
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
