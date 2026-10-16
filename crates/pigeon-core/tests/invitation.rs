//! Laws of admission by the group's invitation (09 §9.6, ADR 0075), on the
//! rosters that the simulated group of `common/roster_sim.rs` reaches.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::roster_sim::{ND, Sim, did};
use common::{flag, index, int, pick};
use hegel::TestCase;
use pigeon_core::invitation::{Admission, Arrival, admit};
use pigeon_core::roster::{DeviceId, Edit, Role, Roster, apply};

/// A roster the simulated group reaches.
fn roster(tc: &TestCase) -> Roster {
    let mut s = Sim::new();
    let steps = int(tc, 0, 10) as usize;
    s.run(steps, &mut |n| index(tc, n));
    let ids = s.ids();
    s.tree[&pick(tc, &ids)].clone()
}

/// A person's name: one of the roster's, in any case, or a new one.
fn name(tc: &TestCase, r: &Roster) -> String {
    let mut names: Vec<String> = r.persons.values().map(|p| p.name.clone()).collect();
    names.push("person 1".to_uppercase());
    names.push("Someone new".into());
    pick(tc, &names)
}

fn arrival<'a>(tc: &TestCase, device: &'a DeviceId, person: &'a str, uuid: &'a str) -> Arrival<'a> {
    Arrival {
        device,
        person,
        existing: flag(tc),
        name: "device 1",
        role: pick(tc, &[Role::Laptop, Role::Custodian]),
        annex_uuid: uuid,
        capacity: flag(tc).then_some(1 << 30),
    }
}

#[hegel::test(test_cases = 1000)]
fn every_addition_applies_and_never_makes_an_admin(tc: TestCase) {
    let r = roster(&tc);
    let device = did(ND + 1);
    let person = name(&tc, &r);
    let a = arrival(&tc, &device, &person, "uuid-new");
    if let Admission::Add(change) = admit(&r, &a) {
        let applied = apply(&r, &change);
        assert_eq!(applied.skipped, None);
        let d = &applied.roster.devices[&device];
        assert_eq!(d.role, a.role);
        let added: Vec<_> = change
            .iter()
            .filter_map(|e| match e {
                Edit::AddPerson(id, p) => Some((id, p)),
                _ => None,
            })
            .collect();
        match added.as_slice() {
            [] => assert!(a.existing && r.persons.contains_key(&d.person)),
            [(id, p)] => {
                assert!(!p.admin && **id == d.person && !r.persons.contains_key(*id));
                assert_eq!(p.name, person.trim());
            }
            _ => panic!("two persons added"),
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn a_name_of_the_roster_is_taken_only_with_consent(tc: TestCase) {
    let r = roster(&tc);
    let device = did(ND + 1);
    let person = name(&tc, &r);
    let a = arrival(&tc, &device, &person, "uuid-new");
    match (r.person_named(&person), admit(&r, &a)) {
        (Some(p), Admission::PersonExists(q)) => assert!(!a.existing && p == q),
        (Some(p), Admission::Add(change)) => {
            assert!(a.existing);
            assert!(!change.iter().any(|e| matches!(e, Edit::AddPerson(..))));
            assert_eq!(apply(&r, &change).roster.devices[&device].person, p);
        }
        (None, Admission::Add(_)) => {}
        (known, other) => panic!("{known:?} admitted as {other:?}"),
    }
}

#[hegel::test(test_cases = 1000)]
fn a_device_of_the_roster_returns_only_as_itself(tc: TestCase) {
    let r = roster(&tc);
    let Some((id, d)) = r.devices.iter().next().map(|(i, d)| (i.clone(), d.clone())) else {
        return;
    };
    let uuid = d.annex_uuid.clone().unwrap_or_default();
    let person = name(&tc, &r);
    let same = admit(
        &r,
        &Arrival {
            role: d.role,
            ..arrival(&tc, &id, &person, &uuid)
        },
    );
    if d.role == Role::Recovery {
        assert_eq!(same, Admission::Refused);
    } else {
        assert_eq!(same, Admission::Again);
    }
    let other = admit(&r, &arrival(&tc, &id, &person, "uuid-other"));
    assert_eq!(other, Admission::Refused);
}

#[hegel::test(test_cases = 1000)]
fn a_removed_device_is_never_admitted(tc: TestCase) {
    let r = roster(&tc);
    let person = name(&tc, &r);
    for id in r.removed.keys() {
        assert_eq!(
            admit(&r, &arrival(&tc, id, &person, "uuid-new")),
            Admission::Refused
        );
    }
}

#[hegel::test(test_cases = 1000)]
fn a_new_persons_id_and_a_new_devices_name_are_fresh(tc: TestCase) {
    let mut r = roster(&tc);
    let names = [
        "Ana", "ana", "Ana2", "Zoé", "", "Ünal", "person", "device 1",
    ];
    for _ in 0..int(&tc, 0, 6) {
        let name = pick(&tc, &names);
        let id = r.fresh_person_id(name);
        assert!(
            !id.0.is_empty()
                && id
                    .0
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()),
            "{id:?}"
        );
        assert!(!r.persons.contains_key(&id), "{id:?}");
        r.persons.insert(
            id.clone(),
            pigeon_core::roster::Person {
                name: name.to_owned(),
                admin: false,
            },
        );
        assert_eq!(r.person_named(&id.0), Some(id));
        let device = r.fresh_device_name(name);
        assert!(device.starts_with(name));
        assert!(!r.devices.values().any(|d| d.name == device), "{device}");
    }
}
