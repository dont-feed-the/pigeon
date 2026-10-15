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

use crate::api::{problem, relative};
use crate::daemon::Daemon;
use crate::group::Group;
use crate::install::Put;

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
    let Some(rel) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let _busy = g.busy.lock().await;
    if let Err(e) = crate::capture::capture(&g, &d.device, &d.tracer).await {
        tracing::warn!(group = %g.id, error = %e, "capture-failed");
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
    let n = version.unwrap_or(1);
    let wanted = usize::try_from(n)
        .ok()
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| versions.get(i));
    let Some((_, key)) = wanted.cloned() else {
        let args = [("path", text(&rel)), ("version", Arg::Number(n as i64))];
        return vec![problem("no-such-version", &args, None)];
    };
    // Installing copies the content even when its object is here, so the
    // restore must fit above the reserve either way (S8).
    if !fits(&g, &key) {
        let me = crate::status::this_name(&g, &d.device);
        return vec![problem("disk-full", &[("machine", text(&me))], None)];
    }
    match bring(&g, &key).await {
        Ok(()) => {}
        Err("disk-full") => {
            let me = crate::status::this_name(&g, &d.device);
            return vec![problem("disk-full", &[("machine", text(&me))], None)];
        }
        Err(code) => return vec![problem(code, &[], None)],
    }
    let current = versions.last().map(|(_, k)| k.clone());
    match crate::install::put(&g, &rel, &key, current.as_deref(), None, &d.tracer).await {
        Ok(Put::Done | Put::Raced) => {}
        Ok(Put::InUse) => {
            let e = "the file is being written; try again".to_owned();
            return vec![problem("internal-error", &[], Some(e))];
        }
        Err(e) => return vec![problem("internal-error", &[], Some(e))],
    }
    if let Err(e) = crate::capture::capture(&g, &d.device, &d.tracer).await {
        tracing::warn!(group = %g.id, error = %e, "capture-failed");
    }
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
    if crate::custody::present(g, key).await.is_some() {
        return Ok(());
    }
    if !fits(g, key) {
        return Err("disk-full");
    }
    let args = ["annex", "get", "--json", "--key", key];
    g.git
        .run_for(&args, Duration::from_secs(600))
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
