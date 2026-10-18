//! Bound memoized catalogues by retained collection entries as well as
//! states (15 §15.7). Eviction changes no view or judgement (05 §5.2).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{Content, View};

const STATES: usize = 256;
const ENTRIES: usize = 32_768;

type Stored<K> = (usize, Arc<View<K>>);

#[derive(Clone, Debug)]
pub(super) struct Memo<K> {
    values: BTreeMap<Arc<BTreeSet<usize>>, Stored<K>>,
    weight: usize,
    states: usize,
    entries: usize,
}

impl<K> Default for Memo<K> {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
            weight: 0,
            states: STATES,
            entries: ENTRIES,
        }
    }
}

impl<K> Memo<K> {
    pub(super) fn get(&self, state: &BTreeSet<usize>) -> Option<&Arc<View<K>>> {
        self.values.get(state).map(|(_, view)| view)
    }

    fn clear(&mut self) {
        self.values.clear();
        self.weight = 0;
    }

    // Release a full cache before computing its successor, especially the
    // single oversized current view. External Arc owners remain valid.
    pub(super) fn prepare(&mut self) {
        if self.values.len() >= self.states || self.weight >= self.entries {
            self.clear();
        }
    }

    pub(super) fn insert(&mut self, state: BTreeSet<usize>, view: Arc<View<K>>) {
        if let Some((cost, _)) = self.values.remove(&state) {
            self.weight = self.weight.saturating_sub(cost);
        }
        let cost = state.len().saturating_add(weight(&view));
        if self.values.len() >= self.states || self.weight.saturating_add(cost) > self.entries {
            self.clear();
        }
        // Keep even one oversized view: repeated reads of the current
        // catalogue must not recompute it. The next miss releases it first.
        self.weight = self.weight.saturating_add(cost);
        self.values.insert(Arc::new(state), (cost, view));
    }
}

fn total(items: impl Iterator<Item = usize>) -> usize {
    items.fold(0, usize::saturating_add)
}

fn content<K>(value: &Content<K>) -> usize {
    match value {
        Content::File(_) => 1,
        Content::Package(files) => files.len().saturating_add(1),
    }
}

// A logical-entry budget, not an allocator byte estimate. Include sparse
// and deleted-slot metadata, package children and rename expansion: an
// empty visible tree need not be a small cached view.
fn weight<K>(view: &View<K>) -> usize {
    total(
        [
            view.tree.len(),
            view.shown.len(),
            view.unsettled.len(),
            total(
                view.shown
                    .values()
                    .flatten()
                    .map(|s| content(&s.version.content).saturating_add(1)),
            ),
            view.writers.len(),
            total(view.writers.values().map(BTreeSet::len)),
            view.heads.len(),
            total(view.heads.values().map(Vec::len)),
            view.eff.writes.len(),
            total(view.eff.writes.values().map(|writes| {
                writes.len().saturating_add(total(
                    writes
                        .values()
                        .filter_map(Option::as_ref)
                        .map(|(v, _)| content(&v.content)),
                ))
            })),
            view.eff.via.len(),
            total(view.eff.via.values().map(|(events, _)| events.len())),
            view.eff.fslot.len(),
            view.eff.yielded.len(),
        ]
        .into_iter(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::{Effective, Engine, Meta, Outcome, Tree};
    use crate::{names::Stamp, roster::PersonId};
    use hegel::{TestCase, generators as gs};

    fn empty(tree: Tree<u8>) -> View<u8> {
        View {
            tree,
            shown: BTreeMap::new(),
            unsettled: Vec::new(),
            writers: BTreeMap::new(),
            heads: BTreeMap::new(),
            eff: Effective::default(),
        }
    }

    #[hegel::test(test_cases = 1000)]
    fn eviction_bounds_retention_and_preserves_owned_snapshots(tc: TestCase) {
        let mut cache = Memo {
            states: 3,
            entries: 20,
            ..Memo::default()
        };
        let mut snapshots = Vec::new();
        for n in 0..tc.draw(gs::integers::<u8>().min_value(1).max_value(30)) {
            let count = tc.draw(gs::integers::<u8>().max_value(30));
            let tree: Tree<u8> = (0..count).map(|i| (format!("file-{i}"), n)).collect();
            let state = BTreeSet::from([usize::from(n)]);
            let view = Arc::new(empty(tree.clone()));
            cache.prepare();
            cache.insert(state.clone(), Arc::clone(&view));
            assert!(cache.values.len() <= cache.states);
            assert!(cache.weight <= cache.entries || cache.values.len() == 1);
            assert_eq!(
                cache.weight,
                total(cache.values.values().map(|(cost, _)| *cost))
            );
            assert!(cache.get(&state).is_some_and(|hit| Arc::ptr_eq(hit, &view)));
            snapshots.push((view, tree));
        }
        cache.clear();
        for (snapshot, expected) in snapshots {
            assert_eq!(snapshot.tree, expected);
        }
    }

    #[hegel::test(test_cases = 1000)]
    fn eviction_never_changes_judgements_or_public_views(tc: TestCase) {
        let mut limited = Engine::new();
        limited.memo.states = 2;
        limited.memo.entries = 10;
        let mut reference = Engine::new();
        let mut meta = Meta {
            id: "g".into(),
            parents: vec![],
            device: "A".into(),
            who: "Alice".into(),
            person: PersonId("Alice".into()),
            when: Stamp {
                utc: 1_791_849_600,
                offset: 0,
            },
            resolves: vec![],
            renames: vec![],
        };
        assert_eq!(
            limited.genesis(&meta, &Tree::<u8>::new()),
            Outcome::Contribution
        );
        assert_eq!(
            reference.genesis(&meta, &Tree::<u8>::new()),
            Outcome::Contribution
        );
        let mut ids = vec![meta.id.clone()];
        for n in 1..=tc.draw(gs::integers::<u8>().min_value(1).max_value(16)) {
            let parent = usize::from(tc.draw(gs::integers::<u8>())) % ids.len();
            meta.id = format!("c{n}");
            meta.parents = vec![ids[parent].clone()];
            meta.when.utc += 1;
            let count = tc.draw(gs::integers::<u8>().max_value(8));
            let tree = (0..count)
                .map(|i| (format!("team/file-{i}"), tc.draw(gs::integers::<u8>())))
                .collect();
            reference.memo.clear();
            assert_eq!(
                limited.record(&meta, &tree, &|_, _| true),
                reference.record(&meta, &tree, &|_, _| true)
            );
            ids.push(meta.id.clone());
            let heads = [meta.id.clone(), ids[parent].clone()];
            reference.memo.clear();
            let actual = limited.catalogue(&heads);
            let expected = reference.catalogue(&heads);
            assert_eq!(actual.tree, expected.tree);
            assert_eq!(actual.shown, expected.shown);
            assert_eq!(actual.writers, expected.writers);
            assert_eq!(actual.unsettled, expected.unsettled);
            assert_eq!(limited.open(&heads), reference.open(&heads));
        }
    }

    #[hegel::test(test_cases = 1000)]
    fn borrowing_then_owning_units_preserves_their_full_values(tc: TestCase) {
        use crate::catalogue::{own_version, units, units_by};
        const PATHS: [&str; 12] = [
            "a",
            "A",
            "R.pages",
            "r.PAGES",
            "R.pages/a",
            "R.pages/A",
            "Straße",
            "STRASSE",
            "é",
            "e\u{301}",
            "R.pages/nested.pages/data",
            "plain/folder/file",
        ];
        let mut tree = Tree::new();
        for _ in 0..tc.draw(gs::integers::<u8>().max_value(64)) {
            let index = tc.draw(gs::integers::<u8>().max_value(11));
            tree.insert(
                PATHS[usize::from(index)].to_owned(),
                format!("{:064x}", tc.draw(gs::integers::<u64>())),
            );
        }
        let borrowed = units_by(&tree, |key| key);
        let owned: BTreeMap<_, _> = borrowed
            .iter()
            .map(|(slot, value)| (slot.clone(), own_version(value)))
            .collect();
        assert_eq!(owned, units(&tree));
    }

    #[test]
    fn deleted_slot_metadata_counts_even_with_no_visible_files() {
        let mut view = empty(Tree::new());
        view.writers
            .insert("gone".into(), BTreeSet::from([PersonId("Alice".into())]));
        view.heads.insert("gone".into(), vec![1, 2, 3]);
        assert_eq!(weight(&view), 6);
    }
}
