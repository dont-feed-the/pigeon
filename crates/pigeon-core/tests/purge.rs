//! Purge laws (08 §8.6), 1000 generated cases each. Reference births and
//! sealer histories travel independently; a current/copy/restore reference
//! cannot be released by an older decision or incomplete seal evidence.
#![allow(clippy::unwrap_used, reason = "law fixtures")]
use hegel::{TestCase, generators as gs};
use pigeon_core::purge::{Decision, Epoch, Reference, Seal, View, can_seal, valid_end};
use std::collections::{BTreeMap, BTreeSet};

struct World {
    key: u64,
    main: u64,
    references: BTreeSet<Reference<u64, u64>>,
    current: BTreeSet<u64>,
    decisions: BTreeMap<u64, Decision<u64, u64>>,
    custodians: BTreeSet<u8>,
    seals: BTreeMap<(u64, u8), Seal<u64, u64>>,
    epochs: BTreeMap<u64, Epoch<u64, u8>>,
}

impl World {
    fn new(tc: &TestCase) -> Self {
        let key = tc.draw(gs::integers::<u64>());
        let path = format!("Photos-{}/old.jpg", tc.draw(gs::integers::<u32>()));
        let decision = Decision {
            base: 1,
            paths: [path.clone()].into(),
            keys: BTreeSet::new(),
            proposals: BTreeSet::new(),
        };
        Self {
            key,
            main: 3,
            references: [Reference {
                birth: 1,
                path,
                key,
            }]
            .into(),
            current: BTreeSet::new(),
            decisions: [(100, decision)].into(),
            custodians: [0, 1].into(),
            epochs: BTreeMap::new(),
            seals: [
                (
                    (100, 0),
                    Seal {
                        main: 2,
                        held: BTreeSet::new(),
                    },
                ),
                (
                    (100, 1),
                    Seal {
                        main: 2,
                        held: BTreeSet::new(),
                    },
                ),
            ]
            .into(),
        }
    }

    fn view(&self) -> View<'_, u64, u64, u8> {
        View {
            main: &self.main,
            references: &self.references,
            current: &self.current,
            decisions: &self.decisions,
            custodians: &self.custodians,
            seals: &self.seals,
            epochs: &self.epochs,
        }
    }

    fn released(&self) -> BTreeSet<u64> {
        self.view().released(|tip, ancestor| tip >= ancestor)
    }
}

#[hegel::test(test_cases = 1000)]
fn every_active_custodian_must_seal_and_its_history_must_be_merged(tc: TestCase) {
    let mut world = World::new(&tc);
    assert_eq!(world.released(), [world.key].into());
    let signer = tc.draw(gs::integers::<u8>().max_value(1));
    let seal = world.seals.remove(&(100, signer)).unwrap();
    assert!(world.released().is_empty());
    world.seals.insert(
        (100, signer),
        Seal {
            main: world.main + 1,
            ..seal
        },
    );
    assert!(world.released().is_empty());
    world.custodians.clear();
    assert!(world.released().is_empty());
}

#[hegel::test(test_cases = 1000)]
fn a_current_reference_is_never_released(tc: TestCase) {
    let mut world = World::new(&tc);
    world.current.insert(world.key);
    assert!(world.released().is_empty());
}

#[hegel::test(test_cases = 1000)]
fn shared_and_later_references_keep_the_same_key(tc: TestCase) {
    let mut world = World::new(&tc);
    let old = world.references.first().unwrap().clone();
    let later = tc.draw(gs::booleans());
    world.references.insert(Reference {
        birth: if later { 4 } else { 1 },
        path: if later {
            old.path
        } else {
            "Elsewhere/copy.jpg".into()
        },
        key: world.key,
    });
    assert!(world.released().is_empty());
}

#[hegel::test(test_cases = 1000)]
fn a_timely_outside_proposal_is_held_even_before_others_receive_it(tc: TestCase) {
    let mut sealer = World::new(&tc);
    let mut receiver = World::new(&tc);
    // Both views represent the same old reference; only one has received
    // the new proposal. The receiver can learn its seal before its content.
    receiver.key = sealer.key;
    receiver.references = sealer.references.clone();
    receiver.decisions = sealer.decisions.clone();
    sealer.references.insert(Reference {
        birth: 4,
        path: "Elsewhere/copy.jpg".into(),
        key: sealer.key,
    });
    let held = sealer
        .view()
        .held(&sealer.decisions[&100], |tip, ancestor| tip >= ancestor);
    assert_eq!(held, [sealer.key].into());
    receiver.seals.get_mut(&(100, 0)).unwrap().held = held;
    assert!(receiver.released().is_empty());
}

#[hegel::test(test_cases = 1000)]
fn cancelling_or_reintroducing_a_decision_never_reuses_its_old_seals(tc: TestCase) {
    let mut world = World::new(&tc);
    let decision = world.decisions.remove(&100).unwrap();
    assert!(world.released().is_empty());
    world.decisions.insert(101, decision);
    assert!(world.released().is_empty());
}

#[hegel::test(test_cases = 1000)]
fn a_named_proposal_and_key_filter_release_only_the_observed_scope(tc: TestCase) {
    let mut world = World::new(&tc);
    let reference = world.references.first().unwrap().clone();
    world.references.clear();
    world.references.insert(Reference {
        birth: 4,
        ..reference
    });
    assert!(world.released().is_empty());
    let decision = world.decisions.get_mut(&100).unwrap();
    decision.proposals.insert(4);
    assert_eq!(world.released(), [world.key].into());
    world
        .decisions
        .get_mut(&100)
        .unwrap()
        .keys
        .insert(world.key.wrapping_add(1));
    assert!(world.released().is_empty());
}

#[hegel::test(test_cases = 1000)]
fn clocks_cannot_shorten_retention_or_replace_elapsed_receipt_time(tc: TestCase) {
    let at = tc.draw(gs::integers::<i64>());
    let period = tc.draw(gs::integers::<u32>().min_value(1));
    let required = i128::from(at) + i128::from(period);
    if let Ok(end) = i64::try_from(required) {
        assert!(valid_end(at, period.into(), end));
        assert!(!valid_end(at, period.into(), end.saturating_sub(1)));
        assert!(!can_seal(
            i64::MAX,
            end,
            u64::from(period) - 1,
            period.into()
        ));
        assert!(can_seal(end, end, period.into(), period.into()));
    } else {
        assert!(!valid_end(at, period.into(), i64::MAX));
    }
}

#[hegel::test(test_cases = 1000)]
fn a_proven_closed_epoch_survives_later_membership_but_never_covers_a_new_birth(tc: TestCase) {
    let mut world = World::new(&tc);
    world.epochs.insert(
        100,
        Epoch {
            main: 3,
            custodians: world.custodians.clone(),
        },
    );
    let extra = tc.draw(gs::integers::<u8>().min_value(2));
    world.custodians.insert(extra);
    assert_eq!(world.released(), [world.key].into());
    world.custodians.clear();
    assert_eq!(world.released(), [world.key].into());
    let old = world.references.first().unwrap().clone();
    world.references.insert(Reference { birth: 4, ..old });
    world.main = 4;
    assert!(world.released().is_empty());
}

#[hegel::test(test_cases = 1000)]
fn a_completion_epoch_cannot_precede_its_seals_or_be_empty_or_unseen(tc: TestCase) {
    let mut world = World::new(&tc);
    for epoch in [
        Epoch {
            main: 1,
            custodians: world.custodians.clone(),
        },
        Epoch {
            main: 3,
            custodians: BTreeSet::new(),
        },
        Epoch {
            main: 4,
            custodians: world.custodians.clone(),
        },
    ] {
        world.epochs.insert(100, epoch);
        assert!(world.released().is_empty());
    }
    world.epochs.insert(
        100,
        Epoch {
            main: 3,
            custodians: world.custodians.clone(),
        },
    );
    let signer = tc.draw(gs::integers::<u8>().max_value(1));
    world
        .seals
        .get_mut(&(100, signer))
        .unwrap()
        .held
        .insert(world.key);
    assert!(world.released().is_empty());
}
