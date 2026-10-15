//! Laws of status reports (06 §6.3; pigeon-core `report`): which report of
//! each device counts, and the latest of each, met in any order.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;

use common::{flag, index, int};
use hegel::TestCase;
use pigeon_core::report::{Ignored, Latest, Offer, SKEW, Seen, judge};
use pigeon_core::roster::{Device, DeviceId, Person, PersonId, Role, Roster};
use pigeon_core::time::Time;

fn dev(i: i64) -> DeviceId {
    DeviceId(format!("d{i}"))
}

/// Devices d0 to d2; d3 is not in the roster.
fn roster() -> Roster {
    let device = |i: i64| Device {
        person: PersonId("p".into()),
        name: format!("device {i}"),
        role: if i == 0 {
            Role::Custodian
        } else {
            Role::Laptop
        },
        annex_uuid: Some(format!("uuid-{i}")),
        maxsize: None,
    };
    Roster {
        rules: 1,
        group: "g".into(),
        k: 2,
        retention_days: 30,
        relay: None,
        persons: [(
            PersonId("p".into()),
            Person {
                name: "P".into(),
                admin: true,
            },
        )]
        .into_iter()
        .collect(),
        devices: (0..3).map(|i| (dev(i), device(i))).collect(),
        removed: BTreeMap::new(),
        folders: Vec::new(),
    }
}

fn seen(tc: &TestCase) -> Seen {
    Seen {
        at: Time(int(tc, 0, 5)),
        commit: format!("c{}", int(tc, 0, 3)),
    }
}

fn arrivals(tc: &TestCase) -> Vec<(DeviceId, Seen)> {
    (0..int(tc, 0, 8))
        .map(|_| (dev(int(tc, 0, 3)), seen(tc)))
        .collect()
}

fn latest(xs: &[(DeviceId, Seen)]) -> Latest {
    let mut l = Latest::default();
    for (d, s) in xs {
        l.offer(d.clone(), s.clone());
    }
    l
}

fn shuffled<T: Clone>(tc: &TestCase, xs: &[T]) -> Vec<T> {
    let mut v = xs.to_vec();
    for i in (1..v.len()).rev() {
        let j = index(tc, i + 1);
        v.swap(i, j);
    }
    v
}

#[hegel::test(test_cases = 1000)]
fn latest_is_a_semilattice(tc: TestCase) {
    let (a, b, c) = (
        latest(&arrivals(&tc)),
        latest(&arrivals(&tc)),
        latest(&arrivals(&tc)),
    );
    assert_eq!(a.join(&b), b.join(&a));
    assert_eq!(a.join(&b.join(&c)), a.join(&b).join(&c));
    assert_eq!(a.join(&a), a);
    assert_eq!(a.join(&Latest::default()), a);
}

#[hegel::test(test_cases = 1000)]
fn the_order_of_arrival_does_not_matter(tc: TestCase) {
    let xs = arrivals(&tc);
    assert_eq!(latest(&xs), latest(&shuffled(&tc, &xs)));
    let cut = index(&tc, xs.len() + 1);
    assert_eq!(latest(&xs[..cut]).join(&latest(&xs[cut..])), latest(&xs));
}

#[hegel::test(test_cases = 1000)]
fn an_older_report_never_replaces_a_newer(tc: TestCase) {
    let xs = arrivals(&tc);
    let l = latest(&xs);
    for (d, kept) in &l.0 {
        let best = xs.iter().filter(|(x, _)| x == d).map(|(_, s)| s).max();
        assert_eq!(Some(kept), best);
    }
    assert!(xs.iter().all(|(d, _)| l.get(d).is_some()));
    let mut more = l.clone();
    let (d, s) = (dev(int(&tc, 0, 3)), seen(&tc));
    let took = more.offer(d.clone(), s.clone());
    assert_eq!(took, l.get(&d).is_none_or(|k| *k < s));
    assert!(more.get(&d).is_some_and(|k| *k >= s));
}

#[hegel::test(test_cases = 1000)]
fn only_a_report_signed_by_its_roster_device_counts(tc: TestCase) {
    let r = roster();
    let device = dev(int(&tc, 0, 3));
    let other = dev(int(&tc, 0, 3));
    let signer = match int(&tc, 0, 2) {
        0 => None,
        1 => Some(&other),
        _ => Some(&device),
    };
    let offer = Offer {
        signer,
        device: &device,
        known: flag(&tc),
        seen: seen(&tc),
    };
    let last = flag(&tc).then(|| seen(&tc));
    let now = Time(int(&tc, -SKEW.0 - 2, 5));
    let verdict = judge(&r, &offer, last.as_ref(), now);
    let counts = signer == Some(&device)
        && r.devices.contains_key(&device)
        && offer.known
        && offer.seen.at.0 <= now.0 + SKEW.0
        && last.as_ref().is_none_or(|l| *l < offer.seen);
    assert_eq!(verdict.is_ok(), counts, "{verdict:?}");
    if signer.is_none() {
        assert_eq!(verdict, Err(Ignored::Unsigned));
    }
    if verdict.is_ok() {
        let mut l = Latest::default();
        if let Some(s) = &last {
            l.offer(device.clone(), s.clone());
        }
        assert!(l.offer(device.clone(), offer.seen.clone()));
    }
}

#[hegel::test(test_cases = 1000)]
fn relaying_what_counted_changes_nothing(tc: TestCase) {
    let r = roster();
    let a = latest(&arrivals(&tc));
    let b = latest(&arrivals(&tc)).join(&a);
    let mut again = b.clone();
    for (d, s) in &a.0 {
        assert!(!again.offer(d.clone(), s.clone()));
        let offer = Offer {
            signer: Some(d),
            device: d,
            known: true,
            seen: s.clone(),
        };
        let verdict = judge(&r, &offer, b.get(d), Time(5));
        assert!(verdict.is_err());
    }
    assert_eq!(again, b);
}
