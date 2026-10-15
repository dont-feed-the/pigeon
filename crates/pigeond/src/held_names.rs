//! Non-portable names held back at capture (05 §5.4, 07 §7.4): a new path
//! whose name is not portable, or that respells a name `main` or another
//! new path has, stays on this computer as a held batch of its own, with a
//! suggested name. Each round looks again: a held path renamed or removed
//! by the person leaves its batch, and one portable again is captured.
//! *Rename to the suggested name* renames the prefix at fault on disk; the
//! next capture adds the files under their new name.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pigeon_core::authority::Tree;
use pigeon_core::names::{hold_back, take_prefixes};

use crate::held_file::{NAME, Table};

/// The names directly in `worktree/parent`, and those `taken` has there.
fn siblings(worktree: &Path, taken: &BTreeMap<String, String>, parent: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(worktree.join(parent))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.extend(
        taken
            .values()
            .filter_map(|s| s.strip_prefix(parent))
            .filter(|rest| !rest.is_empty() && !rest.contains('/'))
            .map(str::to_owned),
    );
    names
}

/// Sifts the additions of a round: answers those capture adds, and holds
/// back the new ones (`fresh`) whose name is not portable, in `held`.
/// Answers whether `held` changed.
pub fn sift(
    worktree: &Path,
    held: &mut Table,
    main: Option<&Tree>,
    add: &mut Vec<String>,
    fresh: &BTreeSet<String>,
    now: i64,
) -> bool {
    let mut taken = BTreeMap::new();
    for p in main.into_iter().flat_map(|t| t.keys()) {
        take_prefixes(&mut taken, p);
    }
    for p in add.iter().filter(|p| !fresh.contains(*p)) {
        take_prefixes(&mut taken, p);
    }
    let before = held.names();
    let mut back = BTreeMap::new();
    add.retain(|p| {
        if !fresh.contains(p) {
            return true;
        }
        match hold_back(p, &taken, |parent| siblings(worktree, &taken, parent)) {
            Some(h) => {
                back.insert(p.clone(), h);
                false
            }
            None => {
                take_prefixes(&mut taken, p);
                true
            }
        }
    });
    let gone: BTreeSet<String> = before
        .iter()
        .filter(|p| !back.contains_key(*p))
        .cloned()
        .collect();
    held.batches
        .retain(|b| b.why != NAME || b.paths.is_disjoint(&gone));
    let changed = !gone.is_empty() || back.keys().any(|p| !before.contains(p));
    for (p, h) in back {
        if !before.contains(&p) {
            tracing::warn!(path = %p, from = %h.from, "name-held");
            held.hold_name(&p, h, now);
        }
    }
    changed
}
