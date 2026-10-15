//! The catalogue merge on random histories, logged as `Catalogue` trace
//! events for TLC (05 §5.2, 13 §13.3). The world is `Catalogue.tla`'s:
//! devices X, Y and Z of Alice, Bob and Paul; paths in `team/` that anyone
//! may change, `alice/a.txt` that only Alice may (a proposal otherwise),
//! the root `a.txt`, add-only with Alice as steward, and a package. Saves
//! are a minute apart, so conflict names never repeat; no two paths fold
//! alike. Each commit, merges included, is logged as `catalogue::Engine`
//! judges it, in abstract slots (a copy is its slot and the commit it
//! represents), with the verdict, the catalogue and the open conflicts;
//! `CatalogueTrace.tla` checks them against `Record`. With
//! `PIGEON_TRACE_DIR` set, one NDJSON file per seed is written there.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(feature = "tla-trace")]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use pigeon_core::catalogue::{Content, Engine, Meta, Outcome, Tree, units};
use pigeon_core::names::{Stamp, fold};
use pigeon_core::trace::{Catalogue, Event, Value};

const DAY: i64 = 1_791_849_600;
const PATHS: [&str; 6] = [
    "team/a.txt",
    "team/b.txt",
    "alice/a.txt",
    "a.txt",
    "team/R.pages/index.xml",
    "team/R.pages/data.bin",
];
const PEOPLE: [&str; 3] = ["Alice", "Bob", "Paul"];

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[usize::try_from(self.below(xs.len() as u64)).unwrap()]
    }
}

fn may(person: &str, path: &str, added: bool) -> bool {
    let p = fold(path);
    if p.starts_with("alice/") {
        person == "Alice"
    } else if p.contains('/') {
        true
    } else {
        added || person == "Alice"
    }
}

fn s(x: &str) -> Value {
    Value::Str(x.to_owned())
}

fn rec(fields: Vec<(&str, Value)>) -> Value {
    Value::Record(fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn content(c: &Content<String>) -> String {
    match c {
        Content::File(k) => k.clone(),
        Content::Package(files) => {
            let parts: Vec<String> = files.iter().map(|(p, k)| format!("{p}={k}")).collect();
            format!("pkg({})", parts.join(","))
        }
    }
}

struct World {
    engine: Engine<String>,
    trees: BTreeMap<String, Tree<String>>,
    anc: BTreeMap<String, BTreeSet<String>>,
    /// Each code slot's abstract slot: `[path]`, or a copy's slot and origin.
    abs: BTreeMap<String, Vec<String>>,
    tips: [String; 3],
    events: Vec<Event>,
    keys: u32,
    minute: i64,
}

impl World {
    fn slot(&self, x: &str) -> Value {
        Value::List(self.abs[x].iter().map(|p| s(p)).collect())
    }

    /// Learns the abstract slot of every copy a catalogue shows.
    fn learn(&mut self, id: &str) {
        let view = self.engine.catalogue(&[id.to_owned()]);
        for (x, shown) in &view.shown {
            for v in shown {
                let y = fold(&v.path);
                if y != *x && !self.abs.contains_key(&y) {
                    let mut a = self.abs[x].clone();
                    a.push(v.origin.clone());
                    self.abs.insert(y, a);
                }
            }
        }
    }

    fn record(
        &mut self,
        d: usize,
        parents: Vec<String>,
        tree: Tree<String>,
        resolves: Vec<String>,
    ) -> Outcome {
        let id = format!("c{:04}", self.trees.len());
        self.minute += 1;
        let device = if parents.is_empty() {
            "G"
        } else {
            ["X", "Y", "Z"][d]
        };
        let meta = Meta {
            id: id.clone(),
            parents: parents.clone(),
            device: device.to_owned(),
            who: PEOPLE[d].to_owned(),
            person: pigeon_core::roster::PersonId(PEOPLE[d].to_owned()),
            renames: Vec::new(),
            when: Stamp {
                utc: DAY + 60 * self.minute,
                offset: 0,
            },
            resolves: resolves.clone(),
        };
        let person = PEOPLE[d];
        let out = self
            .engine
            .record(&meta, &tree, &|p, added| may(person, p, added));
        let mut anc: BTreeSet<String> = parents
            .iter()
            .flat_map(|p| self.anc[p].iter().cloned())
            .collect();
        anc.insert(id.clone());
        self.anc.insert(id.clone(), anc);
        self.trees.insert(id.clone(), tree.clone());
        let accepted = matches!(out, Outcome::Clean | Outcome::Contribution);
        if accepted {
            self.learn(&id);
        }
        let rank = match self.engine.rank(&id) {
            Some(r) => {
                let dev = ["G", "X", "Y", "Z"]
                    .iter()
                    .position(|x| *x == r.device)
                    .unwrap();
                let n = r.id[1..].parse::<i64>().unwrap();
                vec![(r.when - DAY) / 60, dev as i64, i64::from(r.seq), n]
            }
            None => vec![0, 0, 0, 0],
        };
        let tree_v = units(&tree)
            .iter()
            .map(|(x, v)| rec(vec![("s", self.slot(x)), ("c", s(&content(&v.content)))]))
            .collect();
        let (view, open) = if accepted {
            let ids = [id.clone()];
            let view = self.engine.catalogue(&ids);
            let shown = view
                .shown
                .values()
                .flatten()
                .map(|v| {
                    rec(vec![
                        ("s", self.slot(&fold(&v.path))),
                        ("c", s(&content(&v.version.content))),
                        ("o", s(&v.origin)),
                    ])
                })
                .collect();
            let open = self
                .engine
                .open(&ids)
                .iter()
                .map(|x| self.slot(x))
                .collect();
            (shown, open)
        } else {
            (vec![], vec![])
        };
        let outcome = match &out {
            Outcome::Clean => "clean",
            Outcome::Contribution => "contribution",
            Outcome::Proposal => "proposal",
            Outcome::Refused(_) => "refused",
        };
        let res = resolves
            .iter()
            .filter(|p| self.abs.contains_key(&fold(p)))
            .map(|p| self.slot(&fold(p)))
            .collect();
        let vars = vec![
            ("id".to_owned(), s(&id)),
            (
                "parents".to_owned(),
                Value::List(parents.iter().map(|p| s(p)).collect()),
            ),
            (
                "rank".to_owned(),
                Value::List(rank.into_iter().map(Value::Int).collect()),
            ),
            ("alice".to_owned(), Value::Bool(person == "Alice")),
            ("tree".to_owned(), Value::List(tree_v)),
            ("res".to_owned(), Value::List(res)),
            ("outcome".to_owned(), s(outcome)),
            ("view".to_owned(), Value::List(view)),
            ("open".to_owned(), Value::List(open)),
        ];
        let clock = self.events.len() as u64 + 1;
        self.events
            .push(Event::new(Catalogue::Record, "l1", clock, vars));
        if matches!(out, Outcome::Contribution | Outcome::Clean) && !parents.is_empty() {
            self.tips[d] = id;
        }
        out
    }

    fn key(&mut self) -> String {
        self.keys += 1;
        format!("k{}", self.keys)
    }

    fn pull(&mut self, i: usize, j: usize) {
        let (ti, tj) = (self.tips[i].clone(), self.tips[j].clone());
        if self.anc[&ti].contains(&tj) {
            return;
        }
        if self.anc[&tj].contains(&ti) {
            self.tips[i] = tj;
            return;
        }
        let new: Vec<String> = self.anc[&tj].difference(&self.anc[&ti]).cloned().collect();
        if !new
            .iter()
            .any(|c| self.engine.outcome(c) == Some(&Outcome::Contribution))
        {
            return;
        }
        let parents = vec![ti, tj];
        let tree = self.engine.catalogue(&parents).tree.clone();
        assert_eq!(self.record(i, parents, tree, vec![]), Outcome::Clean);
    }

    fn step(&mut self, rng: &mut Rng) {
        let d = usize::try_from(rng.below(3)).unwrap();
        let mut tree = self.trees[&self.tips[d]].clone();
        let unit_paths: Vec<String> = units(&tree).values().map(|v| v.path.clone()).collect();
        let copies: Vec<String> = unit_paths
            .iter()
            .filter(|p| p.contains(" ("))
            .cloned()
            .collect();
        let base_of = |c: &str| {
            PATHS
                .iter()
                .map(|p| pigeon_core::names::unit(p).to_owned())
                .find(|p| pigeon_core::names::is_copy_of(c, p))
        };
        let mut resolves = vec![];
        match rng.below(20) {
            0..=5 => {
                let p = if rng.below(3) == 0 {
                    *rng.pick(&PATHS)
                } else {
                    *rng.pick(&[PATHS[0], PATHS[1], PATHS[4]])
                };
                let k = self.key();
                tree.insert(p.to_owned(), k);
            }
            6 | 7 if !unit_paths.is_empty() => {
                let u = rng.pick(&unit_paths).clone();
                tree.retain(|p, _| *p != u && !p.starts_with(&format!("{u}/")));
            }
            8 if !unit_paths.is_empty() => {
                let u = rng.pick(&unit_paths).clone();
                let to = *rng.pick(&PATHS[..4]);
                if let Some(k) = tree.remove(&u) {
                    tree.insert(to.to_owned(), k);
                }
            }
            9 | 10 if !copies.is_empty() => {
                let c = rng.pick(&copies).clone();
                let k = self.key();
                if tree.contains_key(&c) {
                    tree.insert(c, k);
                } else {
                    tree.insert(format!("{c}/index.xml"), k);
                }
            }
            11 if !copies.is_empty() => {
                let c = rng.pick(&copies).clone();
                if let Some(p) = base_of(&c) {
                    tree.retain(|q, _| {
                        !pigeon_core::names::is_copy_of(pigeon_core::names::unit(q), &p)
                    });
                    if !p.ends_with(".pages") {
                        let k = self.key();
                        tree.insert(p, k);
                    }
                }
            }
            12 if !copies.is_empty() => {
                let c = rng.pick(&copies).clone();
                if let Some(p) = base_of(&c) {
                    resolves.push(p);
                }
            }
            _ => {
                let j = (d + 1 + usize::try_from(rng.below(2)).unwrap()) % 3;
                return self.pull(d, j);
            }
        }
        let parents = vec![self.tips[d].clone()];
        self.record(d, parents, tree, resolves);
    }
}

fn run(seed: u64) -> Vec<Event> {
    let mut abs = BTreeMap::new();
    for p in PATHS {
        let u = pigeon_core::names::unit(p);
        abs.insert(fold(u), vec![u.to_owned()]);
    }
    let mut w = World {
        engine: Engine::new(),
        trees: BTreeMap::new(),
        anc: BTreeMap::new(),
        abs,
        tips: [String::new(), String::new(), String::new()],
        events: vec![],
        keys: 0,
        minute: 0,
    };
    let genesis = Tree::from([
        ("team/a.txt".to_owned(), "k0".to_owned()),
        ("alice/a.txt".to_owned(), "k0".to_owned()),
    ]);
    w.record(0, vec![], genesis, vec![]);
    w.tips = ["c0000".to_owned(), "c0000".to_owned(), "c0000".to_owned()];
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    for _ in 0..50 {
        w.step(&mut rng);
    }
    for (i, j) in [(0, 1), (0, 2), (1, 0), (2, 0)] {
        w.pull(i, j);
    }
    w.events
}

#[test]
fn traces_for_tlc() {
    let seeds: u64 = std::env::var("PIGEON_TRACE_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let dir = std::env::var_os("PIGEON_TRACE_DIR").map(PathBuf::from);
    let mut copies = 0;
    for seed in 1..=seeds {
        let events = run(seed);
        copies += events
            .iter()
            .filter(|e| e.to_ndjson().contains("\"o\":\"c"))
            .count();
        let lines: Vec<String> = events.iter().map(Event::to_ndjson).collect();
        if let Some(d) = &dir {
            fs::create_dir_all(d).unwrap();
            fs::write(
                d.join(format!("catalogue-{seed:03}.ndjson")),
                lines.join("\n") + "\n",
            )
            .unwrap();
        }
    }
    assert!(copies > 0, "the histories have catalogues to check");
}
