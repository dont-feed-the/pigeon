//! Freeing and keeping (08 §8.5 and 07 §7.5), and the recovery of the
//! install journal at start and after a failed step (07 §7.5).
//!
//! A request only records the folder's target and wakes the loops; each
//! round moves every folder toward its target, so a crash at any point
//! ends as one transition after the restart. `free` hides each protected
//! file as an install of its absence — journal
//! `begin`, move aside, writer scan, check, `proved` — then sets the
//! skip-worktree bits with the files already absent, so git unlinks
//! nothing, and drops each key no visible path still names with a verified
//! drop. `keep` gets every key, stages each file, renames it in without
//! replacing anything, and clears the bits; a file saved there meanwhile
//! wins and is only unhidden, so capture takes it. The target of each folder is in
//! `<git_dir>/pigeon/freed.json` in the slice, not yet in `wanted`, and the
//! check hashes the aside against its key (13 §13.12).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use pigeon_core::journal::{
    Begin, End, Found, Record, Stat, Step, can_truncate, entries, recover as steps,
};
use pigeon_core::state::State;
use pigeon_sys::fs::{Renamed, lstat, rename_noreplace};
use pigeon_sys::lab::crash_point;

use crate::group::Group;
use crate::journal_file::{append, read};
use crate::net::sync::{key_sha256, sha256_file};
use crate::trace::Tracer;
use crate::trace_state::{Checkpoint, Span};

/// What `free` did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Freed {
    /// Files hidden, now or before.
    pub hidden: usize,
    /// Files under the path.
    pub total: usize,
    /// Files not protected yet, kept visible.
    pub waiting: usize,
    /// Keys dropped.
    pub dropped: usize,
}

pub(crate) fn stat_of(p: &Path) -> Option<Stat> {
    let st = lstat(p).ok()??;
    st.is_regular().then_some(Stat {
        dev: st.id.dev,
        ino: st.id.ino,
        size: st.size,
        mtime_ns: i128::from(st.mtime_ns),
    })
}

pub(crate) fn journal(g: &Group) -> PathBuf {
    g.files.install_journal()
}

pub(crate) fn next_seq(records: &[Record]) -> u64 {
    records
        .iter()
        .filter_map(|r| match r {
            Record::Begin(b) => Some(b.seq),
            _ => None,
        })
        .max()
        .map_or(1, |s| s + 1)
}

pub(crate) fn under(path: &str, prefix: &str) -> bool {
    prefix.is_empty() || path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// The paths whose skip-worktree bit is set.
pub async fn hidden_paths(g: &Group) -> BTreeSet<String> {
    let Ok(out) = g
        .git
        .repo
        .run(&["ls-files", "-v", "-z"], None, pigeon_git::DEFAULT_TIMEOUT)
        .await
    else {
        return BTreeSet::new();
    };
    out.stdout
        .split(|b| *b == 0)
        .filter_map(|rec| {
            let rec = String::from_utf8_lossy(rec);
            rec.strip_prefix("S ").map(str::to_owned)
        })
        .collect()
}

fn escape(path: &str) -> String {
    let mut out = String::new();
    for c in path.chars() {
        if matches!(c, '*' | '?' | '[' | '\\' | '!' | '#') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Writes the sparse patterns that hide exactly `hidden` (non-cone).
pub(crate) fn write_sparse(g: &Group, hidden: &BTreeSet<String>) {
    let mut text = String::from("/*\n");
    for p in hidden {
        text.push_str(&format!("!/{}\n", escape(p)));
    }
    let file = g.git_dir.join("info").join("sparse-checkout");
    let _ = pigeon_sys::fs::write_atomic(&file, text.as_bytes());
}

/// This computer's folder choices as the loops apply them: its own, and
/// other people's folders of a per-person folder freed by default (08 §8.5).
pub(crate) fn targets(g: &Group) -> pigeon_core::keep_targets::Targets {
    let chosen = crate::keep_targets::read(g);
    let Some(doc) = g.roster() else {
        return chosen;
    };
    let me = crate::git::signer_device().and_then(|device| {
        doc.roster
            .devices
            .get(&pigeon_core::roster::DeviceId(device))
            .map(|d| d.person.clone())
    });
    match me {
        Some(me) => {
            chosen.with_defaults(pigeon_core::keep_targets::others_folders(&doc.roster, &me))
        }
        None => chosen,
    }
}

/// Orders the requests that change a target against the loop's check
/// before each hide, so that no file is hidden once `keep` has answered.
static TARGETS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn targets_lock() -> std::sync::MutexGuard<'static, ()> {
    TARGETS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Records whether the folder `prefix` is to be freed (`pigeon free`) or
/// kept (`pigeon keep`); the loops do the rest, and never record one.
pub fn set_target(g: &Group, prefix: &str, freed: bool) -> Result<(), String> {
    let _order = targets_lock();
    let mut t = crate::keep_targets::read(g);
    t.set(prefix, freed);
    crate::keep_targets::write(g, t)
}

/// Forgets this computer's choices of the folder `prefix` and inside it, so
/// that it follows the folder holding it, or its default, again.
pub fn forget_target(g: &Group, prefix: &str) -> Result<(), String> {
    let _order = targets_lock();
    let mut t = crate::keep_targets::read(g);
    t.0.retain(|path, _| !under(path, prefix));
    crate::keep_targets::write(g, t)
}

pub(crate) fn record(g: &Group, r: &Record) -> Result<(), String> {
    append(&journal(g), r).map_err(|e| e.to_string())?;
    if let Record::End(seq, _) = r {
        append(&journal(g), &Record::Kept(*seq, crate::util::now())).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn is_old(aside: &Path, key: Option<&str>) -> bool {
    match (key.and_then(key_sha256), sha256_file(aside)) {
        (Some(want), Ok(have)) => have == want.to_ascii_lowercase(),
        _ => false,
    }
}

/// Hides one protected file; answers whether it is hidden.
fn hide(
    g: &Group,
    wt: &Path,
    seq: u64,
    path: &str,
    key: &str,
    trace: &Checkpoint<'_>,
) -> Result<bool, String> {
    let file = wt.join(path);
    let Some(st) = stat_of(&file) else {
        return Ok(false);
    };
    let aside = g.files.aside(seq);
    let start = trace.begin("Start");
    record(
        g,
        &Record::Begin(Begin {
            seq,
            path: path.to_owned(),
            old_key: Some(key.to_owned()),
            new_key: None,
            stat: Some(st),
            stage: None,
            aside: aside.to_string_lossy().into_owned(),
        }),
    )?;
    record(g, &Record::Operation(seq, "free".into()))?;
    start.finish("begun");
    crash_point("free-after-begin");
    let moved = trace.begin("MoveAside");
    if rename_noreplace(&file, &aside).map_err(|e| e.to_string())? != Renamed::Done {
        record(g, &Record::End(seq, End::Skip))?;
        moved.finish("idle");
        return Ok(false);
    }
    moved.finish("aside");
    crash_point("free-after-move-aside");
    let clear = lstat(&aside)
        .ok()
        .flatten()
        .is_some_and(|a| crate::asides::writers_clear(&a));
    let scan = trace.scan(clear);
    if !clear {
        crate::install::rollback(g, seq, &aside, &file, None)?;
        scan.finish("idle");
        return Ok(false);
    }
    scan.finish("scanned");
    let check = trace.begin("Check");
    if stat_of(&aside) != Some(st) || !is_old(&aside, Some(key)) {
        crate::install::rollback(g, seq, &aside, &file, None)?;
        check.finish("idle");
        return Ok(false);
    }
    record(g, &Record::Proved(vec![seq]))?;
    check.finish("checked");
    crash_point("free-after-proved");
    Ok(true)
}

/// Frees the folder `prefix` while it is a target (`pigeon free` records it).
pub async fn free(g: &Group, me: &str, prefix: &str, t: &Tracer) -> Result<Freed, String> {
    free_with(g, me, prefix, t, true).await
}

/// Frees `prefix`; without `check`, every file present, protected or not,
/// and its content dropped with `--force` (`pigeon free --without-checking`,
/// 08 §8.7). A file changed since its version is never hidden.
pub async fn free_with(
    g: &Group,
    me: &str,
    prefix: &str,
    t: &Tracer,
    check: bool,
) -> Result<Freed, String> {
    if crate::capture_file::pending(g) || unsettled(g) {
        return Err("capture-or-install-pending".to_owned());
    }
    let wt = g.worktree.clone().ok_or("not-in-group")?;
    let observed = crate::status::observe(g, me, prefix).await;
    let mut hidden = hidden_paths(g).await;
    let mut seq = next_seq(&read(&journal(g)));
    let mut done = Freed {
        total: observed.len(),
        ..Freed::default()
    };
    let mut new = Vec::new();
    for o in &observed {
        let ready = match o.state {
            State::Protected(_) => true,
            State::Missing(_) => false,
            _ => !check,
        };
        if !ready {
            done.waiting += 1;
            continue;
        }
        if hidden.contains(&o.path) {
            continue;
        }
        let trace = Checkpoint::new(t, g, &o.path, seq, "free").await;
        let order = targets_lock();
        if !targets(g).is_freed(prefix) {
            break;
        }
        if !targets(g).is_freed(&o.path) {
            continue;
        }
        let hidden_now = hide(g, &wt, seq, &o.path, &o.key, &trace);
        drop(order);
        if hidden_now? {
            new.push((seq, o.path.clone()));
        }
        seq += 1;
    }
    if !new.is_empty() {
        let mut indexing = Vec::new();
        for (seq, path) in &new {
            indexing.push(Span::begin(t, g, path, *seq, "Index", "free").await);
        }
        hidden.extend(new.iter().map(|(_, p)| p.clone()));
        write_sparse(g, &hidden);
        let mut args = vec!["update-index", "--skip-worktree", "--"];
        args.extend(new.iter().map(|(_, p)| p.as_str()));
        if let Err(error) = g.git.run(&args).await {
            for span in indexing {
                span.interrupted("checked").await;
            }
            return Err(error.to_string());
        }
        for span in indexing {
            span.finish("indexed").await;
        }
        crash_point("free-after-index");
        for (seq, path) in &new {
            let done = Span::begin(t, g, path, *seq, "Done", "free").await;
            record(g, &Record::End(*seq, End::Done))?;
            done.finish("idle").await;
        }
    }
    done.hidden = observed.iter().filter(|o| hidden.contains(&o.path)).count();
    if targets(g).is_freed(prefix) {
        done.dropped = drop_hidden(g, &hidden, prefix, check).await;
    }
    truncate_if_done(g);
    Ok(done)
}

/// Frees again each folder freed on this computer, so that a file which
/// became protected since is hidden and dropped (08 §8.5, step 5). The
/// round runs it before capture, so a restart first resumes the transition.
pub async fn free_targets(g: &Group, me: &str, t: &Tracer) {
    for prefix in targets(g).free_roots() {
        if let Err(e) = free(g, me, prefix, t).await {
            tracing::warn!(group = %g.id, prefix = %prefix, error = %e, "free-failed");
        }
    }
}

/// Drops the keys of hidden paths under `prefix` that no visible path names.
async fn drop_hidden(g: &Group, hidden: &BTreeSet<String>, prefix: &str, check: bool) -> usize {
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return 0;
    };
    let keys = g.git.keys(&main).await.unwrap_or_default();
    let freed = targets(g);
    let visible: BTreeSet<&String> = keys
        .iter()
        .filter(|(p, _)| !hidden.contains(*p) || !freed.is_freed(p))
        .map(|(_, k)| k)
        .collect();
    let k = crate::status::settings(g).k.to_string();
    let mut dropped = 0;
    let wanted: BTreeSet<&String> = keys
        .iter()
        .filter(|(p, _)| hidden.contains(*p) && under(p, prefix) && freed.is_freed(p))
        .map(|(_, k)| k)
        .collect();
    for key in wanted {
        if visible.contains(key) || crate::custody::present(g, key).await.is_none() {
            continue;
        }
        let mut args = vec!["annex", "drop", "--json", "--key", key.as_str()];
        if check {
            args.extend(["--numcopies", k.as_str(), "--mincopies", k.as_str()]);
        } else {
            args.push("--force");
        }
        match g.git.run_for(&args, Duration::from_secs(300)).await {
            Ok(_) => dropped += 1,
            Err(e) => tracing::info!(group = %g.id, key = %key, error = %e, "drop-refused"),
        }
    }
    dropped
}

pub(crate) fn truncate_if_done(g: &Group) {
    let records = read(&journal(g));
    if !records.is_empty() && can_truncate(&entries(&records)) {
        let _ = pigeon_sys::fs::write_atomic(&journal(g), b"");
    }
}

/// Brings back every hidden file outside the folders to free: answers how
/// many came back. A key that cannot be received yet waits for the next
/// round, and the folder stays hidden meanwhile (08 §8.5, keep step 1).
pub async fn keep(g: &Group, t: &Tracer) -> Result<usize, String> {
    if crate::capture_file::pending(g) || unsettled(g) {
        return Err("capture-or-install-pending".to_owned());
    }
    let wt = g.worktree.clone().ok_or("not-in-group")?;
    let freed = targets(g);
    let mut hidden = hidden_paths(g).await;
    if hidden.iter().all(|p| freed.is_freed(p)) {
        return Ok(0);
    }
    let main = g.git.rev("refs/heads/main").await.ok_or("not-in-group")?;
    let keys = g.git.keys(&main).await.map_err(|e| e.to_string())?;
    let modes: BTreeMap<String, String> = g
        .git
        .tree_files(&main)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| (f.path, f.mode))
        .collect();
    let wanted: Vec<(String, String)> = hidden
        .iter()
        .filter(|p| !freed.is_freed(p))
        .filter_map(|p| Some((p.clone(), keys.get(p)?.clone())))
        .collect();
    let mut objects = BTreeMap::new();
    let mut queue = crate::telemetry::Queue::start(&g.id, "download", "keep", wanted.len());
    for (_, key) in &wanted {
        if objects.contains_key(key) {
            continue;
        }
        if crate::custody::present(g, key).await.is_none() {
            let args = ["annex", "get", "--json", "--key", key.as_str()];
            g.git
                .run_for(&args, Duration::from_secs(600))
                .await
                .map_err(|_| "receiving".to_owned())?;
        }
        let object = crate::custody::present(g, key).await.ok_or("receiving")?;
        objects.insert(key.clone(), object);
    }
    let mut seq = next_seq(&read(&journal(g)));
    let mut moved = Vec::new();
    let mut raced = Vec::new();
    for (path, key) in &wanted {
        queue.complete(1);
        let stage = g.files.stage(seq);
        let object = objects.get(key).ok_or("receiving")?;
        let start = Span::begin(t, g, path, seq, "Start", "keep").await;
        std::fs::copy(object, &stage).map_err(|e| e.to_string())?;
        let mode = if modes.get(path).map(String::as_str) == Some("100755") {
            0o755
        } else {
            0o644
        };
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(mode))
            .map_err(|e| e.to_string())?;
        std::fs::File::open(&stage)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        pigeon_sys::fs::fsync_dir(stage.parent().ok_or("no stage parent")?)
            .map_err(|e| e.to_string())?;
        record(
            g,
            &Record::Begin(Begin {
                seq,
                path: path.clone(),
                old_key: None,
                new_key: Some(key.clone()),
                stat: None,
                stage: Some(stage.to_string_lossy().into_owned()),
                aside: g.files.aside(seq).to_string_lossy().into_owned(),
            }),
        )?;
        record(g, &Record::Operation(seq, "keep".into()))?;
        start.finish("begun").await;
        crash_point("keep-after-begin");
        let check = Span::begin(t, g, path, seq, "MoveAside", "keep").await;
        let file = wt.join(path);
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if lstat(&file).ok().flatten().is_some() {
            record(g, &Record::End(seq, End::Raced))?;
            let _ = std::fs::remove_file(&stage);
            check.finish("idle").await;
            raced.push((seq, path.clone()));
            seq += 1;
            continue;
        }
        record(g, &Record::Proved(vec![seq]))?;
        check.finish("checked").await;
        let renamed = Span::begin(t, g, path, seq, "RenameIn", "keep").await;
        if rename_noreplace(&stage, &file).map_err(|e| e.to_string())? != Renamed::Done {
            record(g, &Record::End(seq, End::Raced))?;
            let _ = std::fs::remove_file(&stage);
            renamed.finish("idle").await;
            raced.push((seq, path.clone()));
            seq += 1;
            continue;
        }
        renamed.finish("renamed").await;
        crash_point("keep-after-rename-in");
        moved.push((seq, path.clone()));
        seq += 1;
    }
    if !moved.is_empty() || !raced.is_empty() {
        let mut indexing = Vec::new();
        for (seq, path) in moved.iter().chain(&raced) {
            indexing.push(Span::begin(t, g, path, *seq, "Index", "keep").await);
            hidden.remove(path);
        }
        write_sparse(g, &hidden);
        let mut args = vec!["update-index", "--no-skip-worktree", "--"];
        args.extend(moved.iter().chain(&raced).map(|(_, p)| p.as_str()));
        if let Err(error) = g.git.run(&args).await {
            for span in indexing {
                span.interrupted("renamed").await;
            }
            return Err(error.to_string());
        }
        let _ = g.git.run(&["update-index", "-q", "--refresh"]).await;
        for span in indexing {
            span.finish("indexed").await;
        }
        for (seq, path) in &moved {
            let done = Span::begin(t, g, path, *seq, "Done", "keep").await;
            record(g, &Record::End(*seq, End::Done))?;
            record(g, &Record::Released(*seq))?;
            done.finish("idle").await;
        }
    }
    truncate_if_done(g);
    Ok(moved.len())
}

/// Whether an entry of the journal has no end: a transition that a crash
/// or a failed step left half done, which [`recover`] settles (07 §7.5).
/// Capture takes nothing while one has none: a file moved aside is no
/// deletion of the person's.
pub fn unsettled(g: &Group) -> bool {
    entries(&read(&journal(g)))
        .values()
        .any(|e| e.end.is_none())
}

/// Replays every entry of the journal without an end (07 §7.5), then
/// releases the asides of ended entries and truncates the journal.
pub async fn recover(g: &Group, t: &Tracer) {
    let Some(wt) = g.worktree.clone() else {
        return;
    };
    let records = read(&journal(g));
    for (seq, e) in entries(&records) {
        let b = &e.begin;
        let file = wt.join(&b.path);
        let aside = PathBuf::from(&b.aside);
        let stage = b.stage.as_ref().map(PathBuf::from);
        let found = Found {
            path: lstat(&file).ok().flatten().is_some(),
            aside: aside.exists(),
            stage: stage.as_ref().is_some_and(|s| s.exists()),
            aside_is_old: aside.exists() && is_old(&aside, b.old_key.as_deref()),
        };
        let kind = crate::trace_state::operation(&e);
        let replay = steps(&e, found);
        if e.end.is_some() && replay.is_empty() && !t.unfinished(&g.id, &b.path) {
            continue;
        }
        let action = if e.end.is_none() {
            "Recover"
        } else {
            "Cleanup"
        };
        let mut recovering = Some(Span::begin(t, g, &b.path, seq, action, kind).await);
        let mut ending = None;
        let mut failed = false;
        for step in replay {
            if matches!(step, Step::Restage | Step::Append(_))
                && let Some(span) = recovering.take()
            {
                span.finish("rec").await;
                ending = Some(
                    crate::trace_state::Span::begin(t, g, &b.path, seq, "RecoverEnd", kind).await,
                );
            }
            let ok = match step {
                Step::RenameIn => stage
                    .as_ref()
                    .is_some_and(|s| matches!(rename_noreplace(s, &file), Ok(Renamed::Done))),
                Step::Restage => {
                    if e.operation.as_deref() == Some("restore") {
                        // Restore leaves the pointer for the next capture,
                        // including when the path was deleted from the index.
                        true
                    } else if let Some(entry) = &e.install_index {
                        if crate::install::index_entry(g, &b.path, entry.as_ref())
                            .await
                            .is_err()
                        {
                            false
                        } else if entry.is_some() {
                            g.git
                                .run(&["update-index", "--no-skip-worktree", "--", &b.path])
                                .await
                                .is_ok()
                        } else {
                            true
                        }
                    } else {
                        let bit = if b.new_key.is_none() {
                            "--skip-worktree"
                        } else {
                            "--no-skip-worktree"
                        };
                        if b.new_key.is_none() {
                            let mut h = hidden_paths(g).await;
                            h.insert(b.path.clone());
                            write_sparse(g, &h);
                        }
                        g.git
                            .run(&["update-index", bit, "--", &b.path])
                            .await
                            .is_ok()
                    }
                }
                Step::PutAsideBack => matches!(rename_noreplace(&aside, &file), Ok(Renamed::Done)),
                Step::Append(end) => record(g, &Record::End(seq, end)).is_ok(),
                Step::RemoveStage => stage.as_ref().is_none_or(|s| {
                    std::fs::remove_file(s)
                        .map_or_else(|e| e.kind() == std::io::ErrorKind::NotFound, |()| true)
                }),
            };
            if !ok {
                if let Some(span) = recovering.take() {
                    span.interrupted("down").await;
                }
                if let Some(span) = ending.take() {
                    span.interrupted("rec").await;
                }
                tracing::warn!(group = %g.id, seq, step = ?step, "recover-step-failed");
                failed = true;
                break;
            }
        }
        if !failed {
            if let Some(span) = ending {
                span.finish("idle").await;
            }
            if let Some(span) = recovering {
                span.finish("idle").await;
            }
        }
    }
    crate::asides::release(g, t).await;
    for (path, seq, kind) in t.unfinished_paths(&g.id) {
        Span::begin(t, g, &path, seq, "Cleanup", kind)
            .await
            .finish("idle")
            .await;
    }
    let _ = g.git.run(&["update-index", "-q", "--refresh"]).await;
    truncate_if_done(g);
}
