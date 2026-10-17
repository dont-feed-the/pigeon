//! Diagnose a fresh reference to previously releasable contents (08 §8.6).
//! Recompute from signed history, original seals and reference births; no
//! persisted key tombstone or exception to retention/content gates exists.
//! A diagnostic never proves that any particular physical drop happened.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::purge::View;

use crate::{group::Group, purge_index::Index, purge_state::Active};

fn hash(key: &str) -> Option<String> {
    crate::net::sync::key_sha256(key).map(str::to_ascii_lowercase)
}

/// Keys needed now whose contents qualified for release before their new
/// reference. Unrelated uncovered histories remain conservative blockers.
pub(crate) async fn renewed(
    g: &Group,
    index: &Index,
    view: &View<'_, String, String, String>,
    active: &BTreeMap<String, Active>,
) -> BTreeSet<String> {
    if view.current.is_empty() || view.decisions.is_empty() {
        return BTreeSet::new();
    }
    let roots: BTreeSet<_> = view
        .decisions
        .values()
        .flat_map(|d| std::iter::once(&d.base).chain(d.proposals.iter()))
        .chain(view.seals.values().map(|s| &s.main))
        .chain(view.epochs.values().map(|e| &e.main))
        .collect();
    let ancestry: BTreeMap<_, _> = roots
        .into_iter()
        .filter_map(|tip| index.ancestors(tip).map(|set| (tip.clone(), set)))
        .collect();
    let contains = |tip: &String, ancestor: &String| {
        ancestry.get(tip).is_some_and(|set| set.contains(ancestor))
    };
    let covered: BTreeSet<String> = view
        .references
        .iter()
        .filter(|r| view.decisions.values().any(|d| d.covers(r, &contains)))
        .filter_map(|r| hash(&r.key))
        .collect();
    let mut renewed_hashes = BTreeSet::new();
    let mut checked = BTreeSet::new();
    for reference in view
        .references
        .iter()
        .filter(|r| view.current.contains(&r.key))
    {
        let Some(content) = hash(&reference.key) else {
            continue;
        };
        if !covered.contains(&content)
            || renewed_hashes.contains(&content)
            || !checked.insert((reference.birth.clone(), content.clone()))
            || view
                .decisions
                .values()
                .any(|d| d.covers(reference, &contains))
        {
            continue;
        }
        let Some(parents) = index.parents(&reference.birth) else {
            continue;
        };
        let later = index.descendants(&reference.birth);
        let references = view
            .references
            .iter()
            .filter(|r| !later.contains(&r.birth))
            .cloned()
            .collect();
        for parent in parents {
            let Some(parent_ancestors) = index.ancestors(parent) else {
                continue;
            };
            let earlier_contains = |tip: &String, ancestor: &String| {
                if tip == parent {
                    parent_ancestors.contains(ancestor)
                } else {
                    contains(tip, ancestor)
                }
            };
            // Unknown historical evidence suppresses this diagnostic only;
            // the separately verified current release policy is unchanged.
            let Ok(current) = g.git.keys(parent).await else {
                continue;
            };
            let current = current.into_values().collect();
            let Ok(membership) = crate::membership::read(g, parent).await else {
                continue;
            };
            let decisions = view
                .decisions
                .iter()
                .filter(|(id, _)| {
                    active
                        .get(*id)
                        .is_some_and(|a| earlier_contains(parent, &a.introduced))
                })
                .map(|(id, decision)| (id.clone(), decision.clone()))
                .collect();
            let earlier = View {
                main: parent,
                references: &references,
                current: &current,
                decisions: &decisions,
                custodians: &membership.eligible,
                seals: view.seals,
                epochs: view.epochs,
            };
            let released = earlier.released(earlier_contains);
            // Different annex extensions can share one physical pool object.
            // A still-retained alias means those bytes did not qualify.
            if released
                .iter()
                .any(|key| hash(key).as_ref() == Some(&content))
                && !earlier
                    .references
                    .iter()
                    .any(|r| !released.contains(&r.key) && hash(&r.key).as_ref() == Some(&content))
            {
                renewed_hashes.insert(content.clone());
                break;
            }
        }
    }
    view.current
        .iter()
        .filter(|key| hash(key).is_some_and(|content| renewed_hashes.contains(&content)))
        .cloned()
        .collect()
}
