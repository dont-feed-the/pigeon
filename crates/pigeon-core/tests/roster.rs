//! Laws of the roster fold (04 §4.8, ADR 0051; pigeon-core `roster`), on
//! histories that the simulated group of `common/roster_sim.rs` makes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::roster_sim::{NP, Sim, device, did, genesis, pid};
use common::{index, int};
use hegel::TestCase;
use pigeon_core::roster::{Edit, Field, Value, apply, may_sign};

fn sim(tc: &TestCase, replicas_merge: bool) -> Sim {
    let mut s = Sim::new();
    let steps = int(tc, 0, 14) as usize;
    let mut choose = |n: usize| index(tc, n);
    if replicas_merge {
        s.run(steps, &mut choose);
    } else {
        for _ in 0..steps {
            let t = int(tc, 0, 2);
            s.commit(0, t, &mut choose);
        }
    }
    s
}

#[hegel::test(test_cases = 1000)]
fn every_tree_is_the_fold_of_its_history(tc: TestCase) {
    let s = sim(&tc, true);
    for c in s.ids() {
        assert_eq!(s.history.fold(&c).roster, s.tree[&c], "commit {c}");
    }
}

#[hegel::test(test_cases = 1000)]
fn the_fold_depends_only_on_the_set_of_changes(tc: TestCase) {
    let s = sim(&tc, true);
    let ids = s.ids();
    for &a in &ids {
        for &b in &ids {
            if s.changes_of(a) == s.changes_of(b) {
                assert_eq!(
                    s.history.fold(&a).roster,
                    s.history.fold(&b).roster,
                    "{a} and {b}"
                );
            }
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn every_fold_is_valid_and_removals_win(tc: TestCase) {
    let s = sim(&tc, true);
    for c in s.ids() {
        let r = s.history.fold(&c).roster;
        assert_eq!(r.validate(), Ok(()));
        for x in s.changes_of(c) {
            for e in &s.history.commits[&x].change {
                if let Edit::RemoveDevice(d) = e {
                    assert!(!r.devices.contains_key(d), "{d:?} came back in {c}");
                    assert!(r.removed.contains_key(d), "{d:?} not kept out in {c}");
                }
            }
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn skips_need_concurrency(tc: TestCase) {
    let s = sim(&tc, true);
    for c in s.ids() {
        let changes = s.changes_of(c);
        for (x, _) in s.history.fold(&c).skipped {
            assert!(
                changes.iter().any(|&y| !s.is_anc(x, y) && !s.is_anc(y, x)),
                "{x} skipped in {c}"
            );
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn a_linear_history_skips_nothing(tc: TestCase) {
    let s = sim(&tc, false);
    let head = s.head[0];
    let folded = s.history.fold(&head);
    assert!(folded.skipped.is_empty());
    let mut r = s.history.roster.clone();
    for x in s.history.canonical_order(&head) {
        r = apply(&r, &s.history.commits[&x].change).roster;
    }
    assert_eq!(r, folded.roster);
}

#[hegel::test(test_cases = 1000)]
fn the_canonical_order_is_topological(tc: TestCase) {
    let s = sim(&tc, true);
    for c in s.ids() {
        let order = s.history.canonical_order(&c);
        for (i, x) in order.iter().enumerate() {
            for y in &order[i + 1..] {
                assert!(
                    !s.is_anc(*y, *x) || x == y,
                    "{y} is an ancestor of {x} but comes after it"
                );
            }
        }
    }
}

#[test]
fn a_member_who_is_no_admin_signs_only_their_devices() {
    let s = Sim::new();
    let r = &s.tree[&1];
    let me = did(NP);
    let promote = Edit::Set {
        field: Field::PersonAdmin(pid(NP)),
        from: Value::Flag(false),
        to: Value::Flag(true),
    };
    assert!(!may_sign(r, &me, &[promote]));
    assert!(!may_sign(r, &me, &[Edit::RemoveDevice(did(1))]));
    assert!(!may_sign(r, &me, &[Edit::RemovePerson(pid(1))]));
    assert!(may_sign(r, &me, &[Edit::RemoveDevice(me.clone())]));
    assert!(
        !may_sign(r, &did(1), &[Edit::RemovePerson(pid(1))]),
        "nobody removes their own person"
    );
}

/// Seed 69 of the trace check: a rotation that adds device 5 is skipped, the
/// removal of device 5 still applies, and a later addition of it must fail
/// (Roster.tla's "rem" always records the device as gone).
#[test]
fn removing_an_absent_device_keeps_it_out() {
    let five = did(5);
    let removed = apply(&genesis(), &[Edit::RemoveDevice(five.clone())]);
    assert_eq!(removed.skipped, None);
    assert!(removed.roster.removed.contains_key(&five));
    let added = apply(
        &removed.roster,
        &[Edit::AddDevice(five.clone(), device(5, 1))],
    );
    assert!(added.skipped.is_some());
    assert!(!added.roster.devices.contains_key(&five));
}
