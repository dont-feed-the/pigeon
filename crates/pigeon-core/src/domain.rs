//! Failure domains: which custodians fail together (ADR 0046, 06 §6.2).
//!
//! Custodians whose storage fingerprints intersect, whose host is equal, or
//! whose roster `domain` is equal are one domain: the domains are the
//! connected components of the "shares a fact" relation.
//!
//! Laws (`tests/domain.rs`): the domains partition the custodians; they do
//! not depend on the order of the input; two custodians sharing a fact are
//! in one domain; adding a fact can only merge domains, never split them;
//! custodians sharing nothing are one domain each.

use std::collections::{BTreeMap, BTreeSet};

/// What is known of one custodian's hardware.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts<D> {
    /// The custodian.
    pub device: D,
    /// Storage fingerprints it reports (file-system UUIDs, disk serials).
    pub fingerprints: BTreeSet<String>,
    /// The host it reports, if any.
    pub host: Option<String>,
    /// The `domain` the roster declares for it, if any.
    pub domain: Option<String>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Fact<'a> {
    Fingerprint(&'a str),
    Host(&'a str),
    Domain(&'a str),
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra != rb {
        let (low, high) = if ra < rb { (ra, rb) } else { (rb, ra) };
        parent[high] = low;
    }
}

/// The failure domains of `custodians`, each a set of devices, sorted by
/// their smallest device. Entries naming one device are one custodian.
pub fn domains<D: Ord + Clone>(custodians: &[Facts<D>]) -> Vec<BTreeSet<D>> {
    let mut parent: Vec<usize> = (0..custodians.len()).collect();
    let mut first: BTreeMap<Fact<'_>, usize> = BTreeMap::new();
    let mut by_device: BTreeMap<&D, usize> = BTreeMap::new();
    for (i, c) in custodians.iter().enumerate() {
        if let Some(&j) = by_device.get(&c.device) {
            union(&mut parent, i, j);
        } else {
            by_device.insert(&c.device, i);
        }
        let facts = c
            .fingerprints
            .iter()
            .map(|f| Fact::Fingerprint(f))
            .chain(c.host.as_deref().map(Fact::Host))
            .chain(c.domain.as_deref().map(Fact::Domain));
        for fact in facts {
            match first.get(&fact) {
                Some(&j) => union(&mut parent, i, j),
                None => {
                    first.insert(fact, i);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, BTreeSet<D>> = BTreeMap::new();
    for (i, c) in custodians.iter().enumerate() {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().insert(c.device.clone());
    }
    let mut out: Vec<BTreeSet<D>> = groups.into_values().collect();
    out.sort();
    out
}
