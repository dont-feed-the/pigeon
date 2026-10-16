//! Local folder choices (08 §8.5): the nearest explicit Free or Keep
//! applies to a path, with Keep as the default. Choosing a folder replaces
//! its descendants' older choices and preserves all choices outside it.
//! Laws: scope, last request, inheritance and unrelated-choice independence.

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

    /// Explicit Free scopes; callers still check each path's nearest choice.
    pub fn free_roots(&self) -> impl Iterator<Item = &String> {
        self.0
            .iter()
            .filter_map(|(path, freed)| freed.then_some(path))
    }
}
