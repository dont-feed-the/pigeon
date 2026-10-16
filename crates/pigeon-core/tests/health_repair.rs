//! Repair-epoch projection laws (12 §12.2): effective computer removals
//! only, stable metadata, skipped edits and canonical fork ordering.
//! Every law runs 1000 cases against the accepted roster fold.
mod common;
use common::roster_sim::{NP, did, genesis};
use hegel::{TestCase, generators as gs};
use pigeon_core::{
    health::repair_epochs,
    roster::{Commit, Edit, History, Role},
};
use std::collections::{BTreeMap, BTreeSet};

fn history(tc: &TestCase) -> (History<u32>, u32) {
    let mut roster = genesis();
    for (id, device) in &mut roster.devices {
        if *id != did(1) && tc.draw(gs::booleans()) {
            device.role = Role::Recovery;
            device.annex_uuid = None;
        }
    }
    let mut history = History {
        genesis: 0,
        roster,
        commits: BTreeMap::new(),
    };
    let tip = tc.draw(gs::integers::<u32>().min_value(1).max_value(20));
    for c in 1..=tip {
        let device = did(tc.draw(gs::integers::<usize>().min_value(2).max_value(NP + 1)));
        history.commits.insert(
            c,
            Commit {
                parents: vec![c - 1],
                time: i64::from(c),
                signer: did(1),
                change: vec![Edit::RemoveDevice(device)],
            },
        );
    }
    (history, tip)
}

#[hegel::test(test_cases = 1000)]
fn every_effectively_removed_computer_has_one_epoch_and_paper_has_none(tc: TestCase) {
    let (history, tip) = history(&tc);
    let final_roster = history.fold(&tip).roster;
    let expected: BTreeSet<_> = history
        .roster
        .devices
        .iter()
        .filter(|(id, d)| d.role != Role::Recovery && final_roster.removed.contains_key(*id))
        .map(|(id, _)| id.clone())
        .collect();
    let epochs = repair_epochs(&history, &tip);
    assert_eq!(
        epochs
            .iter()
            .map(|e| e.device.clone())
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(epochs.len(), expected.len());
    for epoch in epochs {
        assert_eq!(
            Some(&epoch.name),
            history.roster.devices.get(&epoch.device).map(|d| &d.name)
        );
        assert_eq!(epoch.at, i64::from(epoch.commit));
    }
}

#[hegel::test(test_cases = 1000)]
fn later_file_commits_do_not_change_repair_history(tc: TestCase) {
    let (mut history, tip) = history(&tc);
    let before = repair_epochs(&history, &tip);
    history.commits.insert(
        tip + 1,
        Commit {
            parents: vec![tip],
            time: i64::from(tip + 1),
            signer: did(1),
            change: Vec::new(),
        },
    );
    assert_eq!(repair_epochs(&history, &(tip + 1)), before);
}

#[hegel::test(test_cases = 1000)]
fn skipped_and_unknown_removals_cannot_create_repair_work(tc: TestCase) {
    let (mut history, tip) = history(&tc);
    let before = repair_epochs(&history, &tip);
    let removed = history
        .fold(&tip)
        .roster
        .removed
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| did(NP + 1));
    history.commits.insert(
        tip + 1,
        Commit {
            parents: vec![tip],
            time: i64::from(tip + 1),
            signer: did(1),
            change: vec![Edit::RemoveDevice(removed), Edit::RemoveDevice(did(NP + 2))],
        },
    );
    assert_eq!(repair_epochs(&history, &(tip + 1)), before);
}

#[hegel::test(test_cases = 1000)]
fn concurrent_duplicate_removals_have_one_canonical_epoch(tc: TestCase) {
    let mut history = History {
        genesis: 0,
        roster: genesis(),
        commits: BTreeMap::new(),
    };
    let device = did(tc.draw(gs::integers::<usize>().min_value(2).max_value(NP)));
    let time = tc.draw(gs::integers::<i64>().min_value(0).max_value(1_000_000));
    for c in [1, 2] {
        history.commits.insert(
            c,
            Commit {
                parents: vec![0],
                time,
                signer: did(1),
                change: vec![Edit::RemoveDevice(device.clone())],
            },
        );
    }
    history.commits.insert(
        3,
        Commit {
            parents: vec![1, 2],
            time: time + 1,
            signer: did(1),
            change: Vec::new(),
        },
    );
    let expected = repair_epochs(&history, &3);
    if let Some(record) = history.commits.get_mut(&3) {
        record.parents.reverse();
    }
    assert_eq!(repair_epochs(&history, &3), expected);
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].device, device);
}
