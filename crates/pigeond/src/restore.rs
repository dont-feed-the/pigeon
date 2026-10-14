//! `pigeon restore PATH --version N` (10 §10.1, 05 §5.1): an older version
//! of a file becomes its current one, as an install (07 §7.5) that capture
//! then records as a new version naming the old key. The newer version
//! stays in the history.
//!
//! Versions are numbered from 1, the oldest, as `history` numbers them. A
//! version this laptop no longer holds is received first, and refused with
//! `disk-full` before anything starts when it would not fit above the
//! laptop's `annex.diskreserve` (08 §8.9, S8).

use std::path::{Path, PathBuf};
use std::time::Duration;

use pigeon_core::journal::{Begin, End, Record};
use pigeon_proto::Response;
use pigeon_proto::api::Arg;
use pigeon_sys::fs::{Renamed, lstat, rename_noreplace};

use crate::api::{problem, relative};
use crate::daemon::Daemon;
use crate::evict::{is_old, journal, next_seq, record, stat_of, truncate_if_done};
use crate::group::Group;
use crate::journal_file::read;
use crate::trace::{Act, Tracer};

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

/// `pigeon restore PATH --version N`.
pub async fn restore(d: &Daemon, path: &Path, version: Option<u64>) -> Vec<Response> {
    let Some(g) = d.group_at(path).or_else(|| d.group_at(path.parent()?)) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(rel) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let _busy = g.busy.lock().await;
    if let Err(e) = crate::capture::capture(&g, &d.tracer).await {
        tracing::warn!(group = %g.id, error = %e, "capture-failed");
    }
    let mut versions = crate::history::versions(&g, &rel).await;
    versions.reverse();
    let n = version.unwrap_or(1);
    let wanted = usize::try_from(n)
        .ok()
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| versions.get(i));
    let Some((_, key)) = wanted.cloned() else {
        let args = [("path", text(&rel)), ("version", Arg::Number(n as i64))];
        return vec![problem("no-such-version", &args, None)];
    };
    match bring(&g, &key).await {
        Ok(()) => {}
        Err("disk-full") => {
            let me = this_name(d, &g);
            return vec![problem("disk-full", &[("machine", text(&me))], None)];
        }
        Err(code) => return vec![problem(code, &[], None)],
    }
    let current = versions.last().map(|(_, k)| k.clone());
    match install(&g, &rel, &key, current.as_deref(), &d.tracer).await {
        Ok(()) => {}
        Err(e) => return vec![problem("internal-error", &[], Some(e))],
    }
    if let Err(e) = crate::capture::capture(&g, &d.tracer).await {
        tracing::warn!(group = %g.id, error = %e, "capture-failed");
    }
    d.wake.notify_one();
    vec![Response::Said {
        message: crate::status::msg("restore-result", &[("path", text(&rel))]),
    }]
}

fn this_name(d: &Daemon, g: &Group) -> String {
    g.roster()
        .and_then(|doc| {
            let id = pigeon_core::roster::DeviceId(d.device.clone());
            doc.roster.devices.get(&id).map(|x| x.name.clone())
        })
        .unwrap_or_else(|| "This computer".to_owned())
}

/// Makes `key`'s content present here: answers a problem code if it does
/// not fit above the reserve, or cannot be received.
async fn bring(g: &Group, key: &str) -> Result<(), &'static str> {
    if crate::custody::present(g, key).await.is_some() {
        return Ok(());
    }
    let need = key_size(key).unwrap_or(0) + crate::settings::reserve_bytes(g.role);
    let room = pigeon_sys::fs::space(&g.git_dir).map(|s| s.available);
    if room.is_ok_and(|room| room < need) {
        tracing::info!(group = %g.id, key = %key, need, "restore-disk-full");
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

/// Installs `key`'s content at `rel`, moving the current file aside first
/// if there is one; a file saved meanwhile wins (07 §7.5).
async fn install(
    g: &Group,
    rel: &str,
    key: &str,
    current: Option<&str>,
    t: &Tracer,
) -> Result<(), String> {
    let wt = g.worktree.clone().ok_or("not-in-group")?;
    let object = crate::custody::present(g, key).await.ok_or("receiving")?;
    let file = wt.join(rel);
    let seq = next_seq(&read(&journal(g)));
    let stage = g.files.stage(seq);
    let aside: PathBuf = g.files.aside(seq);
    std::fs::copy(&object, &stage).map_err(|e| e.to_string())?;
    if let Ok(f) = std::fs::File::open(&stage) {
        let _ = f.sync_all();
    }
    let st = stat_of(&file);
    record(
        g,
        &Record::Begin(Begin {
            seq,
            path: rel.to_owned(),
            old_key: st.and(current.map(str::to_owned)),
            new_key: Some(key.to_owned()),
            stat: st,
            stage: Some(stage.to_string_lossy().into_owned()),
            aside: aside.to_string_lossy().into_owned(),
        }),
    )?;
    t.event(Act::Start, &[("path", rel)]);
    if st.is_some() {
        if rename_noreplace(&file, &aside).map_err(|e| e.to_string())? != Renamed::Done {
            record(g, &Record::End(seq, End::Raced))?;
            let _ = std::fs::remove_file(&stage);
            return Ok(());
        }
        t.event(Act::MoveAside, &[("path", rel)]);
        let clear = lstat(&aside)
            .ok()
            .flatten()
            .is_some_and(|a| pigeon_sys::writers::scan(a.id).is_clear());
        t.event(Act::Scan, &[("path", rel)]);
        if !clear || stat_of(&aside) != st || !is_old(&aside, current) {
            let end = match rename_noreplace(&aside, &file).map_err(|e| e.to_string())? {
                Renamed::Done | Renamed::SourceMissing => End::Skip,
                Renamed::TargetExists => End::Recovered,
            };
            record(g, &Record::End(seq, end))?;
            let _ = std::fs::remove_file(&stage);
            return Err("the file is being written; try again".to_owned());
        }
    } else if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    record(g, &Record::Proved(vec![seq]))?;
    t.event(Act::Check, &[("path", rel)]);
    if rename_noreplace(&stage, &file).map_err(|e| e.to_string())? != Renamed::Done {
        record(g, &Record::End(seq, End::Raced))?;
        let _ = std::fs::remove_file(&stage);
        return Ok(());
    }
    t.event(Act::RenameIn, &[("path", rel)]);
    record(g, &Record::End(seq, End::Done))?;
    t.event(Act::Done, &[("path", rel)]);
    let _ = std::fs::remove_file(&aside);
    record(g, &Record::Released(seq))?;
    t.event(Act::Release, &[("path", rel)]);
    truncate_if_done(g);
    Ok(())
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
}
