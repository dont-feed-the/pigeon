//! Refounding laws (09 §9.9, ADR 0047): valid sole-person authority,
//! preserved folder boundaries/settings, admitted survivors, idempotence.
#![allow(clippy::unwrap_used, reason = "tests")]

mod common;
use common::roster_sim::{NP, did, genesis, pid};
use hegel::{TestCase, generators as gs};
use pigeon_core::{
    refound::roster,
    roster::{DeviceId, Folder, Role, Roster, Rule},
};

fn input(tc: &TestCase) -> (Roster, DeviceId) {
    let mut old = genesis();
    old.k = tc.draw(gs::integers::<u32>().min_value(1).max_value(9));
    old.retention_days = tc.draw(gs::integers::<u32>().min_value(1).max_value(365));
    old.relay = tc
        .draw(gs::booleans())
        .then(|| "https://relay.example".into());
    for i in 0..tc.draw(gs::integers::<usize>().max_value(12)) {
        let owner = tc.draw(gs::integers::<usize>().min_value(1).max_value(NP));
        let rule = if tc.draw(gs::booleans()) {
            Rule::Owned(pid(owner))
        } else {
            Rule::AddOnly(
                (1..=NP)
                    .filter(|_| tc.draw(gs::booleans()))
                    .map(pid)
                    .collect(),
            )
        };
        old.folders.push(Folder {
            path: format!("folder-{i}"),
            rule,
        });
    }
    let me = did(tc.draw(gs::integers::<usize>().min_value(1).max_value(NP)));
    if tc.draw(gs::booleans()) {
        let device = old.devices.get_mut(&me).unwrap();
        device.role = Role::Custodian;
        device.maxsize = Some(tc.draw(gs::integers::<u64>().min_value(1).max_value(1_000_000)));
    }
    assert!(old.validate().is_ok());
    (old, me)
}

/// The refounded group's invitation.
const NEW: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

#[hegel::test(test_cases = 1000)]
fn sole_person_authority_is_valid(tc: TestCase) {
    let (old, me) = input(&tc);
    let new = roster(&old, &me, NEW).unwrap();
    assert!(new.validate().is_ok());
    assert_eq!(new.devices.len(), 1);
    assert_eq!(new.devices[&me], old.devices[&me]);
    assert_eq!(new.persons.len(), 1);
    assert!(new.persons[&old.devices[&me].person].admin);
    assert!(new.removed.is_empty());
}

#[hegel::test(test_cases = 1000)]
fn folder_boundaries_and_group_settings_survive(tc: TestCase) {
    let (old, me) = input(&tc);
    let new = roster(&old, &me, NEW).unwrap();
    assert_eq!(
        (
            &new.group,
            new.k,
            new.retention_days,
            &new.relay,
            &new.telemetry_device,
            new.rules
        ),
        (
            &old.group,
            old.k,
            old.retention_days,
            &old.relay,
            &old.telemetry_device,
            old.rules
        )
    );
    assert_eq!(new.folders.len(), old.folders.len());
    for (a, b) in old.folders.iter().zip(&new.folders) {
        assert_eq!(a.path, b.path);
        assert_eq!(
            std::mem::discriminant(&a.rule),
            std::mem::discriminant(&b.rule)
        );
        match &b.rule {
            Rule::Owned(owner) => assert_eq!(owner, &old.devices[&me].person),
            Rule::AddOnly(stewards) => {
                assert!(stewards.iter().all(|p| p == &old.devices[&me].person))
            }
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn only_a_surviving_computer_can_refound(tc: TestCase) {
    let (mut old, me) = input(&tc);
    assert!(roster(&old, &DeviceId("unknown".into()), NEW).is_none());
    let device = old.devices.get_mut(&me).unwrap();
    device.role = Role::Recovery;
    device.annex_uuid = None;
    assert!(roster(&old, &me, NEW).is_none());
}

#[hegel::test(test_cases = 1000)]
fn refounding_is_idempotent(tc: TestCase) {
    let (old, me) = input(&tc);
    let new = roster(&old, &me, NEW).unwrap();
    assert_eq!(new.invitation, NEW);
    assert_eq!(roster(&new, &me, NEW), Some(new));
}
