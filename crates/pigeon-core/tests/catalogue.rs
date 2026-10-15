//! Laws of the catalogue merge (05 §5.2; ADR 0012), as hegel-rust property
//! tests on random schedules of three devices: saves, deletions, renames,
//! *Keep mine*, *Keep both*, edits of copies and pulls, over names that
//! fold alike, packages and folders. Laws: `tree` (an accepted commit's
//! tree is the catalogue of its ancestry), `names`, `kept`, `seen`,
//! `order` (judging the same commits in another order gives the same
//! verdicts and catalogues; devices agree after a full sync), `algebra` (a
//! clean merge's catalogue is its parents') and `authority` (each copy is
//! allowed to its representative's person). The fixed cases are in
//! `tests/catalogue_oracle.rs`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]

use std::collections::{BTreeMap, BTreeSet};

use hegel::TestCase;
use hegel::generators as gs;
use pigeon_core::catalogue::{Content, Engine, Meta, Outcome, Tree, units};
use pigeon_core::names::{Stamp, fold, is_copy_of, portable_tree};

const DAY: i64 = 1_791_849_600 + 14 * 3600;

const PATHS: [&str; 14] = [
    "team/a.txt",
    "team/A.txt",
    "team/b.txt",
    "team/Straße/x.txt",
    "team/STRASSE/y.txt",
    "team/strasse",
    "team/R.pages/index.xml",
    "team/r.PAGES/data/p.bin",
    "team/Photo.jpg",
    "team/photo.jpg",
    "team/é.txt",
    "alice/a.txt",
    "alice/Notes/n.txt",
    "a.txt",
];

fn may(person: &str, path: &str, added: bool) -> bool {
    let folded = fold(path);
    let parts: Vec<&str> = folded.split('/').collect();
    for i in (1..parts.len()).rev() {
        match parts[..i].join("/").as_str() {
            "team" => return added || ["Alice", "Bob", "Paul"].contains(&person),
            "alice" => return person == "Alice",
            _ => {}
        }
    }
    added || person == "Alice"
}

fn int(tc: &TestCase, max: u8) -> usize {
    usize::from(tc.draw(gs::integers::<u8>().max_value(max)))
}

struct Commit {
    meta: Meta,
    tree: Tree<String>,
    person: String,
    anc: BTreeSet<String>,
}

struct Sim {
    engine: Engine<String>,
    commits: BTreeMap<String, Commit>,
    order: Vec<String>,
    people: Vec<&'static str>,
    tips: Vec<String>,
    known: Vec<BTreeSet<String>>,
}

impl Sim {
    fn new(people: Vec<&'static str>, genesis: Tree<String>) -> Sim {
        let mut sim = Sim {
            engine: Engine::new(),
            commits: BTreeMap::new(),
            order: Vec::new(),
            tips: vec!["g".to_owned(); people.len()],
            known: vec![BTreeSet::from(["g".to_owned()]); people.len()],
            people,
        };
        let meta = meta("g", vec![], "G", "Alice", 0, vec![]);
        assert_eq!(sim.add(meta, genesis, "Alice"), Outcome::Contribution);
        sim
    }

    fn add(&mut self, meta: Meta, tree: Tree<String>, person: &str) -> Outcome {
        let mut anc = BTreeSet::from([meta.id.clone()]);
        for p in &meta.parents {
            anc.extend(self.commits[p].anc.iter().cloned());
        }
        let out = self
            .engine
            .record(&meta, &tree, &|p, added| may(person, p, added));
        self.order.push(meta.id.clone());
        let c = Commit {
            meta,
            tree,
            person: person.to_owned(),
            anc,
        };
        self.commits.insert(c.meta.id.clone(), c);
        out
    }

    fn tree(&self, d: usize) -> &Tree<String> {
        &self.commits[&self.tips[d]].tree
    }

    /// Device `d` saves `tree` (with `resolves`), unless capture would hold
    /// it back; it keeps a proposal beside its main.
    fn save(
        &mut self,
        d: usize,
        tree: Tree<String>,
        when: i64,
        offset: i32,
        resolves: Vec<String>,
    ) {
        if portable_tree(tree.keys().map(String::as_str)).is_err() {
            return;
        }
        let id = format!("c{}", self.order.len());
        let who = self.people[d];
        let device = ["X", "Y", "Z"][d];
        let mut m = meta(&id, vec![self.tips[d].clone()], device, who, when, resolves);
        m.when.offset = offset;
        let out = self.add(m, tree, who);
        self.known[d].insert(id.clone());
        if out != Outcome::Proposal {
            assert!(
                matches!(out, Outcome::Contribution | Outcome::Clean),
                "{out:?}"
            );
            self.tips[d] = id;
        }
    }

    /// Device `i` fetches what `j` has, then fast-forwards, merges or
    /// records `j`'s tip as equivalent (05 §5.2).
    fn pull(&mut self, i: usize, j: usize) {
        let known: Vec<String> = self.known[j].iter().cloned().collect();
        self.known[i].extend(known);
        let (ti, tj) = (self.tips[i].clone(), self.tips[j].clone());
        let (ai, aj) = (&self.commits[&ti].anc, &self.commits[&tj].anc);
        if ai.contains(&tj) {
            return;
        }
        if aj.contains(&ti) {
            self.tips[i] = tj;
            return;
        }
        if !aj
            .difference(ai)
            .any(|c| self.engine.outcome(c) == Some(&Outcome::Contribution))
        {
            return;
        }
        let parents = vec![ti, tj];
        let tree = self.engine.catalogue(&parents).tree.clone();
        let id = format!("m{}", self.order.len());
        let who = self.people[i];
        let m = meta(&id, parents, ["X", "Y", "Z"][i], who, 0, vec![]);
        assert_eq!(
            self.add(m, tree, who),
            Outcome::Clean,
            "law algebra: a merge changes nothing"
        );
        self.known[i].insert(id.clone());
        self.tips[i] = id;
    }

    fn step(&mut self, tc: &TestCase) {
        let d = int(tc, 2);
        let when = DAY + 60 * i64::try_from(int(tc, 3)).unwrap();
        let offset = [0, 120, -300][int(tc, 2)];
        let mut tree = self.tree(d).clone();
        let paths: Vec<String> = tree.keys().cloned().collect();
        let copies: Vec<String> = paths.iter().filter(|p| p.contains(" (")).cloned().collect();
        match int(tc, 6) {
            0 | 1 => {
                tree.insert(PATHS[int(tc, 13)].to_owned(), format!("k{}", int(tc, 3)));
            }
            2 if !paths.is_empty() => {
                tree.remove(&paths[int(tc, 255) % paths.len()]);
            }
            3 if !paths.is_empty() => {
                let from = &paths[int(tc, 255) % paths.len()];
                let k = tree.remove(from).unwrap();
                tree.insert(PATHS[int(tc, 13)].to_owned(), k);
            }
            4 if !copies.is_empty() => {
                // Keep mine: one version at the name, no copies.
                let copy = &copies[int(tc, 255) % copies.len()];
                let Some(p) = paths
                    .iter()
                    .find(|p| !p.contains(" (") && is_copy_of(copy, p))
                else {
                    return;
                };
                let p = p.clone();
                tree.retain(|q, _| !is_copy_of(q, &p));
                tree.insert(p, format!("k{}", int(tc, 3)));
            }
            5 if !copies.is_empty() => {
                // Keep both: no change of files, a resolution.
                let copy = &copies[int(tc, 255) % copies.len()];
                if let Some(p) = paths
                    .iter()
                    .find(|p| !p.contains(" (") && is_copy_of(copy, p))
                {
                    let p = p.clone();
                    return self.save(d, tree, when, offset, vec![p]);
                }
                return;
            }
            6 if !copies.is_empty() => {
                tree.insert(
                    copies[int(tc, 255) % copies.len()].clone(),
                    format!("k{}", int(tc, 3)),
                );
            }
            _ => {
                let j = (d + 1 + int(tc, 1)) % 3;
                return self.pull(d, j);
            }
        }
        self.save(d, tree, when, offset, vec![]);
    }
}

fn meta(
    id: &str,
    parents: Vec<String>,
    device: &str,
    who: &str,
    when: i64,
    resolves: Vec<String>,
) -> Meta {
    Meta {
        id: id.to_owned(),
        parents,
        device: device.to_owned(),
        who: who.to_owned(),
        person: pigeon_core::roster::PersonId(who.to_owned()),
        when: Stamp {
            utc: when,
            offset: 0,
        },
        resolves,
    }
}

fn contents(tree: &Tree<String>) -> BTreeSet<Content<String>> {
    units(tree).into_values().map(|v| v.content).collect()
}

/// Each content of a tree, counted: a contribution removes a key when it
/// holds fewer of it than its base, since one key may be at two paths.
fn counts(tree: &Tree<String>) -> BTreeMap<Content<String>, usize> {
    let mut out = BTreeMap::new();
    for v in units(tree).into_values() {
        *out.entry(v.content).or_insert(0) += 1;
    }
    out
}

fn run(tc: &TestCase) -> Sim {
    let people = if tc.draw(gs::booleans()) {
        vec!["Alice", "Bob", "Paul"]
    } else {
        vec!["Alice", "Alice", "Bob"]
    };
    let mut genesis = Tree::new();
    for p in ["team/a.txt", "team/R.pages/index.xml", "alice/a.txt"] {
        if tc.draw(gs::booleans()) {
            genesis.insert(p.to_owned(), "k0".to_owned());
        }
    }
    let mut sim = Sim::new(people, genesis);
    for _ in 0..int(tc, 14) {
        sim.step(tc);
    }
    sim
}

/// `tree`, `names`, `kept` and `seen` on every device's main.
fn check_views(sim: &mut Sim) {
    for d in 0..3 {
        let known: Vec<String> = sim.known[d].iter().cloned().collect();
        let view = sim.engine.catalogue(&known);
        let cat = view.tree.clone();
        assert_eq!(&cat, sim.tree(d), "law tree: device {d} shows its main");
        assert!(
            portable_tree(cat.keys().map(String::as_str)).is_ok(),
            "law names: {cat:?}"
        );
        let contribs: Vec<&String> = known
            .iter()
            .filter(|c| sim.engine.outcome(c) == Some(&Outcome::Contribution))
            .collect();
        let cur = contents(&cat);
        let mut removed = BTreeSet::new();
        let mut bases = BTreeMap::new();
        for c in &contribs {
            let parents = sim.commits[*c].meta.parents.clone();
            let base = sim.engine.catalogue(&parents).tree.clone();
            let after = counts(&sim.commits[*c].tree);
            for (k, n) in counts(&base) {
                if after.get(&k).copied().unwrap_or(0) < n {
                    removed.insert(k);
                }
            }
            bases.insert((*c).clone(), contents(&base));
        }
        for c in &contribs {
            let mine = contents(&sim.commits[*c].tree);
            for k in mine.difference(&bases[*c]) {
                assert!(
                    cur.contains(k) || removed.contains(k),
                    "law kept: {k:?} of {c} left {cat:?}"
                );
            }
        }
        let tops = contribs.iter().filter(|c| {
            !contribs
                .iter()
                .any(|e| e != *c && sim.commits[*e].anc.contains(**c))
        });
        let seen: BTreeSet<Content<String>> =
            tops.flat_map(|c| contents(&sim.commits[*c].tree)).collect();
        let unseen: Vec<_> = cur.difference(&seen).collect();
        assert!(
            unseen.is_empty(),
            "law seen: {unseen:?} are in {cat:?} but in no newest contribution"
        );
    }
}

#[hegel::test(test_cases = 1000)]
fn the_catalogue_keeps_its_laws_on_every_schedule(tc: TestCase) {
    let mut sim = run(&tc);
    check_views(&mut sim);
    for id in sim.order.clone() {
        if sim.engine.accepted(&id) {
            let c = &sim.commits[&id];
            assert_eq!(
                sim.engine.catalogue(std::slice::from_ref(&id)).tree,
                c.tree,
                "law tree: {id}"
            );
        }
    }
    for (path, origin) in sim.engine.copies() {
        let person = &sim.commits[&origin].person;
        assert!(
            may(person, &path, true),
            "law authority: copy {path} is not allowed to {person}"
        );
    }
    for (i, j) in [(0, 1), (0, 2), (1, 0), (2, 0)] {
        sim.pull(i, j);
    }
    check_views(&mut sim);
    let trees: Vec<&Tree<String>> = (0..3).map(|d| sim.tree(d)).collect();
    assert!(
        trees.iter().all(|t| *t == trees[0]),
        "law order: devices differ after a full sync"
    );
}

#[hegel::test(test_cases = 1000)]
fn the_verdicts_do_not_depend_on_the_order_commits_arrive_in(tc: TestCase) {
    let mut sim = run(&tc);
    let mut other: Engine<String> = Engine::new();
    let mut pending = sim.order.clone();
    let mut done = BTreeSet::new();
    while !pending.is_empty() {
        let ready: Vec<usize> = (0..pending.len())
            .filter(|&i| {
                sim.commits[&pending[i]]
                    .meta
                    .parents
                    .iter()
                    .all(|p| done.contains(p))
            })
            .collect();
        let id = pending.remove(ready[int(&tc, 255) % ready.len()]);
        let c = &sim.commits[&id];
        let person = c.person.clone();
        let out = other.record(&c.meta, &c.tree, &|p, added| may(&person, p, added));
        assert_eq!(
            Some(&out),
            sim.engine.outcome(&id),
            "law order: verdict of {id}"
        );
        done.insert(id);
    }
    for id in &sim.order {
        let ids = [id.clone()];
        assert_eq!(
            other.catalogue(&ids).tree,
            sim.engine.catalogue(&ids).tree,
            "law order: catalogue of {id}"
        );
        assert_eq!(
            other.open(&ids),
            sim.engine.open(&ids),
            "law order: open conflicts of {id}"
        );
    }
}
