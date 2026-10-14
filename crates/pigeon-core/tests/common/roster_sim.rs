//! A simulated group signing and merging roster changes, as the `Roster`
//! TLA+ model does (models/tla/Roster.tla, design mode: fold, signer rule
//! "some", removals "always"). Persons 1..=NP, 1 and 2 admins; device i is
//! person i's for i <= NP; devices NP+1..=ND are fresh keys. The changes a
//! replica may sign are the model's `Ops`; the rosters come from pigeon-core.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::roster::{
    Commit, Device, DeviceId, Edit, Field, History, Person, PersonId, Role, Roster, Value, apply,
    may_sign, may_sign_merge,
};

/// Persons.
pub const NP: usize = 3;
/// Devices, fresh ones included.
pub const ND: usize = 5;
/// Replicas: devices that commit on their own branch.
pub const NR: usize = 2;
/// Author times.
pub const TIMES: [i64; 3] = [0, 1, 2];

/// Person `i`'s ID.
pub fn pid(i: usize) -> PersonId {
    PersonId(format!("p{i}"))
}

/// Device `i`'s ID.
pub fn did(i: usize) -> DeviceId {
    DeviceId(format!("d{i}"))
}

/// The number of a device ID.
pub fn dnum(d: &DeviceId) -> usize {
    d.0[1..].parse().unwrap_or(0)
}

/// The number of a person ID.
pub fn pnum(p: &PersonId) -> usize {
    p.0[1..].parse().unwrap_or(0)
}

/// Device `i` of person `p`: every field a function of both.
pub fn device(i: usize, p: usize) -> Device {
    Device {
        person: pid(p),
        name: format!("device {i}"),
        role: Role::Laptop,
        annex_uuid: Some(format!("uuid-{i}")),
        maxsize: None,
        domain: None,
        site: None,
    }
}

/// The model's `Genesis`.
pub fn genesis() -> Roster {
    Roster {
        rules: 1,
        group: "group".into(),
        k: 1,
        min_sites: 1,
        retention_days: 30,
        persons: (1..=NP)
            .map(|i| {
                (
                    pid(i),
                    Person {
                        name: format!("person {i}"),
                        admin: i <= 2,
                    },
                )
            })
            .collect(),
        devices: (1..=NP).map(|i| (did(i), device(i, i))).collect(),
        removed: BTreeMap::new(),
        folders: Vec::new(),
    }
}

/// One logged step.
#[derive(Clone, Debug)]
pub enum Logged {
    /// `Commit(r, t)` with the op chosen, its edits and the roster written.
    Commit {
        r: usize,
        t: i64,
        op: &'static str,
        edits: Vec<Edit>,
        ros: Roster,
    },
    /// `Merge(r, y)`: a fast-forward, or a merge commit with its roster.
    Merge {
        r: usize,
        y: u32,
        ff: bool,
        ros: Roster,
    },
}

/// The simulated group.
pub struct Sim {
    /// The accepted commits.
    pub history: History<u32>,
    /// Each replica's head.
    pub head: Vec<u32>,
    /// Each replica's signing device.
    pub key: Vec<usize>,
    /// The person each fresh key was first added for (the model's `FreshFor`).
    pub fresh_owner: BTreeMap<usize, usize>,
    /// The roster each commit's tree holds.
    pub tree: BTreeMap<u32, Roster>,
    /// The steps taken.
    pub log: Vec<Logged>,
}

impl Default for Sim {
    fn default() -> Self {
        Sim::new()
    }
}

impl Sim {
    /// A group at its genesis, commit 1.
    pub fn new() -> Sim {
        let g = genesis();
        Sim {
            history: History {
                genesis: 1,
                roster: g.clone(),
                commits: BTreeMap::new(),
            },
            head: vec![1; NR],
            key: (1..=NR).collect(),
            fresh_owner: BTreeMap::new(),
            tree: [(1, g)].into(),
            log: Vec::new(),
        }
    }

    fn next_id(&self) -> u32 {
        self.history.commits.len() as u32 + 2
    }

    /// Whether `x` is an ancestor of `y` (or `y` itself).
    pub fn is_anc(&self, x: u32, y: u32) -> bool {
        self.history.ancestors(&y).contains(&x)
    }

    fn can_add(&self, s: &Roster, n: usize, q: usize) -> bool {
        !s.devices.contains_key(&did(n))
            && !s.removed.contains_key(&did(n))
            && self.fresh_owner.get(&n).is_none_or(|&o| o == q)
    }

    /// The model's `Ops(S, ps)`: (op, edits).
    pub fn ops(&self, s: &Roster, ps: usize) -> Vec<(&'static str, Vec<Edit>)> {
        let adm = s.persons.get(&pid(ps)).is_some_and(|p| p.admin);
        let mem: Vec<usize> = s.persons.keys().map(pnum).collect();
        let admins: BTreeSet<usize> = s
            .persons
            .iter()
            .filter(|(_, p)| p.admin)
            .map(|(i, _)| pnum(i))
            .collect();
        let present: Vec<(usize, usize)> = s
            .devices
            .iter()
            .map(|(d, x)| (dnum(d), pnum(&x.person)))
            .collect();
        let set = |q: usize, b: bool| Edit::Set {
            field: Field::PersonAdmin(pid(q)),
            from: Value::Flag(!b),
            to: Value::Flag(b),
        };
        let mut out = Vec::new();
        if adm {
            out.push(("retire", vec![set(ps, false)]));
            for &q in admins.iter().filter(|&&q| q != ps) {
                out.push(("demote", vec![set(q, false)]));
            }
            for &q in mem.iter().filter(|q| !admins.contains(q)) {
                out.push(("promote", vec![set(q, true)]));
            }
        }
        for n in NP + 1..=ND {
            for &q in &mem {
                if (adm || q == ps) && self.can_add(s, n, q) {
                    out.push(("add", vec![Edit::AddDevice(did(n), device(n, q))]));
                }
            }
        }
        for &(d, owner) in &present {
            if adm || owner == ps {
                out.push(("remove", vec![Edit::RemoveDevice(did(d))]));
                for n in NP + 1..=ND {
                    if self.can_add(s, n, owner) {
                        out.push((
                            "rotate",
                            vec![
                                Edit::RemoveDevice(did(d)),
                                Edit::AddDevice(did(n), device(n, owner)),
                            ],
                        ));
                    }
                }
            }
        }
        if adm {
            for &q in mem.iter().filter(|&&q| q != ps) {
                out.push(("remove-person", vec![Edit::RemovePerson(pid(q))]));
            }
        }
        out
    }

    /// `Commit(r, t)` with the op `choose` picks among those that apply.
    /// Returns false when replica `r` can sign nothing.
    pub fn commit(&mut self, r: usize, t: i64, choose: &mut dyn FnMut(usize) -> usize) -> bool {
        let x = self.head[r];
        let s = self.tree[&x].clone();
        let Some(signer) = s.devices.get(&did(self.key[r])) else {
            return false;
        };
        let ps = pnum(&signer.person);
        let ops: Vec<(&'static str, Vec<Edit>)> = self
            .ops(&s, ps)
            .into_iter()
            .filter(|(_, e)| apply(&s, e).skipped.is_none())
            .collect();
        for (op, e) in &ops {
            assert!(
                may_sign(&s, &did(self.key[r]), e),
                "pigeon-core refuses the model's {op} {e:?}"
            );
        }
        if ops.is_empty() {
            return false;
        }
        let (op, edits) = ops[choose(ops.len())].clone();
        let id = self.next_id();
        let ros = apply(&s, &edits).roster;
        self.history.commits.insert(
            id,
            Commit {
                parents: vec![x],
                time: t,
                signer: did(self.key[r]),
                change: edits.clone(),
            },
        );
        for e in &edits {
            if let Edit::AddDevice(d, dev) = e {
                self.fresh_owner.entry(dnum(d)).or_insert(pnum(&dev.person));
            }
        }
        if op == "rotate"
            && edits.contains(&Edit::RemoveDevice(did(self.key[r])))
            && let Some(Edit::AddDevice(d, _)) = edits.get(1)
        {
            self.key[r] = dnum(d);
        }
        self.tree.insert(id, ros.clone());
        self.head[r] = id;
        self.log.push(Logged::Commit {
            r,
            t,
            op,
            edits,
            ros,
        });
        true
    }

    /// `Merge(r, y)`; false if it is not enabled.
    pub fn merge(&mut self, r: usize, y: u32) -> bool {
        let x = self.head[r];
        if x != y && self.is_anc(x, y) {
            self.head[r] = y;
            let ros = self.tree[&y].clone();
            self.log.push(Logged::Merge {
                r,
                y,
                ff: true,
                ros,
            });
            return true;
        }
        let concurrent = !self.is_anc(x, y) && !self.is_anc(y, x);
        let signer = did(self.key[r]);
        if !concurrent || !may_sign_merge(&[&self.tree[&x], &self.tree[&y]], &signer) {
            return false;
        }
        let id = self.next_id();
        self.history.commits.insert(
            id,
            Commit {
                parents: vec![x, y],
                time: 0,
                signer,
                change: Vec::new(),
            },
        );
        let ros = self.history.fold(&id).roster;
        self.tree.insert(id, ros.clone());
        self.head[r] = id;
        self.log.push(Logged::Merge {
            r,
            y,
            ff: false,
            ros,
        });
        true
    }

    /// Run up to `steps` random steps.
    pub fn run(&mut self, steps: usize, choose: &mut dyn FnMut(usize) -> usize) {
        for _ in 0..steps {
            let r = choose(NR);
            if choose(2) == 0 {
                let t = TIMES[choose(TIMES.len())];
                self.commit(r, t, choose);
            } else {
                let n = self.history.commits.len() + 1;
                let y = choose(n) as u32 + 1;
                self.merge(r, y);
            }
        }
    }

    /// Every commit ID, the genesis included.
    pub fn ids(&self) -> Vec<u32> {
        (1..=self.history.commits.len() as u32 + 1).collect()
    }

    /// The commits with a change among `c`'s ancestors.
    pub fn changes_of(&self, c: u32) -> BTreeSet<u32> {
        self.history.canonical_order(&c).into_iter().collect()
    }
}
