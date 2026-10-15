//! The complete aside lifecycle (07 §7.5): keep across restarts, check
//! writers and content at release, and capture changed content before
//! releasing its journal entry. Recovery names never replace older saves.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pigeon_core::authority::Entry as FileEntry;
use pigeon_core::journal::{Entry, Record, Release, entries, release as decision};
use pigeon_core::names;
use pigeon_sys::fs::{Renamed, fsync_dir, lstat, rename_noreplace};

use crate::evict::{is_old, journal, record, truncate_if_done};
use crate::group::Group;
use crate::trace::{Act, Tracer};

/// The minimum keep period of an install's displaced inode, in seconds.
pub const KEEP_SECONDS: i64 = 10 * 60;

/// A complete-enough writer scan: unreadable processes are unsafe when
/// another user could write the inode (07 §7.5 step 5).
pub(crate) fn writers_clear(st: &pigeon_sys::fs::Stat) -> bool {
    let scan = pigeon_sys::writers::scan(st.id);
    scan.is_clear() && (scan.unreadable == 0 || st.mode & 0o022 == 0)
}

fn source(e: &Entry) -> Result<PathBuf, String> {
    let aside = PathBuf::from(&e.begin.aside);
    // Preserved is written before rename. If the source still exists,
    // the move did not finish; a collision at the destination is not ours.
    if lstat(&aside).map_err(|e| e.to_string())?.is_some() {
        return Ok(aside);
    }
    if let Some(path) = &e.preserved {
        let path = PathBuf::from(path);
        if lstat(&path).map_err(|e| e.to_string())?.is_none() {
            return Err("preserved save is missing; keep its recovery intent".to_owned());
        }
        return Ok(path);
    }
    Ok(aside)
}

/// Release eligible ended entries, after the keep period. This also runs
/// in ordinary rounds, so a retained entry need not wait for a restart.
pub async fn release(g: &Group, t: &Tracer) {
    release_at(g, t, crate::util::now()).await;
}

/// Release at `now` (Unix seconds). Separating time lets regressions drive
/// complete keep/recovery/release lifecycles without sleeping ten minutes.
pub async fn release_at(g: &Group, t: &Tracer, now: i64) {
    if crate::capture_file::pending(g) || crate::evict::unsettled(g) {
        return;
    }
    for (seq, e) in entries(&crate::journal_file::read(&journal(g))) {
        if e.end.is_none() || e.released {
            continue;
        }
        if let Err(error) = one(g, t, now, seq, &e).await {
            tracing::warn!(group = %g.id, seq, %error, "aside-release-waits");
        }
    }
    truncate_if_done(g);
}

async fn one(g: &Group, t: &Tracer, now: i64, seq: u64, e: &Entry) -> Result<(), String> {
    let file = source(e)?;
    let Some(st) = lstat(&file).map_err(|e| e.to_string())? else {
        return record(g, &Record::Released(seq));
    };
    let Some(at) = e.kept_at else {
        // Older journals and a crash between End and Kept start a full
        // keep period now, never infer it from the old file's mtime.
        return record(g, &Record::Kept(seq, now));
    };
    let long_enough = now.saturating_sub(at) >= KEEP_SECONDS;
    if !long_enough {
        return Ok(());
    }
    let written = !writers_clear(&st);
    let old = long_enough && !written && is_old(&file, e.begin.old_key.as_deref());
    match decision(long_enough, written, false, old) {
        Release::Wait => return Ok(()),
        Release::Unlink if e.preserved.is_none() => {
            std::fs::remove_file(&file).map_err(|e| e.to_string())?;
            sync_parent(&file)?;
        }
        _ => {
            let kept = preserve(g, seq, e, &file)?;
            let (name, key, commit) = capture(g, e, &kept, at).await?;
            let meta = serde_json::json!({
                "format": 1, "path": e.begin.path, "captured_as": name,
                "key": key, "commit": commit,
            });
            pigeon_sys::fs::write_atomic(&kept.with_extension("json"), meta.to_string().as_bytes())
                .map_err(|e| e.to_string())?;
        }
    }
    record(g, &Record::Released(seq))?;
    t.event(Act::Release, &[("path", &e.begin.path)]);
    Ok(())
}

fn sync_parent(file: &Path) -> Result<(), String> {
    fsync_dir(file.parent().ok_or("no parent")?).map_err(|e| e.to_string())
}

fn preserve(g: &Group, seq: u64, e: &Entry, file: &Path) -> Result<PathBuf, String> {
    if e.preserved.as_ref().is_some_and(|p| Path::new(p) == file) {
        return Ok(file.to_path_buf());
    }
    let mut n = 0;
    loop {
        let name = if n == 0 {
            seq.to_string()
        } else {
            format!("{seq}-{n}")
        };
        let dest = g.files.recovered().join(name);
        n += 1;
        if dest.exists() || dest.with_extension("json").exists() {
            continue;
        }
        record(
            g,
            &Record::Preserved(seq, dest.to_string_lossy().into_owned()),
        )?;
        match rename_noreplace(file, &dest).map_err(|e| e.to_string())? {
            Renamed::Done => {
                sync_parent(file)?;
                sync_parent(&dest)?;
                return Ok(dest);
            }
            Renamed::TargetExists => continue,
            Renamed::SourceMissing => {
                return Err("aside disappeared before preservation".to_owned());
            }
        }
    }
}

/// Capture an immutable snapshot under a deterministic conflict name.
/// Normal authority splitting decides whether it is accepted or proposed.
/// The preserved file remains available even if any Git operation fails.
async fn capture(
    g: &Group,
    e: &Entry,
    file: &Path,
    at: i64,
) -> Result<(String, String, String), String> {
    let me = crate::git::signer_device().ok_or("no signing device")?;
    let snapshot = g.files.stage(e.begin.seq).with_extension("recovered");
    let before = crate::evict::stat_of(file).ok_or("aside unreadable")?;
    std::fs::copy(file, &snapshot).map_err(|e| e.to_string())?;
    if crate::evict::stat_of(file) != Some(before) {
        return Err("aside changed while snapshotting".to_owned());
    }
    let path = snapshot.to_str().ok_or("non-UTF-8 snapshot path")?;
    let key = g
        .git
        .run(&["annex", "calckey", path])
        .await
        .map_err(|e| e.to_string())?;
    let key = key.trim().to_owned();
    g.git
        .run(&["annex", "setkey", &key, path])
        .await
        .map_err(|e| e.to_string())?;
    let object = crate::custody::present(g, &key)
        .await
        .ok_or("snapshot not stored")?;
    crate::net::sync::sync_object(&object).map_err(|e| e.to_string())?;
    let pointer = format!("/annex/objects/{key}\n");
    let oid = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], pointer.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let exec = lstat(file)
        .map_err(|e| e.to_string())?
        .is_some_and(|s| s.is_executable());
    let entry = FileEntry {
        mode: if exec { "100755" } else { "100644" }.to_owned(),
        kind: "blob".to_owned(),
        oid: oid.trim().to_owned(),
    };
    let main = g.git.rev("refs/heads/main").await.ok_or("no main")?;
    let tree = g.git.tree(&main).await.map_err(|e| e.to_string())?;
    let mut known = vec![(main.clone(), tree.clone())];
    for (_, c) in crate::proposal::kept(&g.git).await {
        known.push((c.clone(), g.git.tree(&c).await.map_err(|e| e.to_string())?));
    }
    let taken: BTreeSet<String> = tree.keys().map(|p| names::fold(p)).collect();
    let when = names::Stamp { utc: at, offset: 0 };
    let mut n = 1;
    let name = loop {
        let name = names::conflict_name(&e.begin.path, "recovered", when, n);
        if let Some((commit, _)) = known.iter().find(|(_, t)| t.get(&name) == Some(&entry)) {
            return Ok((name, key, commit.clone()));
        }
        if !taken.contains(&names::fold(&name)) {
            break name;
        }
        n += 1;
    };
    let changes = BTreeMap::from([(name.clone(), Some(entry))]);
    let empty = crate::conflicts_file::Table::default();
    let (allowed, proposals) = crate::suggest::plan(g, &me, &main, &tree, &empty, changes)
        .await
        .map_err(|e| e.to_string())?;
    let commit = if !allowed.is_empty() {
        let tree = crate::suggest::applied(&tree, &allowed);
        let oid = crate::merge::write_tree(&g.git, &tree)
            .await
            .map_err(|e| e.to_string())?;
        let commit = g
            .git
            .commit_tree(&oid, &[&main], "pigeon: recover saved content")
            .await
            .map_err(|e| e.to_string())?;
        g.git
            .update_ref("refs/heads/main", &commit, Some(&main))
            .await
            .map_err(|e| e.to_string())?;
        commit
    } else {
        let p = proposals
            .first()
            .ok_or("recovered save has no destination")?;
        crate::proposal::write(g, &me, &p.parent, &p.tree)
            .await
            .map_err(|e| e.to_string())?
    };
    Ok((name, key, commit))
}
