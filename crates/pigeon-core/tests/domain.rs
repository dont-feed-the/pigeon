//! Laws of failure domains (ADR 0049; pigeon-core `domain`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeSet;

use common::{flag, index, int, label};
use hegel::TestCase;
use pigeon_core::domain::{Facts, domains};

fn facts(tc: &TestCase) -> Vec<Facts<u8>> {
    let n = int(tc, 0, 6);
    (0..n)
        .map(|i| Facts {
            device: i as u8,
            fingerprints: (0..int(tc, 0, 2))
                .map(|_| format!("f{}", int(tc, 0, 5)))
                .collect(),
            host: label(tc, "h", 4),
            domain: label(tc, "x", 3),
        })
        .collect()
}

fn shares(a: &Facts<u8>, b: &Facts<u8>) -> bool {
    a.fingerprints
        .intersection(&b.fingerprints)
        .next()
        .is_some()
        || (a.host.is_some() && a.host == b.host)
        || (a.domain.is_some() && a.domain == b.domain)
}

#[hegel::test(test_cases = 1000)]
fn domains_partition_the_custodians(tc: TestCase) {
    let cs = facts(&tc);
    let ds = domains(&cs);
    let all: BTreeSet<u8> = cs.iter().map(|c| c.device).collect();
    let union: BTreeSet<u8> = ds.iter().flatten().copied().collect();
    assert_eq!(union, all);
    assert_eq!(ds.iter().map(BTreeSet::len).sum::<usize>(), all.len());
    assert!(ds.iter().all(|d| !d.is_empty()));
}

#[hegel::test(test_cases = 1000)]
fn domains_ignore_input_order(tc: TestCase) {
    let cs = facts(&tc);
    let mut reversed = cs.clone();
    reversed.reverse();
    assert_eq!(domains(&cs), domains(&reversed));
}

#[hegel::test(test_cases = 1000)]
fn sharing_a_fact_is_one_domain(tc: TestCase) {
    let cs = facts(&tc);
    let ds = domains(&cs);
    let of = |d: u8| ds.iter().position(|s| s.contains(&d));
    for a in &cs {
        for b in &cs {
            if shares(a, b) {
                assert_eq!(of(a.device), of(b.device));
            }
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn a_new_fact_only_merges(tc: TestCase) {
    let cs = facts(&tc);
    tc.assume(!cs.is_empty());
    let before = domains(&cs);
    let mut more = cs.clone();
    let i = index(&tc, more.len());
    if flag(&tc) || more[i].domain.is_some() {
        more[i].fingerprints.insert(format!("f{}", int(&tc, 0, 5)));
    } else {
        more[i].domain = Some(format!("x{}", int(&tc, 0, 3)));
    }
    let after = domains(&more);
    assert!(after.len() <= before.len());
    for d in &before {
        assert!(after.iter().any(|e| d.is_subset(e)));
    }
}

#[hegel::test(test_cases = 1000)]
fn custodians_sharing_nothing_are_one_domain_each(tc: TestCase) {
    let n = int(&tc, 0, 6);
    let cs: Vec<Facts<u8>> = (0..n)
        .map(|i| Facts {
            device: i as u8,
            fingerprints: [format!("f{i}")].into(),
            host: Some(format!("h{i}")),
            domain: None,
        })
        .collect();
    assert_eq!(domains(&cs).len(), n as usize);
}
