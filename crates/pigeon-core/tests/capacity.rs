//! Laws of capacity (08 §8.9; pigeon-core `capacity`): admission, the
//! reserve and the need in parts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::{flag, int, pick};
use hegel::TestCase;
use pigeon_core::capacity::{
    Admits, GB, Part, Plan, admit, below_one_copy, capacity, key_size, need, no_spare, reserve,
    room,
};
use pigeon_core::filter::Role;
use pigeon_core::queue::{Work, compare};
use pigeon_core::time::Time;

fn item(tc: &TestCase) -> Work {
    Work::Protect {
        key: format!("k{}", int(tc, 0, 9)),
        custody_copies: int(tc, 0, 2) as u32,
        laptop_copies: int(tc, 0, 1) as u32,
        endangered: flag(tc),
        introduced: Time(int(tc, 0, 3)),
        size: int(tc, 0, 12) as u64,
    }
}

fn items(tc: &TestCase) -> Vec<Work> {
    (0..int(tc, 0, 8)).map(|_| item(tc)).collect()
}

fn size(w: &Work) -> u64 {
    match w {
        Work::Protect { size, .. } => *size,
        _ => 0,
    }
}

fn endangered(w: &Work) -> bool {
    matches!(
        w,
        Work::Protect {
            endangered: true,
            ..
        }
    )
}

fn sorted(mut v: Vec<Work>) -> Vec<Work> {
    v.sort_by(compare);
    v
}

fn planned(p: &Plan) -> Vec<Work> {
    let mut all = p.admitted.clone();
    all.extend(p.deferred.iter().map(|d| d.work.clone()));
    sorted(all)
}

#[hegel::test(test_cases = 1000)]
fn admitted_bytes_never_exceed_the_room(tc: TestCase) {
    let (r, xs) = (int(&tc, 0, 40) as u64, items(&tc));
    let p = admit(r, &xs);
    assert!(p.admitted_bytes() <= r);
}

#[hegel::test(test_cases = 1000)]
fn an_endangered_key_is_never_deferred_for_one_that_is_not(tc: TestCase) {
    let (r, xs) = (int(&tc, 0, 40) as u64, items(&tc));
    let only: Vec<Work> = xs.iter().filter(|w| endangered(w)).cloned().collect();
    let mixed: Vec<Work> = admit(r, &xs)
        .admitted
        .into_iter()
        .filter(endangered)
        .collect();
    assert_eq!(mixed, admit(r, &only).admitted);
}

#[hegel::test(test_cases = 1000)]
fn the_plan_does_not_depend_on_arrival(tc: TestCase) {
    let (r, xs) = (int(&tc, 0, 40) as u64, items(&tc));
    let mut ys = xs.clone();
    ys.reverse();
    if ys.len() > 2 {
        ys.swap(0, 1);
    }
    assert_eq!(admit(r, &xs), admit(r, &ys));
}

#[hegel::test(test_cases = 1000)]
fn the_plan_keeps_queue_order_and_plans_each_item_once(tc: TestCase) {
    let (r, xs) = (int(&tc, 0, 40) as u64, items(&tc));
    let p = admit(r, &xs);
    assert_eq!(p.admitted, sorted(p.admitted.clone()));
    let waits: Vec<Work> = p.deferred.iter().map(|d| d.work.clone()).collect();
    assert_eq!(waits, sorted(waits.clone()));
    assert_eq!(planned(&p), sorted(xs));
}

#[hegel::test(test_cases = 1000)]
fn a_deferred_item_says_what_it_misses(tc: TestCase) {
    let (r, xs) = (int(&tc, 0, 40) as u64, items(&tc));
    for d in admit(r, &xs).deferred {
        assert!(d.missing_bytes > 0);
        assert!(d.missing_bytes <= size(&d.work) + d.held_bytes);
        if endangered(&d.work) {
            assert_eq!(d.held_bytes, 0);
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn missing_bytes_more_room_admits_everything(tc: TestCase) {
    let (r, xs) = (int(&tc, 0, 40) as u64, items(&tc));
    let p = admit(r, &xs);
    let m = p.missing_bytes();
    assert_eq!(m == 0, p.deferred.is_empty());
    assert!(admit(r + m, &xs).deferred.is_empty());
    if m > 0 {
        assert!(!admit(r + m - 1, &xs).deferred.is_empty());
    }
}

#[hegel::test(test_cases = 1000)]
fn a_custodian_admits_only_endangered_keys_while_it_keeps_their_room(tc: TestCase) {
    let (r, xs) = (int(&tc, 0, 40) as u64, items(&tc));
    let p = admit(r, &xs);
    let kept = p
        .deferred
        .iter()
        .any(|d| !endangered(&d.work) && d.held_bytes > 0);
    assert_eq!(p.admits() == Admits::Endangered, kept);
}

#[hegel::test(test_cases = 1000)]
fn the_reserve_is_a_floor_or_a_share(tc: TestCase) {
    let fs = int(&tc, 0, 1_000_000) as u64 * GB / 1000;
    let more = fs + int(&tc, 0, 1_000_000) as u64 * GB / 1000;
    let role = pick(&tc, &[Role::Laptop, Role::Custodian]);
    let (floor, share) = match role {
        Role::Laptop => (GB, fs / 100),
        Role::Custodian => (5 * GB, fs / 50),
    };
    assert_eq!(reserve(role, fs), floor.max(share));
    assert!(reserve(role, fs) <= reserve(role, more));
    let (avail, inflight) = (int(&tc, 0, 100) as u64, int(&tc, 0, 100) as u64);
    let res = int(&tc, 0, 100) as u64;
    assert_eq!(
        room(avail, res, inflight),
        avail.saturating_sub(res + inflight)
    );
}

#[hegel::test(test_cases = 1000)]
fn the_parts_of_the_need_sum_to_k_times_each_distinct_key(tc: TestCase) {
    let k = int(&tc, 1, 4) as u32;
    let parts = [Part::Current, Part::History, Part::Suggestions];
    let keys: Vec<(String, u64)> = (0..6)
        .map(|i| (format!("k{i}"), int(&tc, 0, 9) as u64))
        .collect();
    let vs: Vec<(String, u64, Part)> = (0..int(&tc, 0, 10))
        .map(|_| {
            let (key, size) = pick(&tc, &keys);
            (key, size, pick(&tc, &parts))
        })
        .collect();
    let of = |v: &[(String, u64, Part)]| need(k, v.iter().map(|(a, b, c)| (a.as_str(), *b, *c)));
    let n = of(&vs);
    assert_eq!(n.total(), n.parts.values().sum::<u64>());
    let distinct: std::collections::BTreeMap<&str, u64> =
        vs.iter().map(|(a, b, _)| (a.as_str(), *b)).collect();
    assert_eq!(n.total(), u64::from(k) * distinct.values().sum::<u64>());
    let mut twice = vs.clone();
    twice.extend(vs.iter().cloned());
    twice.reverse();
    assert_eq!(of(&twice), n);
    for (key, _, part) in &vs {
        let first = vs
            .iter()
            .filter(|v| &v.0 == key)
            .map(|v| v.2)
            .min()
            .unwrap();
        assert!(first <= *part);
    }
}

#[hegel::test(test_cases = 300)]
fn capacity_counts_at_most_one_copy_per_custodian(tc: TestCase) {
    let cs: Vec<(Option<u64>, u64)> = (0..int(&tc, 0, 4))
        .map(|_| {
            let all = int(&tc, 0, 20) as u64;
            (flag(&tc).then(|| int(&tc, 0, 30) as u64), all)
        })
        .collect();
    let c = capacity(&cs);
    let full: u64 = cs.iter().map(|x| x.1).sum();
    assert!(c <= full);
    let below = below_one_copy(&cs);
    for (i, (max, all)) in cs.iter().enumerate() {
        assert_eq!(below.contains(&i), max.is_some_and(|m| m < *all));
    }
    // The limits warned about are exactly what keeps the capacity below
    // one copy per custodian; no limit, or a large one, never warns.
    assert_eq!(c < full, !below.is_empty());
    let unlimited: Vec<_> = cs.iter().map(|(_, all)| (None, *all)).collect();
    assert!(below_one_copy(&unlimited).is_empty());
    let k = int(&tc, 1, 4) as u32;
    assert_eq!(no_spare(cs.len(), k), cs.len() == k as usize);
}

#[test]
fn a_key_names_its_size() {
    assert_eq!(key_size("SHA256E-s1234--abcd.txt"), Some(1234));
    assert_eq!(key_size("SHA256E-s0--e3b0"), Some(0));
    assert_eq!(key_size("SHA256-m17-s99--abcd"), Some(99));
    assert_eq!(key_size("SHA256E--abcd-s12.txt"), None);
    assert_eq!(key_size("URL--http://x-s1"), None);
}
