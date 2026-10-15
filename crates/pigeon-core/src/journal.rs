//! The install journal and its recovery (07 §7.5, spec 07 §7.5).
//!
//! Install appends `begin`, then `proved` once a batch's checks pass, then
//! an end (`done`, `skip`, `raced`, `recovered`), then `released` once the
//! aside is gone. Recovery at start decides, for each entry without an end,
//! from the journal and what it finds on disk: a proved entry rolls forward,
//! any other rolls back. Staged files no entry names are removed.
//!
//! Laws (`tests/journal.rs`, against a model of the disk): after recovery
//! every entry has ended; a second recovery does nothing; a crash during
//! recovery, then recovery again, ends as one recovery would; each path ends
//! at its old content, the new one or the person's, and a person's content
//! is never lost; the index agrees with what pigeon left (INV-Journal).

use std::collections::BTreeMap;

/// The stat of a file that install proves its own: `(dev, ino, size, mtime)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Stat {
    /// Device.
    pub dev: u64,
    /// Inode.
    pub ino: u64,
    /// Size in bytes.
    pub size: u64,
    /// Modification time, in nanoseconds.
    pub mtime_ns: i128,
}

/// The `begin` record of one path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Begin {
    /// The entry's sequence number, unique in the journal.
    pub seq: u64,
    /// The path in the folder.
    pub path: String,
    /// The key of the index version, `None` for a creation.
    pub old_key: Option<String>,
    /// The key to install, `None` for a deletion.
    pub new_key: Option<String>,
    /// The index entry's stat, `None` for a creation.
    pub stat: Option<Stat>,
    /// The staged new file, `None` for a deletion.
    pub stage: Option<String>,
    /// Where the old file is moved aside.
    pub aside: String,
}

/// How an entry ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum End {
    /// The new version is at the path and in the index.
    Done,
    /// Nothing changed: the person's file, or the old one, is at the path.
    Skip,
    /// A person's file took the path; the staged file was dropped.
    Raced,
    /// The aside differs from the old version: it is captured.
    Recovered,
}

/// One record of `.git/pigeon/install.ndjson`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    /// An entry starts.
    Begin(Begin),
    /// The checks of these entries passed, before any rename in.
    Proved(Vec<u64>),
    /// An entry ends.
    End(u64, End),
    /// The aside's keep period started, in seconds since the Unix epoch.
    Kept(u64, i64),
    /// A changed aside is preserved at this non-replacing destination.
    Preserved(u64, String),
    /// An entry's aside was unlinked or captured.
    Released(u64),
}

/// An entry, folded from its records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Its `begin`.
    pub begin: Begin,
    /// Whether `proved` was journalled.
    pub proved: bool,
    /// Its end, if any.
    pub end: Option<End>,
    /// Whether its aside was released.
    pub released: bool,
    /// The durable start of the keep period, absent in older journals.
    pub kept_at: Option<i64>,
    /// Where a changed aside is preserved before capturing it.
    pub preserved: Option<String>,
}

/// Fold records into entries by sequence number. Records about an unknown
/// entry are ignored; the first end of an entry wins.
pub fn entries(records: &[Record]) -> BTreeMap<u64, Entry> {
    let mut out: BTreeMap<u64, Entry> = BTreeMap::new();
    for r in records {
        match r {
            Record::Begin(b) => {
                out.entry(b.seq).or_insert_with(|| Entry {
                    begin: b.clone(),
                    proved: false,
                    end: None,
                    released: false,
                    kept_at: None,
                    preserved: None,
                });
            }
            Record::Proved(seqs) => {
                for s in seqs {
                    if let Some(e) = out.get_mut(s) {
                        e.proved = true;
                    }
                }
            }
            Record::End(s, end) => {
                if let Some(e) = out.get_mut(s) {
                    e.end.get_or_insert(*end);
                }
            }
            Record::Released(s) => {
                if let Some(e) = out.get_mut(s) {
                    e.released = true;
                }
            }
            Record::Kept(s, at) => {
                if let Some(e) = out.get_mut(s) {
                    e.kept_at.get_or_insert(*at);
                }
            }
            Record::Preserved(s, path) => {
                if let Some(e) = out.get_mut(s) {
                    e.preserved = Some(path.clone());
                }
            }
        }
    }
    out
}

/// The journal may be truncated: every entry has ended and its aside is
/// released.
pub fn can_truncate(entries: &BTreeMap<u64, Entry>) -> bool {
    entries.values().all(|e| e.end.is_some() && e.released)
}

/// What recovery finds on disk for one entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    /// Some file is at the path.
    pub path: bool,
    /// The aside exists.
    pub aside: bool,
    /// The staged new file exists.
    pub stage: bool,
    /// The aside hashes to the old key (asked only when it decides).
    pub aside_is_old: bool,
}

/// One step of recovery, run in order; an append is fsynced before the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// Step 7: rename the staged file to the path, never over a name.
    RenameIn,
    /// Step 8: write the new pointer to the index, then refresh it.
    Restage,
    /// Put the aside back at the path, never over a name.
    PutAsideBack,
    /// Append the entry's end.
    Append(End),
    /// Remove the staged file of an ended entry.
    RemoveStage,
}

/// The recovery steps of one entry without an end (spec 07 §7.5).
pub fn recover(entry: &Entry, found: Found) -> Vec<Step> {
    let mut steps = Vec::new();
    if entry.end.is_some() {
        if found.stage {
            steps.push(Step::RemoveStage);
        }
        return steps;
    }
    if entry.proved {
        if found.stage && entry.begin.new_key.is_some() {
            if found.path {
                steps.push(Step::Append(End::Raced));
                steps.push(Step::RemoveStage);
                return steps;
            }
            steps.push(Step::RenameIn);
        }
        steps.push(Step::Restage);
        steps.push(Step::Append(End::Done));
    } else if found.aside && !found.path {
        steps.push(Step::PutAsideBack);
        steps.push(Step::Append(End::Skip));
    } else if found.aside {
        let end = if found.aside_is_old {
            End::Raced
        } else {
            End::Recovered
        };
        steps.push(Step::Append(end));
    } else {
        steps.push(Step::Append(End::Skip));
    }
    if found.stage {
        steps.push(Step::RemoveStage);
    }
    steps
}

/// Staged files that no entry names: a crash between writing the new file
/// (step 2) and journalling `begin` (step 3) leaves one. Recovery removes
/// them; no name in the folder ever leads to them.
pub fn orphans<'a>(entries: &BTreeMap<u64, Entry>, staged: &'a [String]) -> Vec<&'a String> {
    staged
        .iter()
        .filter(|f| !entries.values().any(|e| e.begin.stage.as_ref() == Some(*f)))
        .collect()
}

/// What to do with an ended entry's aside at a full scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Release {
    /// Keep it: the keep period has not passed, or a process writes it.
    Wait,
    /// Unlink it, then append `released`.
    Unlink,
    /// Capture it as a recovered version, then append `released`.
    Capture,
}

/// The release rule of step 9: after the keep period, an aside nobody
/// writes is unlinked if it is *linked* or still holds the old key, and
/// captured otherwise.
pub fn release(kept_long_enough: bool, written: bool, linked: bool, holds_old: bool) -> Release {
    if !kept_long_enough || written {
        Release::Wait
    } else if linked || holds_old {
        Release::Unlink
    } else {
        Release::Capture
    }
}
