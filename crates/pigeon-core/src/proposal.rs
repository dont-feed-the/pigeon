//! Adopting a proposal (04 §4.4 rule 7, 04 §4.5) as pure functions. A
//! proposal's change is Δ between the tree of the accepted commit it builds
//! on, its *base*, and its own tree. Its adoption on `main` applies each
//! changed path whose `main` version is still the base's; a path `main`
//! changed meanwhile keeps `main`'s version, and the proposal's goes beside
//! it under a conflict name of its author (05 §5.3), or, for a deletion, is
//! dropped. [`adopts`] is rule 7's "`tree(c)` holds its change". A
//! decision on some files (ADR 0075) applies the change of those paths
//! alone, the same way: [`partial`]; [`holds`] says whether a tree holds
//! the change of one path. Laws (`tests/proposal.rs`): an adoption holds
//! its proposal's change, keeps every version `main` changed and every
//! path the proposal did not change, and is the proposal itself on an
//! unchanged `main`; a partial one is the adoption on the paths it
//! accepts and `main` everywhere else.

use std::collections::BTreeSet;

use crate::authority::{Tree, delta};
use crate::names::{self, Stamp};

/// The tree of the adoption, on `main`'s tree, of the proposal `proposal`
/// built on `base`, whose author is `who` at `when`.
pub fn adoption(base: &Tree, proposal: &Tree, main: &Tree, who: &str, when: Stamp) -> Tree {
    adopt_paths(base, proposal, main, who, when, |_| true)
}

/// The tree, on `main`'s tree, of a decision that accepts the change of
/// the proposal `proposal` on `base` for the paths `accepted` only, as
/// [`adoption`] makes it (ADR 0075): every other path keeps `main`'s
/// version.
pub fn partial(
    base: &Tree,
    proposal: &Tree,
    main: &Tree,
    who: &str,
    when: Stamp,
    accepted: &BTreeSet<String>,
) -> Tree {
    adopt_paths(base, proposal, main, who, when, |p| accepted.contains(p))
}

fn adopt_paths(
    base: &Tree,
    proposal: &Tree,
    main: &Tree,
    who: &str,
    when: Stamp,
    keep: impl Fn(&str) -> bool,
) -> Tree {
    let mut out = main.clone();
    for p in delta(base, proposal).into_iter().filter(|p| keep(p)) {
        let mine = proposal.get(&p);
        if main.get(&p) == base.get(&p) {
            match mine {
                Some(e) => out.insert(p, e.clone()),
                None => out.remove(&p),
            };
            continue;
        }
        let Some(e) = mine else {
            continue;
        };
        if main.get(&p) == Some(e) || out.iter().any(|(q, f)| f == e && names::is_copy_of(q, &p)) {
            continue;
        }
        let taken = |q: &str| out.keys().any(|k| names::fold(k) == names::fold(q));
        let mut n = 1;
        let mut q = names::conflict_name(&p, who, when, n);
        while taken(&q) {
            n += 1;
            q = names::conflict_name(&p, who, when, n);
        }
        out.insert(q, e.clone());
    }
    out
}

/// The changes the adoption of `proposal` on `base` makes on `main`: each
/// path whose entry differs, and whether it adds it. Its deciders are those
/// allowed every one (04 §4.5; `Authority.tla`'s `ADiff`, with
/// `DecidersAt = "decision"`); a conflict copy is an addition in the folder
/// of its path, whatever its name.
pub fn adoption_changes(base: &Tree, proposal: &Tree, main: &Tree) -> Vec<(String, bool)> {
    let a = adoption(base, proposal, main, "-", Stamp { utc: 0, offset: 0 });
    delta(main, &a)
        .into_iter()
        .map(|p| {
            let added = !main.contains_key(&p);
            (p, added)
        })
        .collect()
}

/// Whether `tree` holds the change of `proposal` on `base`, adopted on
/// `main` (rule 7): each changed path has the proposal's version, or
/// `main` changed it meanwhile and, unless the proposal deletes it, a
/// conflict copy of it holds the proposal's version. Else the first path
/// that does not.
pub fn adopts(base: &Tree, proposal: &Tree, main: &Tree, tree: &Tree) -> Result<(), String> {
    match delta(base, proposal)
        .into_iter()
        .find(|p| !holds(base, proposal, main, tree, p))
    {
        Some(p) => Err(p),
        None => Ok(()),
    }
}

/// Whether `tree` holds the change of `proposal` on `base` of `path`,
/// adopted on `main` (rule 7, path by path): its version, or, when `main`
/// changed it meanwhile, a conflict copy of it, or nothing for a deletion.
pub fn holds(base: &Tree, proposal: &Tree, main: &Tree, tree: &Tree, path: &str) -> bool {
    let mine = proposal.get(path);
    if tree.get(path) == mine {
        return true;
    }
    if main.get(path) == base.get(path) {
        return false;
    }
    match mine {
        None => true,
        Some(e) => tree
            .iter()
            .any(|(q, f)| f == e && names::is_copy_of(q, path)),
    }
}
