//! Read-only filesystem observations for CaptureInstall trace refinement.
//! Inodes, hashes, index entries, signed references and journal flags come
//! from the implementation, never from an execution of the model. Errors
//! are trace records too: a failed observation cannot silently pass TLC.

use std::collections::BTreeSet;
use std::path::Path;

use pigeon_core::journal::entries;
use pigeon_sys::fs::lstat;
use serde_json::{Value, json};

use crate::group::Group;
use crate::trace::Tracer;

/// The durable operation tag, with conservative compatibility for journals
/// written before tags existed. Replay itself still uses the index intent.
pub(crate) fn operation(e: &pigeon_core::journal::Entry) -> &'static str {
    match e.operation.as_deref() {
        Some("install") => "install",
        Some("resolve") => "resolve",
        Some("restore") => "restore",
        Some("thin") => "thin",
        Some("free") => "free",
        Some("keep") => "keep",
        _ if e.install_index.is_some() => "install",
        _ if e.begin.new_key.is_none() => "free",
        _ if e.begin.new_key == e.begin.old_key => "thin",
        _ => "keep",
    }
}

/// A filesystem name, read without following links or changing the index.
fn file(path: &Path, observe_writers: bool) -> Result<Value, String> {
    let Some(stat) = lstat(path).map_err(|e| e.to_string())? else {
        return Ok(json!({"inode": "", "content": "", "writers": false, "stat": ""}));
    };
    if !stat.is_regular() {
        return Err("trace encountered a non-regular file".to_owned());
    }
    let hash = crate::net::sync::sha256_file(path).map_err(|e| e.to_string())?;
    if lstat(path).map_err(|e| e.to_string())? != Some(stat) {
        return Err("file changed during trace observation".to_owned());
    }
    Ok(json!({
        "inode": format!("{}:{}", stat.id.dev, stat.id.ino),
        "content": hash, "writers": if observe_writers {
            json!(!crate::asides::writers_clear(&stat))
        } else { Value::Null },
        "stat": format!("{}:{}:{}:{}", stat.id.dev, stat.id.ino, stat.size, stat.mtime_ns),
    }))
}

fn digest(key: Option<&str>) -> Result<String, String> {
    match key {
        None => Ok(String::new()),
        Some(key) => crate::net::sync::key_sha256(key)
            .map(str::to_owned)
            .ok_or_else(|| "trace encountered an unsupported content key".to_owned()),
    }
}

async fn pointer(g: &Group, oid: Option<&str>) -> Result<String, String> {
    let Some(oid) = oid else {
        return Ok(String::new());
    };
    let data = g.git.object(oid).await.map_err(|e| e.to_string())?;
    let key = crate::git::pointer_key(&data)
        .ok_or("observed index/tree entry is not an annex pointer")?;
    digest(Some(&key))
}

/// The observable projection of one path. An ended aside is a kept inode,
/// even while its physical name remains in `aside/` until release.
fn disk(g: &Group, path: &str, seq: u64, observe_writers: bool) -> Result<Value, String> {
    let wt = g.worktree.as_ref().ok_or("trace has no worktree")?;
    let journal = entries(&crate::journal_file::read(&g.files.install_journal()));
    let mut active_entries = journal
        .values()
        .filter(|e| e.begin.path == path && e.end.is_none());
    let active_entry = active_entries.next();
    if active_entries.next().is_some() {
        return Err("more than one active journal entry for a path".into());
    }
    let entry = active_entry.or_else(|| journal.get(&seq));
    let live = file(&wt.join(path), observe_writers)?;
    let active = entry.filter(|e| e.end.is_none());
    let absent = json!({"inode": "", "content": "", "writers": false, "stat": ""});
    let aside = match active {
        Some(e) => file(Path::new(&e.begin.aside), observe_writers)?,
        None => absent.clone(),
    };
    let stage = match active.and_then(|e| e.begin.stage.as_deref()) {
        Some(p) => file(Path::new(p), observe_writers)?,
        None => absent.clone(),
    };
    let mut kept = Vec::new();
    for e in journal
        .values()
        .filter(|e| e.begin.path == path && e.end.is_some() && !e.released)
    {
        let original = Path::new(&e.begin.aside);
        let at = if lstat(original).map_err(|e| e.to_string())?.is_some() {
            original
        } else {
            e.preserved.as_deref().map(Path::new).unwrap_or(original)
        };
        let mut value = file(at, observe_writers)?;
        if value["inode"] == "" {
            continue;
        }
        value["expected"] = json!(digest(e.begin.old_key.as_deref())?);
        kept.push(value);
    }
    Ok(json!({
        "p": live, "aside": aside, "stage": stage, "kept": kept,
        "open": active.is_some(), "proved": entry.is_some_and(|e| e.proved),
        "operation": active.and_then(|e| e.operation.as_deref()).unwrap_or(""),
        "new": digest(entry.and_then(|e| e.begin.new_key.as_deref()))?,
        "expected": digest(entry.and_then(|e| e.begin.old_key.as_deref()))?,
        "expected_stat": entry.and_then(|e| e.begin.stat).map(|s| format!("{}:{}:{}:{}", s.dev, s.ino, s.size, s.mtime_ns)).unwrap_or_default(),
        "pending": crate::capture_file::pending(g),
    }))
}

/// Git facts and the filesystem layout at an observed operation boundary.
pub async fn snapshot(g: &Group, path: &str, seq: u64) -> Result<Value, String> {
    let mut state = disk(g, path, seq, true)?;
    let index = crate::trace_index::read(g, path)?;
    let hidden = index.hidden;
    let physical_index = pointer(g, index.oid.as_deref()).await?;
    let indexed = match crate::capture_file::read(g).and_then(|i| i.before.get(path).cloned()) {
        Some(entry) => pointer(g, entry.as_ref().map(|e| e.oid.as_str())).await?,
        None => physical_index.clone(),
    };
    let clean = if indexed == physical_index {
        index.clean(&g.worktree.as_ref().ok_or("no worktree")?.join(path))?
    } else {
        false
    };
    let head = g.git.rev("refs/heads/main").await;
    let current = match &head {
        Some(head) => {
            let tree = g.git.tree(head).await.map_err(|e| e.to_string())?;
            pointer(g, tree.get(path).map(|e| e.oid.as_str())).await?
        }
        None => String::new(),
    };
    let allowed = if let (Some(head), Some(doc), Some(me)) =
        (&head, g.roster(), crate::git::signer_device())
    {
        let id = pigeon_core::roster::DeviceId(me);
        let person = &doc
            .roster
            .devices
            .get(&id)
            .ok_or("signing device absent from roster")?
            .person;
        let writers = crate::merge::writers(g, head)
            .await
            .map_err(|e| e.to_string())?;
        pigeon_core::authority::allowed(&doc.roster, &writers, person, path, current.is_empty())
    } else {
        return Err("trace has no authority checkpoint".to_owned());
    };
    let signed: BTreeSet<String> = crate::retention::keys(g)
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .map(|k| digest(Some(k)))
        .collect::<Result<_, _>>()?;
    let div = crate::conflicts_file::read(g).ok_or("unreadable divergence table")?;
    let div = div.paths.get(path);
    state["index"] = json!(indexed);
    state["physical_index"] = json!(physical_index);
    state["index_oid"] = json!(index.oid.as_deref().unwrap_or(""));
    state["clean"] = json!(clean);
    state["hidden"] = json!(hidden);
    state["current"] = json!(current);
    state["signed"] = json!(signed);
    state["allowed"] = json!(allowed);
    let recent = g
        .worktree
        .as_ref()
        .and_then(|wt| lstat(&wt.join(path)).ok().flatten())
        .is_some_and(|st| crate::capture::soon_after_install(g, path, st.mtime_ns));
    state["recent"] = json!(recent);
    state["divergence"] = json!(div.map_or("none", |d| match d.state {
        crate::conflicts_file::State::Waiting => "waiting",
        crate::conflicts_file::State::Conflict => "conflict",
    }));
    state["mine"] = json!(digest(div.and_then(|d| d.mine.as_deref()))?);
    Ok(state)
}

/// Before/after observations bracket the actual operation. A killed
/// process leaves an unmatched before, which recovery must explain.
pub struct Span<'a> {
    tracer: &'a Tracer,
    group: &'a Group,
    path: String,
    seq: u64,
    action: &'static str,
    kind: &'static str,
    requested: Option<String>,
    finished: bool,
}

impl<'a> Span<'a> {
    /// Begin observing an operation, without I/O when tracing is off.
    pub async fn begin(
        tracer: &'a Tracer,
        group: &'a Group,
        path: &str,
        seq: u64,
        action: &'static str,
        kind: &'static str,
    ) -> Self {
        Self::begin_request(tracer, group, path, seq, (action, kind), None).await
    }

    /// Include the actual requested key when it differs from main/index.
    pub async fn begin_request(
        tracer: &'a Tracer,
        group: &'a Group,
        path: &str,
        seq: u64,
        action_kind: (&'static str, &'static str),
        requested: Option<&str>,
    ) -> Self {
        let (action, kind) = action_kind;
        let span = Self {
            tracer,
            group,
            path: path.to_owned(),
            seq,
            action,
            kind,
            requested: requested.map(str::to_owned),
            finished: false,
        };
        span.record("before", "", true).await;
        span
    }

    async fn record(&self, edge: &str, phase: &str, ok: bool) {
        if !self.tracer.enabled() {
            return;
        }
        let observation = snapshot(self.group, &self.path, self.seq).await;
        let vars = match observation {
            Ok(state) => json!({"state": state}),
            Err(error) => json!({"error": crate::util::hex(error.as_bytes())}),
        };
        let mut vars = vars;
        vars["ok"] = json!(ok);
        self.emit(edge, phase, vars);
    }

    fn emit(&self, edge: &str, phase: &str, mut vars: Value) {
        vars["group"] = json!(self.group.id);
        vars["path"] = json!(crate::util::hex(self.path.as_bytes()));
        vars["seq"] = json!(self.seq);
        vars["run"] = json!(std::process::id().to_string());
        vars["edge"] = json!(edge);
        vars["phase"] = json!(phase);
        vars["kind"] = json!(self.kind);
        vars["request"] = json!(self.requested.as_deref().unwrap_or(""));
        let spec = match self.kind {
            "free" | "keep" | "evict" => "Evict",
            "thin" => "Thin",
            "restore" => "Restore",
            "resolve" => "Resolve",
            _ => "CaptureInstall",
        };
        self.tracer.observed(spec, self.action, vars);
    }

    /// Observe the real state after the operation, including its outcome.
    pub async fn finish(mut self, phase: &str) {
        self.record("after", phase, true).await;
        self.finished = true;
    }

    /// Observe an operation which returned an error. It may have made no
    /// abstract change, or completed just before a durability check failed.
    pub async fn interrupted(mut self, phase: &str) {
        self.record("after", phase, false).await;
        self.finished = true;
    }
}

impl Drop for Span<'_> {
    fn drop(&mut self) {
        if self.tracer.enabled() && !self.finished {
            self.emit("error", "", json!({"interrupted": true}));
        }
    }
}

/// Git facts are fixed by the group's busy lock during these synchronous
/// filesystem steps. Prepare them before taking the free-target mutex;
/// each edge still reads fresh files and journal records under that mutex.
/// This avoids holding a non-Send mutex guard across an async observation.
pub struct Checkpoint<'a> {
    tracer: &'a Tracer,
    group: &'a Group,
    path: String,
    seq: u64,
    kind: &'static str,
    state: Option<Result<Value, String>>,
}

impl<'a> Checkpoint<'a> {
    /// Prepare read-only Git facts, with no I/O when tracing is disabled.
    pub async fn new(
        tracer: &'a Tracer,
        group: &'a Group,
        path: &str,
        seq: u64,
        kind: &'static str,
    ) -> Self {
        let state = if tracer.enabled() {
            Some(snapshot(group, path, seq).await)
        } else {
            None
        };
        Self {
            tracer,
            group,
            path: path.to_owned(),
            seq,
            kind,
            state,
        }
    }

    /// Observe the beginning of one synchronous step.
    pub fn begin(&self, action: &'static str) -> DiskSpan<'_> {
        self.begin_with_scan(action, None)
    }

    /// Record the actual driver scan input, without repeating a process
    /// scan for every file at every edge of the rename window.
    pub fn scan(&self, clear: bool) -> DiskSpan<'_> {
        self.begin_with_scan("Scan", Some(!clear))
    }

    fn begin_with_scan(&self, action: &'static str, writers: Option<bool>) -> DiskSpan<'_> {
        let span = Span {
            tracer: self.tracer,
            group: self.group,
            path: self.path.clone(),
            seq: self.seq,
            action,
            kind: self.kind,
            requested: None,
            finished: false,
        };
        let step = DiskSpan {
            checkpoint: self,
            span,
            writers,
        };
        step.record("before", "", true);
        step
    }
}

/// A step inside a critical section which changes files and journal only.
pub struct DiskSpan<'a> {
    checkpoint: &'a Checkpoint<'a>,
    span: Span<'a>,
    writers: Option<bool>,
}

impl DiskSpan<'_> {
    fn record(&self, edge: &str, phase: &str, ok: bool) {
        let Some(base) = &self.checkpoint.state else {
            return;
        };
        let observed = base.clone().and_then(|mut base| {
            let mut fresh = disk(self.span.group, &self.span.path, self.span.seq, false)?;
            if edge == "before"
                && let Some(writers) = self.writers
                && fresh["aside"]["inode"] != ""
            {
                fresh["aside"]["writers"] = json!(writers);
            }
            let index = crate::trace_index::read(self.span.group, &self.span.path)?;
            if base["index_oid"].as_str() != Some(index.oid.as_deref().unwrap_or("")) {
                return Err("index pointer changed during a filesystem step".to_owned());
            }
            base["clean"] = json!(
                index.clean(
                    &self
                        .span
                        .group
                        .worktree
                        .as_ref()
                        .ok_or("no worktree")?
                        .join(&self.span.path)
                )?
            );
            base["hidden"] = json!(index.hidden);
            for (k, v) in fresh.as_object().ok_or("invalid disk observation")? {
                base[k] = v.clone();
            }
            Ok(base)
        });
        let mut vars = match observed {
            Ok(state) => json!({"state": state}),
            Err(error) => json!({"error": crate::util::hex(error.as_bytes())}),
        };
        vars["ok"] = json!(ok);
        self.span.emit(edge, phase, vars);
    }

    /// Observe the completed step and its control phase.
    pub fn finish(mut self, phase: &str) {
        self.record("after", phase, true);
        self.span.finished = true;
    }
}
