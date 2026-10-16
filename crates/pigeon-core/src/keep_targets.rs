//! Local folder choices (08 §8.5): the nearest explicit Free or Keep
//! applies to a path, with Keep as the default. Choosing a folder replaces
//! its descendants' older choices and preserves all choices outside it.
//! Other people's folders of a per-person folder are Free by default
//! ([`others_folders`]), unless a choice of the folder or one holding it
//! says otherwise. Laws: scope, last request, inheritance, unrelated-choice
//! independence, and defaults (`tests/keep_targets.rs`).

use std::collections::BTreeMap;

/// Explicit local choices; `true` means Free, `false` means Keep.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Targets(pub BTreeMap<String, bool>);

impl Targets {
    /// Whether this path is to be freed, including future children.
    pub fn is_freed(&self, path: &str) -> bool {
        self.0
            .iter()
            .filter(|(prefix, _)| crate::restore::under(path, prefix))
            .max_by_key(|(prefix, _)| prefix.len())
            .is_some_and(|(_, freed)| *freed)
    }

    /// Applies one request, overriding older choices inside its scope.
    pub fn set(&mut self, prefix: &str, freed: bool) {
        self.0
            .retain(|path, _| !crate::restore::under(path, prefix));
        self.0.insert(prefix.to_owned(), freed);
    }

    /// These choices with each of `defaults` freed where no choice of it,
    /// or of a folder holding it, was made; a choice inside it still wins.
    #[must_use]
    pub fn with_defaults(mut self, defaults: impl IntoIterator<Item = String>) -> Targets {
        for folder in defaults {
            if !self
                .0
                .keys()
                .any(|chosen| crate::restore::under(&folder, chosen))
            {
                self.0.insert(folder, true);
            }
        }
        self
    }

    /// Explicit Free scopes; callers still check each path's nearest choice.
    pub fn free_roots(&self) -> impl Iterator<Item = &String> {
        self.0
            .iter()
            .filter_map(|(path, freed)| freed.then_some(path))
    }
}

/// The folders of `roster`'s per-person folders that belong to someone
/// other than `me`, as the folder rules resolve them (04 §4.3): what a
/// computer of `me` does not keep unless it chooses to.
pub fn others_folders(roster: &crate::roster::Roster, me: &crate::roster::PersonId) -> Vec<String> {
    use crate::roster::Rule;
    let mut out = Vec::new();
    for f in roster.folders.iter().filter(|f| f.rule == Rule::PerPerson) {
        for id in roster.persons.keys().filter(|id| *id != me) {
            let folder = format!("{}/{}", f.path, id.0);
            if crate::authority::rule_of(roster, &folder) == Some(Rule::Owned(id.clone())) {
                out.push(folder);
            }
        }
    }
    out
}
