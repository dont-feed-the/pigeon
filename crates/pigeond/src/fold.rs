//! Roster forks (04 §4.8, ADR 0051): the accepted commits' roster changes,
//! kept as a `roster::History` beside the catalogue engine, and the fold
//! that gives the roster of a merge of parents whose rosters differ.
//!
//! Each accepted commit is recorded with its change δ: for a commit with
//! one roster among its parents, the edits from that roster to its tree's
//! (`roster::diff`); for a merge of forks, none, its tree holding the fold.
//! A tree's roster is R(c) for its children, so a fork's merge must hold
//! exactly `History::fold` of its history (`TreeIsFold`); the laws of the
//! fold are pigeon-core's (`tests/roster.rs`, `Roster.tla`).

use pigeon_core::roster::{Commit, DeviceId, Edit, Folded, History, Roster};

use crate::group::Group;

/// Records the genesis `id` and its roster.
pub fn genesis(g: &Group, id: &str, roster: &Roster) {
    g.lock().history = Some(History {
        genesis: id.to_owned(),
        roster: roster.clone(),
        commits: std::collections::BTreeMap::new(),
    });
}

/// Records the accepted commit `id` with its accepted `parents`, author
/// time, signer and change.
pub fn record(
    g: &Group,
    id: &str,
    parents: &[String],
    time: i64,
    signer: &DeviceId,
    change: Vec<Edit>,
) {
    if let Some(h) = g.lock().history.as_mut() {
        h.commits.insert(
            id.to_owned(),
            Commit {
                parents: parents.to_vec(),
                time,
                signer: signer.clone(),
                change,
            },
        );
    }
}

/// The fold of a merge of `parents` that changes no roster itself.
pub fn of_merge(g: &Group, parents: &[String]) -> Option<Folded<String>> {
    let mut k = g.lock();
    let h = k.history.as_mut()?;
    let id = "merge being judged".to_owned();
    h.commits.insert(
        id.clone(),
        Commit {
            parents: parents.to_vec(),
            time: 0,
            signer: DeviceId(String::new()),
            change: Vec::new(),
        },
    );
    let folded = h.fold(&id);
    h.commits.remove(&id);
    Some(folded)
}
