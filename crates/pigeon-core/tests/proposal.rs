//! Laws of adopting a proposal (04 §4.4 rule 7, 04 §4.5; pigeon-core
//! `proposal`), on random trees of a few paths: a base, a proposal that
//! changes some of its paths, and a `main` that changed some meanwhile;
//! and of a decision that accepts some of its files (ADR 0076).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeSet;

use common::{flag, int};
use hegel::TestCase;
use pigeon_core::authority::{Entry, Tree, delta};
use pigeon_core::names::{self, Stamp};
use pigeon_core::proposal::{adoption, adopts, holds, partial};

const PATHS: [&str; 5] = ["a.txt", "b.txt", "d/c.txt", "d/e", "f"];
const WHEN: Stamp = Stamp {
    utc: 1_791_892_920,
    offset: 120,
};

fn entry(oid: &str) -> Entry {
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: oid.into(),
    }
}

/// A tree over `PATHS`, each present with one of a few contents or absent.
fn tree(tc: &TestCase, tag: &str) -> Tree {
    PATHS
        .iter()
        .filter_map(|p| match int(tc, 0, 2) {
            0 => None,
            1 => Some(((*p).to_owned(), entry("same"))),
            _ => Some(((*p).to_owned(), entry(&format!("{tag}-{p}")))),
        })
        .collect()
}

/// `from` with some paths replaced by `other`'s.
fn changed(tc: &TestCase, from: &Tree, other: &Tree) -> Tree {
    let mut t = from.clone();
    for p in PATHS {
        if flag(tc) {
            match other.get(p) {
                Some(e) => t.insert(p.to_owned(), e.clone()),
                None => t.remove(p),
            };
        }
    }
    t
}

fn draw(tc: &TestCase) -> (Tree, Tree, Tree) {
    let base = tree(tc, "base");
    let proposal = changed(tc, &base, &tree(tc, "mine"));
    let main = changed(tc, &base, &tree(tc, "theirs"));
    (base, proposal, main)
}

#[hegel::test(test_cases = 1000)]
fn an_adoption_holds_its_proposals_change(tc: TestCase) {
    let (base, proposal, main) = draw(&tc);
    let a = adoption(&base, &proposal, &main, "Cid", WHEN);
    assert_eq!(adopts(&base, &proposal, &main, &a), Ok(()));
    // `main` itself holds the change only where it made it already.
    let held = delta(&base, &proposal).iter().all(|p| {
        main.get(p) == proposal.get(p) || (main.get(p) != base.get(p) && !proposal.contains_key(p))
    });
    assert_eq!(
        adopts(&base, &proposal, &main, &main).is_ok(),
        held || a == main
    );
}

#[hegel::test(test_cases = 1000)]
fn an_adoption_keeps_what_main_changed_and_adds_only_copies(tc: TestCase) {
    let (base, proposal, main) = draw(&tc);
    let a = adoption(&base, &proposal, &main, "Cid", WHEN);
    let changes = delta(&base, &proposal);
    for p in PATHS {
        if !changes.contains(p) || main.get(p) != base.get(p) {
            assert_eq!(a.get(p), main.get(p), "{p}");
        } else {
            assert_eq!(a.get(p), proposal.get(p), "{p}");
        }
    }
    for (q, e) in &a {
        if !PATHS.contains(&q.as_str()) {
            let of = changes.iter().find(|p| names::is_copy_of(q, p));
            let of = of.unwrap_or_else(|| panic!("{q} is no copy of a changed path"));
            assert_eq!(proposal.get(of), Some(e));
            assert!(q.contains("Cid"), "{q}");
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn adopting_on_an_unchanged_main_is_the_proposal(tc: TestCase) {
    let (base, proposal, _) = draw(&tc);
    assert_eq!(adoption(&base, &proposal, &base, "Cid", WHEN), proposal);
    // And adopting twice adds nothing.
    let (base, proposal, main) = draw(&tc);
    let once = adoption(&base, &proposal, &main, "Cid", WHEN);
    let twice = adoption(&base, &proposal, &once, "Cid", WHEN);
    assert_eq!(twice, once);
}

/// Some of the paths of `changes`.
fn some(tc: &TestCase, changes: &BTreeSet<String>) -> BTreeSet<String> {
    changes.iter().filter(|_| flag(tc)).cloned().collect()
}

#[hegel::test(test_cases = 1000)]
fn a_partial_decision_is_the_adoption_on_the_files_it_accepts(tc: TestCase) {
    let (base, proposal, main) = draw(&tc);
    let changes = delta(&base, &proposal);
    let all = adoption(&base, &proposal, &main, "Cid", WHEN);
    let none = BTreeSet::new();
    assert_eq!(partial(&base, &proposal, &main, "Cid", WHEN, &changes), all);
    assert_eq!(partial(&base, &proposal, &main, "Cid", WHEN, &none), main);
    let accepted = some(&tc, &changes);
    let t = partial(&base, &proposal, &main, "Cid", WHEN, &accepted);
    for p in &changes {
        if accepted.contains(p) {
            assert!(holds(&base, &proposal, &main, &t, p), "{p}");
            assert_eq!(t.get(p), all.get(p), "{p}");
        } else {
            assert_eq!(t.get(p), main.get(p), "{p}");
        }
    }
    // It adds only the conflict copies of the files it accepts, and
    // changes nothing the proposal did not.
    for q in delta(&main, &t) {
        let of = changes
            .iter()
            .find(|p| **p == q || names::is_copy_of(&q, p));
        let of = of.unwrap_or_else(|| panic!("{q} is no change of the proposal"));
        assert!(accepted.contains(of), "{q} of {of} was declined");
    }
}
