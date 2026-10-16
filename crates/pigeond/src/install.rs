//! Install (07 §7.5): a laptop moves each clean path of its folder from its
//! index version to `main`'s, through the install journal, without a commit.
//!
//! The round runs it after accept moved `main`, under `busy`, and capture
//! took every save first (`CaptureFirst`). A path the person changed since
//! the index is not written: it diverges, and capture keeps it out of `main`
//! (07 §7.6). A path whose content cannot be received yet waits
//! (`receiving`); a hidden path changes in the index only (E15). A file is
//! staged as a copy with its mode, renamed in without replacing anything,
//! and its index entry is written without stat data, so git records the
//! stat only if the file holds the new key (step 8, E19). A deletion moves
//! the file aside, checks that it holds the old key, and removes it with
//! the folders it emptied. pigeon's own blobs (`.pigeon/`, the root
//! `.gitattributes`) are written as they are.

use std::collections::BTreeSet;
use std::path::Path;

use pigeon_core::authority::Entry;
use pigeon_core::catalogue::Content;
use pigeon_core::journal::{Begin, End, Record};
use pigeon_core::names;
use pigeon_sys::fs::{Renamed, lstat, rename_noreplace};

use crate::conflicts_file::{Divergence, Kind, State, Table};
use crate::evict::{is_old, journal, next_seq, record, stat_of, truncate_if_done};
use crate::git::{Error, is_pigeon_path, pointer_key};
use crate::group::Group;
use crate::journal_file::read;
use crate::trace::Tracer;
use crate::trace_state::{Checkpoint, Span};

/// How the install of one path ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Put {
    /// The path holds the new version, or is gone.
    Done(i64), // actual rename time, before post-install observations
    /// A person's file took the path first: theirs stays (`raced`).
    Raced,
    /// A process may be writing the file, or it is not the old version:
    /// nothing was replaced.
    InUse,
}

/// What one round of install did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Installed {
    /// Files written.
    pub written: usize,
    /// Files removed.
    pub removed: usize,
    /// Paths whose content is not here yet (`receiving`).
    pub receiving: usize,
    /// Paths left as they are: changed by the person, in use, or raced.
    pub deferred: usize,
}

fn mode_bits(mode: Option<&str>, file: &Path) -> u32 {
    let exec = match mode {
        Some(mode) => mode == "100755",
        None => lstat(file)
            .ok()
            .flatten()
            .is_some_and(|st| st.mode & 0o111 != 0),
    };
    if exec { 0o755 } else { 0o644 }
}

// Restore and explicit resolution have additional semantics absent from
// CaptureInstall.Start. Keep their observations separately identified.
fn trace_kind(g: &Group, rel: &str, indexed: bool) -> &'static str {
    if !indexed {
        "restore"
    } else if crate::conflicts_file::read(g).is_some_and(|t| t.paths.contains_key(rel)) {
        "resolve"
    } else {
        "install"
    }
}

/// Installs `key`'s content at `rel`, moving the file aside first if there
/// is one, whose version is `current`; a file saved meanwhile wins. With
/// `entry`, the index takes it once the file is in (step 8); without, the
/// next capture records the file (`pigeon restore`).
pub(crate) async fn put(
    g: &Group,
    rel: &str,
    key: &str,
    current: Option<&str>,
    entry: Option<&Entry>,
    t: &Tracer,
) -> Result<Put, String> {
    put_mode(g, rel, key, current, entry, None, t).await
}

/// An explicit restore keeps the saved executable mode while leaving
/// the index for capture to record the restore as a new version.
pub(crate) async fn put_restore(
    g: &Group,
    rel: &str,
    key: &str,
    current: Option<&str>,
    mode: &str,
    t: &Tracer,
) -> Result<Put, String> {
    put_mode(g, rel, key, current, None, Some(mode), t).await
}

async fn put_mode(
    g: &Group,
    rel: &str,
    key: &str,
    current: Option<&str>,
    entry: Option<&Entry>,
    mode: Option<&str>,
    t: &Tracer,
) -> Result<Put, String> {
    if crate::capture_file::pending(g) {
        return Err("capture-pending".to_owned());
    }
    if crate::evict::unsettled(g) {
        return Err("install-pending".to_owned());
    }
    let wt = g.worktree.clone().ok_or("not-in-group")?;
    let object = crate::custody::present(g, key).await.ok_or("receiving")?;
    let file = wt.join(rel);
    let kind = trace_kind(g, rel, entry.is_some());
    let seq = next_seq(&read(&journal(g)));
    let stage = g.files.stage(seq);
    let aside = g.files.aside(seq);
    let start = Span::begin_request(t, g, rel, seq, ("Start", kind), Some(key)).await;
    std::fs::copy(&object, &stage).map_err(|e| e.to_string())?;
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::Permissions::from_mode(mode_bits(
            mode.or(entry.map(|e| e.mode.as_str())),
            &file,
        ));
        let _ = std::fs::set_permissions(&stage, mode);
    }
    std::fs::File::open(&stage)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    pigeon_sys::fs::fsync_dir(stage.parent().ok_or("no stage parent")?)
        .map_err(|e| e.to_string())?;
    let st = stat_of(&file);
    // The caller's dirty scan precedes staging. A creation/deletion in
    // that gap must reach capture, rather than become a new install.
    if entry.is_some() && current.is_some() != st.is_some() {
        std::fs::remove_file(&stage).map_err(|e| e.to_string())?;
        start.interrupted("idle").await;
        return Ok(Put::Raced);
    }
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
    if let Some(entry) = entry {
        record(g, &Record::InstallIndex(seq, Some(entry.clone())))?;
    }
    record(g, &Record::Operation(seq, kind.to_owned()))?;
    start.finish("begun").await;
    pigeon_sys::lab::crash_point("install.after-begin");
    let trace = Checkpoint::new(t, g, rel, seq, kind).await;
    if st.is_some() {
        let moved = trace.begin("MoveAside");
        if rename_noreplace(&file, &aside).map_err(|e| e.to_string())? != Renamed::Done {
            record(g, &Record::End(seq, End::Raced))?;
            let _ = std::fs::remove_file(&stage);
            moved.finish("idle");
            return Ok(Put::Raced);
        }
        moved.finish("aside");
        pigeon_sys::lab::crash_point("install.after-aside");
        let clear = lstat(&aside)
            .ok()
            .flatten()
            .is_some_and(|a| crate::asides::writers_clear(&a));
        let scan = trace.scan(clear);
        if !clear {
            rollback(g, seq, &aside, &file, Some(&stage))?;
            scan.finish("idle");
            return Ok(Put::InUse);
        }
        scan.finish("scanned");
    } else if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let check = trace.begin(if st.is_some() { "Check" } else { "MoveAside" });
    if st.is_some() && (stat_of(&aside) != st || !is_old(&aside, current)) {
        rollback(g, seq, &aside, &file, Some(&stage))?;
        check.finish("idle");
        return Ok(Put::InUse);
    }
    record(g, &Record::Proved(vec![seq]))?;
    check.finish("checked");
    pigeon_sys::lab::crash_point("install.after-proof");
    let renamed = trace.begin("RenameIn");
    if rename_noreplace(&stage, &file).map_err(|e| e.to_string())? != Renamed::Done {
        record(g, &Record::End(seq, End::Raced))?;
        let _ = std::fs::remove_file(&stage);
        renamed.finish("idle");
        return Ok(Put::Raced);
    }
    let installed_at = crate::util::now_ns();
    renamed.finish("renamed");
    pigeon_sys::lab::crash_point("install.after-rename");
    if let Some(e) = entry {
        let index = Span::begin(t, g, rel, seq, "Index", kind).await;
        if let Err(error) = index_entry(g, rel, Some(e)).await {
            index.interrupted("renamed").await;
            return Err(error);
        }
        pigeon_sys::lab::crash_point("install.before-index-observation");
        index.finish("indexed").await;
    }
    finish(g, seq, rel, &aside, t, kind).await?;
    Ok(Put::Done(installed_at))
}

/// Exercise the real replacement driver without API/network setup in the
/// lab. `None` selects restore; an entry selects install or resolution.
#[cfg(feature = "lab")]
pub async fn replace_for_lab(
    g: &Group,
    rel: &str,
    key: &str,
    current: Option<&str>,
    entry: Option<&Entry>,
    tracer: &Tracer,
) -> Result<bool, String> {
    put(g, rel, key, current, entry, tracer)
        .await
        .map(|out| matches!(out, Put::Done(_)))
}

pub(crate) fn rollback(
    g: &Group,
    seq: u64,
    aside: &Path,
    file: &Path,
    stage: Option<&Path>,
) -> Result<(), String> {
    let end = match rename_noreplace(aside, file).map_err(|e| e.to_string())? {
        Renamed::Done | Renamed::SourceMissing => End::Skip,
        Renamed::TargetExists => End::Recovered,
    };
    record(g, &Record::End(seq, end))?;
    if let Some(stage) = stage {
        let _ = std::fs::remove_file(stage);
    }
    Ok(())
}

async fn finish(
    g: &Group,
    seq: u64,
    rel: &str,
    aside: &Path,
    t: &Tracer,
    kind: &'static str,
) -> Result<(), String> {
    let done = Span::begin(t, g, rel, seq, "Done", kind).await;
    record(g, &Record::End(seq, End::Done))?;
    if !aside.exists() {
        record(g, &Record::Released(seq))?;
    }
    truncate_if_done(g);
    pigeon_sys::lab::crash_point("install.before-done-observation");
    done.finish("idle").await;
    pigeon_sys::lab::crash_point("install.after-done");
    Ok(())
}

/// Sets the index entry of `rel` to `entry`, without stat data, or removes it.
pub(crate) async fn index_entry(g: &Group, rel: &str, entry: Option<&Entry>) -> Result<(), String> {
    let out = match entry {
        Some(e) => {
            let info = format!("{},{},{rel}", e.mode, e.oid);
            g.git
                .run(&["update-index", "--add", "--cacheinfo", &info])
                .await
        }
        None => {
            g.git
                .run(&["update-index", "--force-remove", "--", rel])
                .await
        }
    };
    out.map(drop).map_err(|e| e.to_string())
}

/// Removes `rel`, whose version is `current`, as an install of its absence
/// (steps 3–6, 8 and 9), and the folders it emptied.
pub(crate) async fn remove(g: &Group, rel: &str, current: &str, t: &Tracer) -> Result<Put, String> {
    if crate::capture_file::pending(g) {
        return Err("capture-pending".to_owned());
    }
    if crate::evict::unsettled(g) {
        return Err("install-pending".to_owned());
    }
    let wt = g.worktree.clone().ok_or("not-in-group")?;
    let file = wt.join(rel);
    let Some(st) = stat_of(&file) else {
        let observed = Span::begin(t, g, rel, 0, "Adopt", "install").await;
        index_entry(g, rel, None).await?;
        observed.finish("idle").await;
        return Ok(Put::Done(0));
    };
    let kind = trace_kind(g, rel, true);
    let seq = next_seq(&read(&journal(g)));
    let aside = g.files.aside(seq);
    let start = Span::begin(t, g, rel, seq, "Start", kind).await;
    record(
        g,
        &Record::Begin(Begin {
            seq,
            path: rel.to_owned(),
            old_key: Some(current.to_owned()),
            new_key: None,
            stat: Some(st),
            stage: None,
            aside: aside.to_string_lossy().into_owned(),
        }),
    )?;
    record(g, &Record::InstallIndex(seq, None))?;
    record(g, &Record::Operation(seq, kind.to_owned()))?;
    start.finish("begun").await;
    pigeon_sys::lab::crash_point("install.after-begin");
    let trace = Checkpoint::new(t, g, rel, seq, kind).await;
    let moved = trace.begin("MoveAside");
    if rename_noreplace(&file, &aside).map_err(|e| e.to_string())? != Renamed::Done {
        record(g, &Record::End(seq, End::Skip))?;
        moved.finish("idle");
        return Ok(Put::Raced);
    }
    moved.finish("aside");
    pigeon_sys::lab::crash_point("install.after-aside");
    let clear = lstat(&aside)
        .ok()
        .flatten()
        .is_some_and(|a| crate::asides::writers_clear(&a));
    let scan = trace.scan(clear);
    if !clear {
        rollback(g, seq, &aside, &file, None)?;
        scan.finish("idle");
        return Ok(Put::InUse);
    }
    scan.finish("scanned");
    let check = trace.begin("Check");
    if stat_of(&aside) != Some(st) || !is_old(&aside, Some(current)) {
        rollback(g, seq, &aside, &file, None)?;
        check.finish("idle");
        return Ok(Put::InUse);
    }
    record(g, &Record::Proved(vec![seq]))?;
    check.finish("checked");
    pigeon_sys::lab::crash_point("install.after-proof");
    let renamed = trace.begin("RenameIn");
    renamed.finish("renamed");
    let index = Span::begin(t, g, rel, seq, "Index", kind).await;
    if let Err(error) = index_entry(g, rel, None).await {
        index.interrupted("renamed").await;
        return Err(error);
    }
    index.finish("indexed").await;
    finish(g, seq, rel, &aside, t, kind).await?;
    let mut dir = file.parent();
    while let Some(d) = dir.filter(|d| *d != wt.as_path()) {
        if std::fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
    Ok(Put::Done(0))
}

async fn key_of(g: &Group, entry: Option<&Entry>) -> Option<String> {
    let blob = g.git.object(&entry?.oid).await.ok()?;
    pointer_key(&blob)
}

/// Whether `main` shows this device's version of `path`, which the index
/// still has there, under a conflict name of it: a merge gave the path to
/// another version, and the device keeps its file (05 §5.3, 07 §7.6).
async fn lost_the_name(g: &Group, me: &str, main: &str, path: &str, mine: &Entry) -> bool {
    if crate::merge::judge_history(g, &[main.to_owned()])
        .await
        .is_err()
    {
        return false;
    }
    let view = g.lock().engine.catalogue(&[main.to_owned()]);
    let mine = Content::File(mine.clone());
    let origin = view
        .shown
        .values()
        .flatten()
        .find(|s| s.path != path && names::is_copy_of(&s.path, path) && s.version.content == mine)
        .map(|s| s.origin.clone());
    let Some(origin) = origin else {
        return false;
    };
    let raw = g.git.object(&origin).await.unwrap_or_default();
    pigeon_core::sig::verify(&raw).is_ok_and(|d| d.0 == me)
}

/// One round of install: every path whose index entry is not `main`'s,
/// but those of the divergence table (`conflicts.json`), which it updates.
pub async fn incoming(g: &Group, me: &str, t: &Tracer) -> Result<Installed, Error> {
    incoming_at(g, me, t, None, None).await
}

/// An explicit erasure did not capture private staged paths first. Install
/// exactly its committed changes, leaving every unrelated index entry and
/// suggestion untouched, including across a restart after the main CAS.
pub(crate) async fn incoming_erasure(
    g: &Group,
    me: &str,
    t: &Tracer,
    after: &str,
    paths: &BTreeSet<String>,
) -> Result<Installed, Error> {
    incoming_at(g, me, t, Some(after), Some(paths)).await
}

async fn incoming_at(
    g: &Group,
    me: &str,
    t: &Tracer,
    after: Option<&str>,
    scope: Option<&BTreeSet<String>>,
) -> Result<Installed, Error> {
    let mut done = Installed::default();
    if g.worktree.is_none() || crate::capture_file::pending(g) || crate::evict::unsettled(g) {
        return Ok(done);
    }
    let main = match after {
        Some(after) => after.to_owned(),
        None => match g.git.rev("refs/heads/main").await {
            Some(main) => main,
            None => return Ok(done),
        },
    };
    let Some(mut table) = crate::conflicts_file::read(g) else {
        tracing::warn!(group = %g.id, "conflicts-unreadable");
        return Ok(done);
    };
    let mut known = table.clone();
    let want = g.git.tree(&main).await?;
    let mut have = g.git.index().await?;
    table.paths.retain(|p, d| {
        scope.is_some_and(|paths| !paths.contains(p))
            || d.kind == Kind::Suggestion
            || want.get(p) != have.get(p)
    });
    let suggested = table.paths.values().any(|d| d.kind == Kind::Suggestion);
    let mut dirty = None;
    if suggested && scope.is_none() {
        let d = crate::capture::dirty(g).await?;
        let spans = suggestions(g, &want, &mut have, &mut table, &d, t).await;
        save(g, &known, &table, t, spans).await;
        known = table.clone();
        dirty = Some(d);
    }
    let paths: BTreeSet<&String> = want.keys().chain(have.keys()).collect();
    let changed: Vec<&String> = paths
        .into_iter()
        .filter(|p| scope.is_none_or(|paths| paths.contains(*p)) && want.get(*p) != have.get(*p))
        .collect();
    if changed.is_empty() {
        save(g, &known, &table, t, Vec::new()).await;
        return Ok(done);
    }
    let dirty = match dirty {
        Some(d) => d,
        None => crate::capture::dirty(g).await?,
    };
    let freed = crate::shared_overlay::targets(g);
    let before = crate::evict::hidden_paths(g).await;
    let mut hidden = before.clone();
    let mut queue = crate::telemetry::Queue::start(&g.id, "download", "install", changed.len());
    for path in changed {
        queue.complete(1);
        let (w, h) = (want.get(path), have.get(path));
        if is_pigeon_path(path) || path == ".gitattributes" {
            own(g, path, w).await;
            continue;
        }
        let group = key_of(g, w).await;
        if let Some(d) = table.paths.get_mut(path) {
            d.group = group;
            done.deferred += 1;
            continue;
        }
        if dirty.contains(path.as_str()) {
            done.deferred += 1;
            continue;
        }
        let old = key_of(g, h).await;
        if let (Some(h), Some(_)) = (h, w)
            && lost_the_name(g, me, &main, path, h).await
        {
            let stat = g.worktree.as_ref().and_then(|wt| stat_of(&wt.join(path)));
            table.paths.insert(
                path.clone(),
                Divergence {
                    kind: Kind::Concurrent,
                    since: crate::util::rfc3339(crate::util::now()),
                    state: State::Conflict,
                    mine: old.clone(),
                    mine_stat: stat
                        .map(|s| [s.dev.into(), s.ino.into(), s.size.into(), s.mtime_ns]),
                    proposal: None,
                    group,
                    installed: None,
                    told: false,
                },
            );
            tracing::warn!(group = %g.id, path = %path, "conflict");
            done.deferred += 1;
            continue;
        }
        if hidden.contains(path) || freed.is_freed(path) {
            let ok = index_entry(g, path, w).await.is_ok();
            if ok && w.is_some() {
                let _ = g
                    .git
                    .run(&["update-index", "--skip-worktree", "--", path])
                    .await;
                hidden.insert(path.clone());
            } else if ok {
                hidden.remove(path);
            }
            continue;
        }
        let put_or_remove = match w {
            Some(e) => {
                let Some(key) = key_of(g, Some(e)).await else {
                    tracing::warn!(group = %g.id, path = %path, "install-not-a-pointer");
                    done.deferred += 1;
                    continue;
                };
                if crate::restore::bring(g, &key).await.is_err() {
                    done.receiving += 1;
                    continue;
                }
                put(g, path, &key, old.as_deref(), Some(e), t).await
            }
            None => match &old {
                Some(old) => remove(g, path, old, t).await,
                None => index_entry(g, path, None).await.map(|()| Put::Done(0)),
            },
        };
        match put_or_remove {
            Ok(Put::Done(at)) if w.is_some() => {
                done.written += 1;
                if let Some(h) = h {
                    g.lock().installed.insert(path.clone(), (at, h.clone()));
                }
            }
            Ok(Put::Done(_)) => done.removed += 1,
            Ok(_) => done.deferred += 1,
            Err(e) => {
                tracing::warn!(group = %g.id, path = %path, error = %e, "install-failed");
                done.deferred += 1;
            }
        }
    }
    if hidden != before {
        crate::evict::write_sparse(g, &hidden);
    }
    save(g, &known, &table, t, Vec::new()).await;
    let _ = g.git.run(&["update-index", "-q", "--refresh"]).await;
    if done != Installed::default() {
        tracing::info!(group = %g.id, written = done.written, removed = done.removed,
            receiving = done.receiving, deferred = done.deferred, "installed");
    }
    Ok(done)
}

/// Settles the `suggestion`s of `table` against `main`'s tree `want`
/// (07 §7.6): one whose version `main` now has ends, and only the index
/// changes (`Decide`); one whose file is clean again ends too, the person
/// having put a version back; one declined, or whose path `main` changed
/// meanwhile, becomes a conflict, told once.
async fn suggestions<'a>(
    g: &'a Group,
    want: &pigeon_core::authority::Tree,
    have: &mut pigeon_core::authority::Tree,
    table: &mut Table,
    dirty: &BTreeSet<String>,
    tracer: &'a Tracer,
) -> Vec<(String, Span<'a>)> {
    let mut spans = Vec::new();
    let paths: Vec<String> = table
        .paths
        .iter()
        .filter(|(_, d)| d.kind == Kind::Suggestion)
        .map(|(p, _)| p.clone())
        .collect();
    for p in paths {
        spans.push((
            p.clone(),
            Span::begin(tracer, g, &p, 0, "Decide", "install").await,
        ));
        let w = want.get(&p);
        let group = key_of(g, w).await;
        let Some(d) = table.paths.get_mut(&p) else {
            continue;
        };
        if group == d.mine {
            if index_entry(g, &p, w).await.is_ok() {
                match w {
                    Some(e) => have.insert(p.clone(), e.clone()),
                    None => have.remove(&p),
                };
                tracing::info!(group = %g.id, path = %p, "suggestion-adopted");
                table.paths.remove(&p);
            }
        } else if !dirty.contains(&p) {
            tracing::info!(group = %g.id, path = %p, "suggestion-dropped");
            table.paths.remove(&p);
        } else if d.state == State::Waiting
            && d.proposal
                .as_ref()
                .is_some_and(|c| want.contains_key(&pigeon_core::authority::decision_path(c)))
        {
            tracing::warn!(group = %g.id, path = %p, "suggestion-declined");
            d.state = State::Conflict;
            d.told = false;
        } else if group != d.group && d.state == State::Waiting {
            tracing::warn!(group = %g.id, path = %p, "suggestion-overtaken");
            d.group = group;
            d.state = State::Conflict;
            d.told = false;
        }
    }
    spans
}

async fn save<'a>(
    g: &'a Group,
    known: &Table,
    table: &Table,
    tracer: &'a Tracer,
    mut spans: Vec<(String, Span<'a>)>,
) {
    if known != table {
        let paths: BTreeSet<_> = known.paths.keys().chain(table.paths.keys()).collect();
        for path in paths {
            if known.paths.get(path) != table.paths.get(path)
                && !spans.iter().any(|(p, _)| p == path)
            {
                spans.push((
                    path.clone(),
                    Span::begin(tracer, g, path, 0, "Reconcile", "install").await,
                ));
            }
        }
        if let Err(e) = crate::conflicts_file::write(g, table) {
            for (_, span) in spans {
                span.interrupted("idle").await;
            }
            tracing::warn!(group = %g.id, error = %e, "conflicts-write-failed");
            return;
        }
    }
    for (_, span) in spans {
        span.finish("idle").await;
    }
}

/// Writes pigeon's own blob `entry` at `path`, or removes it: the person
/// never edits these (capture skips them), so nothing of theirs is there.
async fn own(g: &Group, path: &str, entry: Option<&Entry>) {
    let Some(wt) = &g.worktree else {
        return;
    };
    let file = wt.join(path);
    let wrote = match entry {
        Some(e) => match g.git.object(&e.oid).await {
            Ok(bytes) => {
                if let Some(dir) = file.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                pigeon_sys::fs::write_atomic(&file, &bytes).map_err(|e| e.to_string())
            }
            Err(e) => Err(e.to_string()),
        },
        None => match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
    };
    let out = match wrote {
        Ok(()) => index_entry(g, path, entry).await,
        Err(e) => Err(e),
    };
    if let Err(e) = out {
        tracing::warn!(group = %g.id, path = %path, error = %e, "install-own-failed");
    }
}
