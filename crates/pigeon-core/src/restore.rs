//! Restore selection (10 §10.1, ADR 0034). A folder comes from one
//! accepted snapshot, never a mixture of different moments. Selecting
//! content never requests deletions outside or inside the folder.

use std::collections::BTreeMap;

/// A snapshot in the caller's accepted history, newest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot<C, V> {
    /// Accepted commit identifying this complete tree.
    pub commit: C,
    /// Its author time.
    pub at: i64,
    /// Saved file versions in that tree.
    pub files: BTreeMap<String, V>,
}

/// Exact path or descendant; similarly named siblings never match.
pub fn under(path: &str, prefix: &str) -> bool {
    prefix.is_empty()
        || path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|tail| tail.starts_with('/'))
}

/// Latest qualifying nonempty snapshot in accepted traversal order.
/// A cutoff is inclusive, and a deletion's empty tree is skipped.
pub fn select<C, V>(
    history: &[Snapshot<C, V>],
    prefix: &str,
    before: Option<i64>,
) -> Option<usize> {
    history.iter().position(|snapshot| {
        before.is_none_or(|time| snapshot.at <= time)
            && snapshot.files.keys().any(|path| under(path, prefix))
    })
}

/// Restores only the selected folder's saved files. Existing newer paths
/// absent from this snapshot are never deleted.
pub fn files<C, V: Clone>(snapshot: &Snapshot<C, V>, prefix: &str) -> BTreeMap<String, V> {
    snapshot
        .files
        .iter()
        .filter(|(path, _)| under(path, prefix))
        .map(|(path, version)| (path.clone(), version.clone()))
        .collect()
}
