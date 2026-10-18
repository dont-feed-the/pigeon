//! Discover background candidates without Git's clean filters (15 §15.5).
//! Cached index stats and lstat can defer a same-size edit before any read;
//! Git lists untracked names without examining their file contents.

use std::collections::BTreeSet;

use pigeon_git::status::Entry;

use crate::git::{Error, is_pigeon_path};
use crate::group::Group;

fn invalid(detail: String) -> Error {
    Error::Parse {
        argv: vec!["capture index observation".into()],
        detail,
    }
}

pub(crate) async fn changes(g: &Group) -> Result<Vec<Entry>, Error> {
    let began = std::time::Instant::now();
    let Some(wt) = &g.worktree else {
        return Ok(Vec::new());
    };
    crate::capture_schedule::load(g);
    let head_ref = g.git.rev("HEAD").await;
    // A freshly initialized, unborn repository legitimately has no index.
    // Once HEAD exists, an unreadable/missing index remains an error.
    let unborn = head_ref.is_none()
        && pigeon_sys::fs::lstat(&g.git_dir.join("index"))
            .map_err(|e| invalid(e.to_string()))?
            .is_none();
    let index = if unborn {
        Default::default()
    } else {
        crate::trace_index::read_all(g).map_err(invalid)?
    };
    let head = match head_ref {
        Some(head) => g.git.tree(&head).await?,
        None => Default::default(),
    };
    let mut out = Vec::new();
    for (path, cached) in &index {
        if is_pigeon_path(path) {
            continue;
        }
        let file = wt.join(path);
        let stat = pigeon_sys::fs::lstat(&file).map_err(|e| invalid(e.to_string()))?;
        if cached.hidden && stat.is_none() {
            continue;
        }
        let entry = cached.entry();
        let verified = stat
            .as_ref()
            .zip(entry.as_ref())
            .is_some_and(|(stat, entry)| g.lock().capture_schedule.verified(path, *stat, entry));
        let clean = (verified || cached.clean(&file).map_err(invalid)?)
            && stat.as_ref().is_none_or(|s| {
                entry
                    .as_ref()
                    .is_some_and(|e| (e.mode == "100755") == (s.mode & 0o111 != 0))
            });
        let x = if entry.as_ref() == head.get(path) {
            '.'
        } else if head.contains_key(path) {
            'M'
        } else {
            'A'
        };
        let y = if stat.is_none() {
            'D'
        } else if clean {
            '.'
        } else {
            'M'
        };
        if x == '.' && y == '.' {
            continue;
        }
        out.push(Entry::Changed {
            xy: format!("{x}{y}"),
            mode_worktree: stat.map_or_else(|| "000000".into(), |s| format!("{:o}", s.mode)),
            oid_index: cached.oid.clone().unwrap_or_default(),
            path: path.clone(),
        });
    }
    // One Git index observation lists cached and untracked names together.
    // A concurrent stage after our earlier index read must not make a new
    // user file vanish between those two independently timed observations.
    let names = g
        .git
        .repo
        .run(
            &["ls-files", "--cached", "--others", "-z"],
            None,
            pigeon_git::DEFAULT_TIMEOUT,
        )
        .await?;
    let mut listed = BTreeSet::new();
    for bytes in names.stdout.split(|b| *b == 0).filter(|b| !b.is_empty()) {
        let path = std::str::from_utf8(bytes).map_err(|e| invalid(e.to_string()))?;
        if !index.contains_key(path) && listed.insert(path.to_owned()) {
            out.push(Entry::Untracked {
                path: path.to_owned(),
            });
        }
    }
    // A staged deletion with a visible file is untracked above. A truly
    // deleted index entry has no local save to stage, as with Git status.
    tracing::debug!(
        entries = index.len(),
        pending = out.len(),
        elapsed_us = u64::try_from(began.elapsed().as_micros()).unwrap_or(u64::MAX),
        "candidate-scan-complete"
    );
    Ok(out)
}
