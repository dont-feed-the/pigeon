//! Re-thinning (07 §7.8): a file a laptop captured is added as a copy
//! (`-c annex.thin=false`), so that the version stays immutable in the
//! annex until it is protected. Once it is, and the file is still that
//! version, the laptop replaces the file by a hard link to the object,
//! as an install (07 §7.5) of the same key, so that it keeps about one
//! copy of what it keeps (S11, 13 §13.11).
//!
//! git-annex on macOS copies without cloning (`cp` takes no `--reflink`
//! there, `Utility/CopyFile.hs`), so the laptop re-thins on every file
//! system. The object is made writable with the file's mode, as git-annex
//! thaws an object it hard-links (`linkAnnex` in `Annex/Content.hs`). An
//! in-place save may then change the object, which never counts toward K
//! (ADR 0002), while the custodians keep theirs.
//!
//! The file is gone from its path between move-aside and rename-in, so
//! that window holds only the writer scan and the stat check of 07 §7.5:
//! the content is hashed in place before, and the aside after, where a
//! mismatch keeps it as a recovered version (step 9). A file changed in
//! the last minute waits, so that one in use is not moved (ADR 0037).

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use pigeon_core::journal::{Begin, End, Record};
use pigeon_core::state::State;
use pigeon_sys::fs::{Renamed, lstat, rename_noreplace};

use crate::evict::{hidden_paths, is_old, journal, next_seq, record, stat_of, truncate_if_done};
use crate::group::Group;
use crate::journal_file::read;
use crate::trace::{Act, Tracer};

/// How long a file stays unchanged (content and metadata) before it is
/// re-thinned.
const QUIET_S: i64 = 60;

/// Whether `file` has not changed for [`QUIET_S`] seconds.
fn quiet(file: &Path) -> bool {
    let Ok(m) = std::fs::symlink_metadata(file) else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    now.saturating_sub(m.ctime().max(m.mtime())) >= QUIET_S
}

fn same_inode(a: &Path, b: &Path) -> bool {
    match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

/// One round: re-thins every visible file whose version is protected and
/// whose content is still a copy of its object. Answers how many it did.
pub async fn rethin(g: &Group, me: &str, t: &Tracer) -> usize {
    if crate::capture_file::pending(g) || crate::evict::unsettled(g) {
        return 0;
    }
    let Some(wt) = g.worktree.clone() else {
        return 0;
    };
    let hidden = hidden_paths(g).await;
    let observed = crate::status::observe(g, me, "").await;
    let mut done = 0;
    for o in observed {
        if !matches!(o.state, State::Protected(_)) || hidden.contains(&o.path) {
            continue;
        }
        let file = wt.join(&o.path);
        let Some(object) = crate::custody::present(g, &o.key).await else {
            continue;
        };
        if same_inode(&file, &object) {
            continue;
        }
        match one(g, &file, &object, &o.path, &o.key, t) {
            Ok(true) => done += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!(group = %g.id, path = %o.path, error = %e, "rethin-failed"),
        }
    }
    if done > 0 {
        let _ = g.git.run(&["update-index", "-q", "--refresh"]).await;
        tracing::info!(group = %g.id, files = done, "rethinned");
    }
    truncate_if_done(g);
    done
}

/// Re-thins one file; answers whether it did.
fn one(
    g: &Group,
    file: &Path,
    object: &Path,
    path: &str,
    key: &str,
    t: &Tracer,
) -> Result<bool, String> {
    let Some(st) = stat_of(file) else {
        return Ok(false);
    };
    if crate::restore::key_size(key).is_some_and(|n| n != st.size) || !quiet(file) {
        return Ok(false);
    }
    // The content is proved in place; in the window only the stat is.
    if !is_old(file, Some(key)) || stat_of(file) != Some(st) {
        return Ok(false);
    }
    let mode = std::fs::symlink_metadata(file)
        .map(|m| m.mode() & 0o777)
        .map_err(|e| e.to_string())?;
    let seq = next_seq(&read(&journal(g)));
    let stage = g.files.stage(seq);
    let aside = g.files.aside(seq);
    std::fs::hard_link(object, &stage).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(mode | 0o200))
        .map_err(|e| e.to_string())?;
    record(
        g,
        &Record::Begin(Begin {
            seq,
            path: path.to_owned(),
            old_key: Some(key.to_owned()),
            new_key: Some(key.to_owned()),
            stat: Some(st),
            stage: Some(stage.to_string_lossy().into_owned()),
            aside: aside.to_string_lossy().into_owned(),
        }),
    )?;
    t.event(Act::Start, &[("path", path)]);
    let window = Instant::now();
    if rename_noreplace(file, &aside).map_err(|e| e.to_string())? != Renamed::Done {
        record(g, &Record::End(seq, End::Skip))?;
        let _ = std::fs::remove_file(&stage);
        return Ok(false);
    }
    t.event(Act::MoveAside, &[("path", path)]);
    let clear = lstat(&aside)
        .ok()
        .flatten()
        .is_some_and(|a| crate::asides::writers_clear(&a));
    t.event(Act::Scan, &[("path", path)]);
    if !clear || stat_of(&aside) != Some(st) {
        let end = match rename_noreplace(&aside, file).map_err(|e| e.to_string())? {
            Renamed::Done | Renamed::SourceMissing => End::Skip,
            Renamed::TargetExists => End::Recovered,
        };
        record(g, &Record::End(seq, end))?;
        let _ = std::fs::remove_file(&stage);
        return Ok(false);
    }
    record(g, &Record::Proved(vec![seq]))?;
    t.event(Act::Check, &[("path", path)]);
    if rename_noreplace(&stage, file).map_err(|e| e.to_string())? != Renamed::Done {
        record(g, &Record::End(seq, End::Raced))?;
        let _ = std::fs::remove_file(&stage);
        return Ok(false);
    }
    t.event(Act::RenameIn, &[("path", path)]);
    let ms = window.elapsed().as_millis();
    tracing::debug!(group = %g.id, path, ms, "rethin-window");
    record(g, &Record::End(seq, End::Done))?;
    t.event(Act::Done, &[("path", path)]);
    Ok(true)
}
