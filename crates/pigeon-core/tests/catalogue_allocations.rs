//! An unchanged catalogue contribution must not clone every file's entry
//! just to compare its units (05 §5.1, 15 §15.7).
use std::sync::atomic::{AtomicUsize, Ordering};

use pigeon_core::catalogue::{Engine, Judgement, Meta, Outcome, Tree};
use pigeon_core::{names::Stamp, roster::PersonId};

static COPIES: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Entry(usize);

impl Clone for Entry {
    fn clone(&self) -> Self {
        COPIES.fetch_add(1, Ordering::Relaxed);
        Self(self.0)
    }
}

#[test]
fn a_clean_contribution_compares_cached_units_without_cloning_entries() {
    let tree: Tree<Entry> = (0..256)
        .map(|n| (format!("team/file-{n}"), Entry(n)))
        .collect();
    let mut engine = Engine::new();
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
    assert_eq!(engine.genesis(&meta, &tree), Outcome::Contribution);
    let snapshot = engine.catalogue(std::slice::from_ref(&meta.id));
    assert_eq!(snapshot.tree, tree);
    meta.parents = vec![meta.id.clone()];
    meta.id = "clean".into();
    meta.when.utc += 1;
    COPIES.store(0, Ordering::Relaxed);
    assert_eq!(
        engine.admit(&meta, &tree, Judgement::Accepted),
        Outcome::Clean
    );
    assert_eq!(COPIES.load(Ordering::Relaxed), 0);
    assert_eq!(snapshot.tree, tree);
}
