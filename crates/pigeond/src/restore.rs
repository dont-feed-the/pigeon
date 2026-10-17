//! `pigeon restore PATH --version N | --at TIME` (10 §10.1, 05 §5.1): an
//! older version
//! of a file becomes its current one, as an install (07 §7.5) that capture
//! then records as a new version naming the old key. The newer version
//! stays in the history. `--at` takes the newest version saved at or
//! before TIME, RFC 3339 or a date (its end, UTC) **[no ADR]**.
//!
//! Versions are numbered from 1, the oldest, as `history` numbers them. A
//! version this laptop no longer holds is received first. Installing copies
//! it, so a restore that would not fit above the laptop's
//! `annex.diskreserve` is refused with `disk-full` before anything starts,
//! held here or not (08 §8.9, S8).

use std::path::Path;
use std::time::Duration;

use pigeon_proto::Response;
use pigeon_proto::api::Arg;

use crate::api::problem;
use crate::daemon::Daemon;
use crate::group::Group;
use crate::install::Put;

mod folder;
mod guard;
pub(crate) mod path;
mod reference;

fn text(s: &str) -> Arg {
    Arg::Text(s.to_owned())
}

/// The size in bytes a git-annex key names (`…-s<size>--…`).
pub fn key_size(key: &str) -> Option<u64> {
    let fields = key.split_once("--")?.0;
    fields
        .split('-')
        .find_map(|f| f.strip_prefix('s'))
        .and_then(|n| n.parse().ok())
}

/// The seconds `at` names: an RFC 3339 time, or the end of a date (UTC).
pub fn instant(at: &str) -> Option<i64> {
    let at = at.trim();
    pigeon_proto::time::parse_rfc3339(at).or_else(|| {
        (at.len() == 10)
            .then(|| pigeon_proto::time::parse_rfc3339(&format!("{at}T23:59:59Z")))
            .flatten()
    })
}

/// The version number, from 1 the oldest, of the newest of `versions`
/// (oldest first, by commit) saved at or before `t`.
async fn number_at(g: &Group, versions: &[(String, String)], t: i64) -> Option<u64> {
    let mut found = None;
    for (i, (commit, _)) in versions.iter().enumerate() {
        let raw = g.git.object(commit).await.ok()?;
        let (secs, _) = pigeon_core::authority::author_time(&raw)?;
        if secs <= t {
            found = Some(i as u64 + 1);
        }
    }
    found
}

/// `pigeon restore PATH --version N | --at TIME`.
pub async fn restore(
    d: &Daemon,
    path: &Path,
    version: Option<u64>,
    at: Option<&str>,
) -> Vec<Response> {
    let Some(g) = d.group_at(path).or_else(|| d.group_at(path.parent()?)) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(rel) = self::path::relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let busy = g.busy.lock().await;
    if let Err(e) = crate::capture::capture(&g, &d.device, &d.tracer).await {
        return vec![problem("internal-error", &[], Some(e.to_string()))];
    }
    if folder::is_folder(&g, &rel).await {
        return folder::restore(d, &g, &rel, version, at, busy).await;
    }
    let mut versions = crate::history::versions(&g, &rel).await;
    versions.reverse();
    let version = match at {
        None => version,
        Some(at) => {
            let found = match instant(at) {
                Some(t) => number_at(&g, &versions, t).await,
                None => None,
            };
            if found.is_none() {
                let args = [("path", text(&rel)), ("at", text(at))];
                return vec![problem("restore-at", &args, None)];
            }
            found
        }
    };
    let n = version.unwrap_or(versions.len().try_into().unwrap_or(u64::MAX));
    let wanted = usize::try_from(n)
        .ok()
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| versions.get(i));
    let Some((commit, key)) = wanted.cloned() else {
        let args = [("path", text(&rel)), ("version", Arg::Number(n as i64))];
        return vec![problem("no-such-version", &args, None)];
    };
    // Installing copies the content even when its object is here, so the
    // restore must fit above the reserve either way (S8).
    if !fits(&g, &key) {
        let me = crate::status::this_name(&g, &d.device);
        return vec![problem("disk-full", &[("machine", text(&me))], None)];
    }
    // A prior missing-content request already has a new history entry but
    // has not changed the visible file. Freeze capture's actual index here.
    let current = match g.git.index().await {
        Ok(index) => match index.get(&rel) {
            Some(entry) => match g.git.object(&entry.oid).await {
                Ok(bytes) => crate::git::pointer_key(&bytes),
                Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
            },
            None => None,
        },
        Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
    };
    let entry = g
        .git
        .tree(&commit)
        .await
        .ok()
        .and_then(|t| t.get(&rel).cloned());
    let Some(entry) = entry else {
        return vec![problem("internal-error", &[], None)];
    };
    if !self::path::parents_clear(&g, &rel) {
        return vec![problem("restore-in-use", &[("path", text(&rel))], None)];
    }
    let absent = crate::custody::present(&g, &key).await.is_none();
    let (intent, before) = {
        let Some(worktree) = &g.worktree else {
            return vec![problem("not-in-group", &[], None)];
        };
        let file = worktree.join(&rel);
        let Some(before) =
            guard::Before::saved(&g, &d.device, &rel, current.as_deref(), &key).await
        else {
            return vec![problem("restore-in-use", &[("path", text(&rel))], None)];
        };
        if !before.clear(&file) {
            return vec![problem("restore-in-use", &[("path", text(&rel))], None)];
        }
        if crate::capture_file::pending(&g) || crate::evict::unsettled(&g) {
            return vec![problem("receiving", &[], None)];
        }
        match reference::retain(&g, &d.device, [(rel.clone(), Some(entry.clone()))].into()).await {
            Ok(intent) => (intent, before),
            Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
        }
    };
    // Complete the new exact reference reading without blocking other
    // worktree requests on a cold scan. Refounding may replace this group.
    drop(busy);
    crate::purge::prepare(&g).await;
    let _busy = g.busy.lock().await;
    if !d.current_group(&g).await {
        return vec![problem("not-in-group", &[], None)];
    }
    crate::purge::round(&g, &d.device, false).await;
    match bring(&g, &key).await {
        Ok(()) => {}
        Err("disk-full") => {
            let me = crate::status::this_name(&g, &d.device);
            return vec![problem("disk-full", &[("machine", text(&me))], None)];
        }
        Err("receiving") if absent => {
            d.wake.notify_one();
            return vec![problem("restore-waiting", &[("path", text(&rel))], None)];
        }
        Err(code) => return vec![problem(code, &[], None)],
    }
    if !self::path::parents_clear(&g, &rel) {
        return vec![problem("restore-in-use", &[("path", text(&rel))], None)];
    }
    // Another round may have completed this exact request. Otherwise put
    // must compare with the original file, even if capture indexed a newer
    // save while the cold scan ran. Never bless a newly read index baseline.
    let already = g
        .worktree
        .as_ref()
        .is_some_and(|w| guard::verified(&w.join(&rel), &key, &entry.mode).is_some());
    if !already {
        match crate::install::put_restore(&g, &rel, &key, before.source(), &entry.mode, &d.tracer)
            .await
        {
            Ok(Put::Done(_)) => {}
            Ok(Put::InUse | Put::Raced) => {
                return vec![problem("restore-in-use", &[("path", text(&rel))], None)];
            }
            Err(e) => return vec![problem("internal-error", &[], Some(e))],
        }
    }
    if let Err(e) = intent.installed(&g, &d.device, &[rel.clone()].into()).await {
        return vec![problem("internal-error", &[], Some(e.to_string()))];
    }
    if let Err(e) = crate::capture::capture(&g, &d.device, &d.tracer).await {
        tracing::warn!(group = %g.id, error = %e, "capture-failed");
    }
    // A normal round may have installed this retained request while its
    // cold policy scan ran. Observe the actual completed postcondition in
    // both paths; a successful restore need not start another install.
    let verified = crate::trace_state::Span::begin_request(
        &d.tracer,
        &g,
        &rel,
        0,
        ("VerifyRestore", "restore"),
        Some(&key),
    )
    .await;
    verified.finish("idle").await;
    d.wake.notify_one();
    vec![Response::Said {
        message: crate::status::msg("restore-result", &[("path", text(&rel))]),
    }]
}

/// Whether `key`'s content fits here above the reserve.
fn fits(g: &Group, key: &str) -> bool {
    let need = key_size(key).unwrap_or(0) + crate::settings::reserve_bytes(g.role, &g.git_dir);
    let room = pigeon_sys::fs::space(&g.git_dir).map(|s| s.available);
    if room.is_ok_and(|room| room < need) {
        tracing::info!(group = %g.id, key = %key, need, "restore-disk-full");
        return false;
    }
    true
}

/// Makes `key`'s content present here: answers a problem code if it does
/// not fit above the reserve, or cannot be received.
pub(crate) async fn bring(g: &Group, key: &str) -> Result<(), &'static str> {
    if available(g, key).await? {
        return Ok(());
    }
    if !fits(g, key) {
        return Err("disk-full");
    }
    receive(g, key).await
}

/// Resolve local/provider objects without waiting on a network transfer
/// while a daemon round holds the worktree lock.
pub(crate) async fn available(g: &Group, key: &str) -> Result<bool, &'static str> {
    if crate::custody::present(g, key).await.is_some() {
        crate::shared::adopt(g, key)
            .await
            .map_err(|_| "receiving")?;
        return Ok(true);
    }
    if g.shared.is_some()
        && crate::shared::hydrate(g, key)
            .await
            .map_err(|_| "receiving")?
    {
        return Ok(true);
    }
    Ok(false)
}

async fn receive(g: &Group, key: &str) -> Result<(), &'static str> {
    let args = ["annex", "get", "--json", "--key", key];
    let (status, output) = g
        .git
        .repo
        .run_status(&args, None, Duration::from_secs(600))
        .await
        .map_err(|error| {
            tracing::debug!(group = %g.id, key, %error, "receive-waits");
            "receiving"
        })?;
    if status != Some(0) {
        tracing::debug!(group = %g.id, key, ?status, stdout = %output.text(),
            stderr = %String::from_utf8_lossy(&output.stderr), "receive-waits");
        return Err("receiving");
    }
    crate::shared::adopt(g, key)
        .await
        .map_err(|_| "receiving")?;
    crate::custody::present(g, key)
        .await
        .map(drop)
        .ok_or("receiving")
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_key_names_its_size() {
        assert_eq!(
            super::key_size("SHA256E-s3145728--ab12.raw"),
            Some(3_145_728)
        );
        assert_eq!(super::key_size("SHA256-s7--ab"), Some(7));
        assert_eq!(super::key_size("WORM-m1-s0--x"), Some(0));
        assert_eq!(super::key_size("nonsense"), None);
    }

    #[test]
    fn a_time_or_a_date_names_an_instant() {
        assert_eq!(super::instant("1970-01-02"), Some(86_400 + 86_399));
        assert_eq!(super::instant("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(super::instant("yesterday"), None);
    }
}
