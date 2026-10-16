//! Several people in the model (M2): joins, and the suggestions a person
//! makes where the authority rule lets them only suggest (04 §4.5).
//!
//! The model follows the default root rule, add-only with the admins as
//! stewards: anyone adds a file, an admin or the person who alone wrote
//! it (ADR 0070) changes one, and another person's change of a file is a
//! suggestion, which one of them accepts into the history or declines. The proposer's file is then theirs alone and
//! is never installed over. Concurrent changes, and the conflict copies
//! they make, are Authority.tla's and the catalogue's (models/catalogue),
//! not this model's: their conditions are observed by the process executor
//! only.

use super::{Model, Ver};
use crate::step::Outcome;
use crate::world::{Device, File, Laptop};

/// A suggestion: a version of `file` that `by` saved but may not make.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    /// The file it changes.
    pub file: File,
    /// The version.
    pub ver: Ver,
    /// The laptop it was saved on.
    pub by: Laptop,
}

impl Model {
    /// Whether laptop `l`'s person may change `file`, which exists: an
    /// admin, or the person who wrote every version of it that `l` knows.
    pub(crate) fn may_change(&self, l: Laptop, file: &File) -> bool {
        let lm = &self.laptops[l.0 as usize];
        lm.admin
            || lm
                .history
                .get(file)
                .is_some_and(|v| !v.is_empty() && v.iter().all(|x| x.by == lm.person))
    }

    /// `by` invites the spare laptop `l`, which joins: the inviter commits
    /// the roster that adds it, and it runs.
    pub(crate) fn join(&mut self, by: Laptop, l: Laptop) -> Outcome {
        let lm = &mut self.laptops[l.0 as usize];
        if lm.present {
            return Outcome::Refused("invite-used".into());
        }
        lm.present = true;
        lm.daemon.running = true;
        self.log("Roster", "Commit", Device::Laptop(by));
        Outcome::Done
    }

    /// Laptop `l` saved `ver` of `file`, a change its person may not make:
    /// it replaces their earlier suggestion for the file, and the file is
    /// theirs until an admin decides.
    pub(crate) fn suggest(&mut self, l: Laptop, file: File, ver: Ver) {
        self.suggestions.retain(|s| !(s.by == l && s.file == file));
        self.laptops[l.0 as usize].diverged.insert(file.clone());
        self.suggestions.push(Suggestion { file, ver, by: l });
    }

    /// A decider's decision on the newest suggestion for `file`: an
    /// adopted one becomes the file's next version, written by its
    /// proposer, in the history the decider knows, which every laptop then
    /// learns.
    pub(crate) fn decide(&mut self, on: Laptop, file: &File, adopt: bool) -> Outcome {
        if !self.may_change(on, file) {
            return Outcome::Refused("not-allowed".into());
        }
        let Some(i) = self.suggestions.iter().rposition(|s| s.file == *file) else {
            return Outcome::Refused("no-such-suggestion".into());
        };
        let s = self.suggestions.remove(i);
        if adopt {
            let now = self.now;
            let lm = &mut self.laptops[on.0 as usize];
            let vers = lm.history.entry(file.clone()).or_default();
            vers.push(Ver {
                saved: now,
                ..s.ver
            });
            let vers = vers.clone();
            self.group.insert(file.clone(), vers);
        }
        Outcome::Done
    }

    /// *Take the group's version*: the file is no longer the person's own,
    /// so keep installs the current version over it; a suggestion of theirs
    /// for it is withdrawn.
    pub(crate) fn take_group(&mut self, on: Laptop, file: &File) -> Outcome {
        if !self.laptops[on.0 as usize].diverged.remove(file) {
            return Outcome::Refused("not-diverging".into());
        }
        self.suggestions
            .retain(|s| !(s.by == on && s.file == *file));
        self.log("Resolve", "Resolve", Device::Laptop(on));
        Outcome::Done
    }

    /// Whether a suggestion for `file` waits.
    pub(crate) fn suggested(&self, file: &File) -> bool {
        self.suggestions.iter().any(|s| s.file == *file)
    }
}
