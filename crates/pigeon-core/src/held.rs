//! Held mass deletions (07 §7.3 step 3, §7.4): which deletions of one
//! capture round wait for the person instead of reaching `main`.
//!
//! A round's deletions are held, all of them, when with those already held
//! they are more than [`MASS_DELETION`] files, or when one of them is in a
//! top-level folder that no longer exists. Deletions the person shared
//! (*Delete them for everyone*) always go through. Laws
//! (`tests/held.rs`): what goes and what waits split the round's
//! deletions; nothing held goes; what goes unasked stays within the limit;
//! a larger round is held whenever a smaller one is.

use std::collections::BTreeSet;

/// More deletions than this, with those already held, are held (07 §7.3).
pub const MASS_DELETION: usize = 100;

/// Why a round's deletions are held.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Why {
    /// More than the limit, with those already held.
    Mass,
    /// A top-level folder was removed: its name.
    TopFolder(String),
}

/// The top-level folder of `path`, if it is in one.
pub fn top(path: &str) -> Option<&str> {
    path.split_once('/').map(|(t, _)| t)
}

/// What one round does with its deletions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Split {
    /// Deletions that reach `main` this round.
    pub delete: BTreeSet<String>,
    /// Deletions newly held, and why; empty when none is.
    pub hold: BTreeSet<String>,
    /// Why `hold` is held.
    pub why: Option<Why>,
}

/// Splits `deleted`, the round's deletions (renames left out), given
/// those already `held`, those the person `released`, and the top-level
/// folders `gone` from the folder, with `limit` as [`MASS_DELETION`].
pub fn split(
    limit: usize,
    deleted: &BTreeSet<String>,
    held: &BTreeSet<String>,
    released: &BTreeSet<String>,
    gone: &BTreeSet<String>,
) -> Split {
    let mut out = Split::default();
    let mut rest = BTreeSet::new();
    for p in deleted {
        if released.contains(p) {
            out.delete.insert(p.clone());
        } else if !held.contains(p) {
            rest.insert(p.clone());
        }
    }
    if rest.is_empty() {
        return out;
    }
    let waiting = held
        .difference(released)
        .filter(|p| !rest.contains(*p))
        .count();
    let total = waiting + rest.len();
    let folder = rest
        .iter()
        .filter_map(|p| top(p))
        .find(|t| gone.contains(*t))
        .map(str::to_owned);
    out.why = if total > limit {
        Some(Why::Mass)
    } else {
        folder.map(Why::TopFolder)
    };
    if out.why.is_some() {
        out.hold = rest;
    } else {
        out.delete.extend(rest);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(xs: &[&str]) -> BTreeSet<String> {
        xs.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn a_few_deletions_go_and_a_removed_folder_waits() {
        let none = BTreeSet::new();
        let s = split(2, &set(&["a", "b/c"]), &none, &none, &none);
        assert_eq!(s.delete, set(&["a", "b/c"]));
        let s = split(2, &set(&["a", "b/c"]), &none, &none, &set(&["b"]));
        assert_eq!(s.why, Some(Why::TopFolder("b".into())));
        assert_eq!(s.hold, set(&["a", "b/c"]));
        let s = split(2, &set(&["x"]), &set(&["a", "b"]), &none, &none);
        assert_eq!(s.why, Some(Why::Mass));
        let s = split(
            2,
            &set(&["a", "b", "x"]),
            &set(&["a", "b"]),
            &set(&["a", "b"]),
            &none,
        );
        assert_eq!((s.delete, s.why), (set(&["a", "b", "x"]), None));
    }
}
