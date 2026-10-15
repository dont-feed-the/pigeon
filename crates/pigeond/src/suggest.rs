//! Capture's suggestions (07 §7.3 steps 5 and 7, 07 §7.6): the changes of a
//! batch that this person may not make under `main`'s roster go to
//! proposals, one per set of deciders (`authority::split`), while the
//! others make the commit on `main`. A change of a path with a pending
//! proposal of this device stacks a new proposal on it. Each suggested
//! path keeps `main`'s version in the index and becomes a `suggestion` of
//! the divergence table, waiting, with the person's key and the file's
//! stat, so that the next capture leaves it alone until it is saved again.

use std::collections::BTreeMap;
use std::path::Path;

use pigeon_core::authority::{self, Entry, Tree};
use pigeon_core::roster::DeviceId;

use crate::conflicts_file::{Divergence, Kind, State, Table};
use crate::git::{Error, pointer_key};
use crate::group::Group;

/// The person's changes: path → its new entry, `None` for a deletion.
pub type Changes = BTreeMap<String, Option<Entry>>;

/// One proposal to write: its parent, its tree and the paths it suggests.
pub struct Proposal {
    /// `main`'s commit, or the tip of this device's chain it stacks on.
    pub parent: String,
    /// Its tree.
    pub tree: Tree,
    /// The paths it suggests.
    pub paths: Vec<String>,
}

/// `tree` with `changes` applied.
pub fn applied(tree: &Tree, changes: &Changes) -> Tree {
    let mut t = tree.clone();
    for (p, e) in changes {
        match e {
            Some(e) => t.insert(p.clone(), e.clone()),
            None => t.remove(p),
        };
    }
    t
}

/// Splits `changes` of `main` (commit `head`, tree `main`) by device `me`:
/// the changes for `main`, and the proposals. Without a roster that knows
/// this device, every change is for `main`, which accept judges anyway.
pub async fn plan(
    g: &Group,
    me: &str,
    head: &str,
    main: &Tree,
    table: &Table,
    changes: Changes,
) -> Result<(Changes, Vec<Proposal>), Error> {
    let Some(doc) = g.roster() else {
        return Ok((changes, Vec::new()));
    };
    let me_id = DeviceId(me.to_owned());
    if !doc.roster.devices.contains_key(&me_id) {
        return Ok((changes, Vec::new()));
    }
    let split = authority::split(&doc.roster, &me_id, main, &changes);
    let mut out = Vec::new();
    for part in split.proposed.into_values() {
        let mut chains: BTreeMap<Option<String>, Changes> = BTreeMap::new();
        for (p, e) in part {
            let tip = match table.paths.get(&p) {
                Some(d) if d.kind == Kind::Suggestion => d.proposal.clone(),
                _ => None,
            };
            let tip = match tip {
                Some(t) if g.git.rev(&crate::proposal::name(me, &t)).await.is_some() => Some(t),
                _ => None,
            };
            chains.entry(tip).or_default().insert(p, e);
        }
        for (tip, part) in chains {
            let (parent, base) = match tip {
                Some(t) => {
                    let tree = g.git.tree(&t).await?;
                    (t, tree)
                }
                None => (head.to_owned(), main.clone()),
            };
            out.push(Proposal {
                parent,
                tree: applied(&base, &part),
                paths: part.into_keys().collect(),
            });
        }
    }
    Ok((split.allowed, out))
}

async fn key_of(g: &Group, e: Option<&Entry>) -> Option<String> {
    pointer_key(&g.git.object(&e?.oid).await.ok()?)
}

/// Records that `commit` suggests `paths`: each becomes a waiting
/// `suggestion` in `table`, and takes `main`'s version in the index again.
pub async fn record(
    g: &Group,
    worktree: &Path,
    main: &Tree,
    table: &mut Table,
    commit: &str,
    paths: &[String],
    changes: &Changes,
) -> Result<(), Error> {
    for p in paths {
        let group = key_of(g, main.get(p)).await;
        let mine = key_of(g, changes.get(p).and_then(Option::as_ref)).await;
        let stat = crate::evict::stat_of(&worktree.join(p));
        let old = table.paths.get(p).filter(|d| d.kind == Kind::Suggestion);
        let entry = Divergence {
            kind: Kind::Suggestion,
            since: old.map_or_else(
                || crate::util::rfc3339(crate::util::now()),
                |d| d.since.clone(),
            ),
            state: State::Waiting,
            mine,
            mine_stat: stat.map(|s| [s.dev.into(), s.ino.into(), s.size.into(), s.mtime_ns]),
            proposal: Some(commit.to_owned()),
            group: group.clone(),
            installed: group,
            told: old.is_some_and(|d| d.told),
        };
        table.paths.insert(p.clone(), entry);
        match main.get(p) {
            Some(e) => {
                let info = format!("{},{},{p}", e.mode, e.oid);
                g.git
                    .run(&["update-index", "--add", "--cacheinfo", &info])
                    .await?;
            }
            None => {
                g.git
                    .run(&["update-index", "--force-remove", "--", p])
                    .await?;
            }
        }
    }
    Ok(())
}

/// Whether `path`, saved as `table` says it was suggested, is unchanged
/// since: the same file, or still gone.
pub fn unchanged(worktree: &Path, table: &Table, path: &str, gone: bool) -> bool {
    let Some(d) = table.paths.get(path) else {
        return false;
    };
    if d.kind != Kind::Suggestion || d.proposal.is_none() {
        return false;
    }
    if gone {
        return d.mine.is_none();
    }
    let stat = crate::evict::stat_of(&worktree.join(path));
    d.mine.is_some()
        && stat.map(|s| [s.dev.into(), s.ino.into(), s.size.into(), s.mtime_ns]) == d.mine_stat
}
