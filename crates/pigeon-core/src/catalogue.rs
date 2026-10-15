//! The logical catalogue and its deterministic merge (05 §5.1–5.3, P5): a
//! pure function of a set of accepted commits, built from events — the
//! contributions, and the conflict events they froze.
//!
//! [`Engine`] judges commits in an order where parents come first, as
//! `models/catalogue/catalogue.py` (mode `new`, the oracle) does: each
//! commit's base B(c) is the catalogue of its accepted parents' states, its
//! Δ is taken by slot (05 §5.1), and a commit that changes something is a
//! contribution, or a proposal when its person may not. Laws
//! (`tests/catalogue.rs`, every schedule of the oracle's cases in
//! `tests/catalogue_oracle.rs`): order, tree, kept, seen, names, algebra
//! and authority of 05 §5.2. `.pigeon/` slots never get conflict names:
//! the first version keeps the name (the roster's is the fold of 04 §4.8,
//! which pigeond writes). A contribution's renames (`Pigeon-Renamed:`,
//! ADR 0072) carry the writes of the old slot made concurrently with them
//! to the new one ([`Engine::effective`]).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::names::{self, Stamp};
use crate::roster::PersonId;

mod rename;

/// The most catalogues an engine keeps computed.
const MEMO: usize = 256;

/// A tree: path → leaf (a key, or an entry of `ls-tree`).
pub type Tree<K> = BTreeMap<String, K>;

/// What a slot holds: one file, or a whole package (07 §7.9).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Content<K> {
    /// A file.
    File(K),
    /// A package: its files, by path below it.
    Package(BTreeMap<String, K>),
}

/// A value of a slot: its spelling and its content.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version<K> {
    /// The unit's path, as written.
    pub path: String,
    /// What it holds.
    pub content: Content<K>,
}

/// A tree's units by slot (its folded unit path, 05 §5.1).
pub fn units<K: Clone + Ord>(tree: &Tree<K>) -> BTreeMap<String, Version<K>> {
    let mut groups: BTreeMap<&str, BTreeMap<String, K>> = BTreeMap::new();
    for (path, k) in tree {
        let u = names::unit(path);
        let rel = path.get(u.len() + 1..).unwrap_or_default();
        groups
            .entry(u)
            .or_default()
            .insert(rel.to_owned(), k.clone());
    }
    groups
        .into_iter()
        .map(|(u, mut files)| {
            let content = match files.remove("") {
                Some(k) if files.is_empty() => Content::File(k),
                _ => Content::Package(files),
            };
            (
                names::fold(u),
                Version {
                    path: u.to_owned(),
                    content,
                },
            )
        })
        .collect()
}

/// The tree of units placed at paths.
pub fn expand<'a, K: Clone + 'a>(
    placed: impl IntoIterator<Item = (&'a str, &'a Content<K>)>,
) -> Tree<K> {
    let mut tree = Tree::new();
    for (path, content) in placed {
        match content {
            Content::File(k) => {
                tree.insert(path.to_owned(), k.clone());
            }
            Content::Package(files) => {
                for (rel, k) in files {
                    tree.insert(format!("{path}/{rel}"), k.clone());
                }
            }
        }
    }
    tree
}

/// What the catalogue needs of a commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    /// Its object ID.
    pub id: String,
    /// Its parents' IDs.
    pub parents: Vec<String>,
    /// The ID of the device that signed it.
    pub device: String,
    /// The display name of its author: the proposal's for an adoption.
    pub who: String,
    /// Its author's person: the proposal's for an adoption.
    pub person: PersonId,
    /// Its author time: the proposal's for an adoption.
    pub when: Stamp,
    /// The paths of its `Pigeon-Resolved:` trailers (*Keep both*).
    pub resolves: Vec<String>,
    /// Its renames, old path and new, from its `Pigeon-Renamed:` trailers.
    pub renames: Vec<(String, String)>,
}

/// Representative order (05 §5.2 step 4): author time, device, that
/// device's earlier contributions, commit ID.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rank {
    /// Author time, in seconds since the epoch.
    pub when: i64,
    /// Device ID.
    pub device: String,
    /// The device's contributions up to this one, in its base.
    pub seq: u32,
    /// Commit ID.
    pub id: String,
}

/// One version where a catalogue shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown<K> {
    /// The version.
    pub version: Version<K>,
    /// The path it is shown at: its own, refolded, or a conflict name.
    pub path: String,
    /// The commit it represents (05 §5.2 step 4).
    pub origin: String,
}

/// The catalogue of a state.
#[derive(Clone, Debug)]
pub struct View<K> {
    /// The tree every device shows (P5).
    pub tree: Tree<K>,
    /// Each slot's versions, in representative order, where they are shown.
    pub shown: BTreeMap<String, Vec<Shown<K>>>,
    /// The slots with several versions, or one away from its own path.
    pub unsettled: Vec<String>,
    /// The persons who wrote each slot (ADR 0070): the authors of the
    /// events that wrote or deleted it, a copy's being its version's.
    pub writers: BTreeMap<String, BTreeSet<PersonId>>,
    heads: BTreeMap<String, Vec<usize>>,
    eff: Effective<K>,
}

/// The writes of a state's events with renames followed (ADR 0072): those
/// of the events whose writes moved, the renames each moved write followed,
/// and where each conflict event's frozen slot went.
#[derive(Clone, Debug)]
struct Effective<K> {
    writes: BTreeMap<usize, BTreeMap<String, Write<K>>>,
    /// For each moved write, by event and new slot: the renames it
    /// followed, and the slot it was written at.
    via: BTreeMap<(usize, String), (BTreeSet<usize>, String)>,
    fslot: BTreeMap<usize, String>,
    /// The pure renames' writes of their new slot that yielded.
    yielded: BTreeSet<(usize, String)>,
}

impl<K> Default for Effective<K> {
    fn default() -> Self {
        Self {
            writes: BTreeMap::new(),
            via: BTreeMap::new(),
            fslot: BTreeMap::new(),
            yielded: BTreeSet::new(),
        }
    }
}

/// How a commit was judged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Accepted, changing nothing: a clean merge.
    Clean,
    /// Accepted, a contribution.
    Contribution,
    /// Not accepted: it changes what its person may not (04 §4.5).
    Proposal,
    /// Not accepted: no parent is, or a name is not portable (rule 5).
    Refused(String),
}

/// The caller's verdict on a commit (04 §4.4), for [`Engine::admit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Judgement {
    /// It may make its change.
    Accepted,
    /// A member signed a change that its person may not make (04 §4.5).
    Proposal,
    /// Refused, and why.
    Refused(String),
}

/// The slots whose value differs between two trees' units.
fn slot_delta<'a, K: PartialEq>(
    old: &'a BTreeMap<String, Version<K>>,
    new: &'a BTreeMap<String, Version<K>>,
) -> Vec<&'a String> {
    old.keys()
        .chain(new.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|x| old.get(*x) != new.get(*x))
        .collect()
}

/// A slot's value in an event, and the commit it represents.
type Write<K> = Option<(Version<K>, String)>;

#[derive(Clone, Debug)]
struct Event<K> {
    /// For a contribution, its commit.
    commit: Option<String>,
    past: BTreeSet<usize>,
    writes: BTreeMap<String, Write<K>>,
    resolves: BTreeSet<String>,
    /// For a conflict event, the slot it froze and whether people are told.
    frozen: Option<(String, bool)>,
    /// For a contribution, each slot it renamed: the new slot, whether the
    /// content stayed the same, and the events whose versions it moved.
    renames: BTreeMap<String, (String, bool, BTreeSet<usize>)>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key<K> {
    Commit(String),
    Frozen(
        String,
        BTreeSet<usize>,
        BTreeSet<(String, Option<Version<K>>)>,
    ),
}

#[derive(Clone, Debug)]
struct Info {
    outcome: Outcome,
    state: Arc<BTreeSet<usize>>,
}

/// A contribution as its versions show it: rank, author's name, time and
/// person.
type Origin = (Rank, String, Stamp, PersonId);

/// The catalogue of every commit judged so far.
#[derive(Clone, Debug)]
pub struct Engine<K> {
    events: Vec<Event<K>>,
    keys: BTreeMap<Key<K>, usize>,
    info: BTreeMap<String, Info>,
    origins: BTreeMap<String, Origin>,
    memo: BTreeMap<Arc<BTreeSet<usize>>, Arc<View<K>>>,
}

impl<K: Clone + Ord> Default for Engine<K> {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            keys: BTreeMap::new(),
            info: BTreeMap::new(),
            origins: BTreeMap::new(),
            memo: BTreeMap::new(),
        }
    }
}

impl<K: Clone + Ord> Engine<K> {
    /// An engine that has judged nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// How commit `id` was judged, if it was.
    pub fn outcome(&self, id: &str) -> Option<&Outcome> {
        self.info.get(id).map(|i| &i.outcome)
    }

    /// Whether commit `id` was accepted.
    pub fn accepted(&self, id: &str) -> bool {
        matches!(
            self.outcome(id),
            Some(Outcome::Clean | Outcome::Contribution)
        )
    }

    /// The rank a contribution represents its versions with.
    pub fn rank(&self, id: &str) -> Option<&Rank> {
        self.origins.get(id).map(|o| &o.0)
    }

    fn add(&mut self, key: Key<K>, event: Event<K>) -> usize {
        if let Some(&i) = self.keys.get(&key) {
            return i;
        }
        self.events.push(event);
        self.keys.insert(key, self.events.len() - 1);
        self.events.len() - 1
    }

    fn origin(&mut self, meta: &Meta, state: &BTreeSet<usize>) {
        let seq = state
            .iter()
            .filter_map(|&e| self.events[e].commit.as_ref())
            .filter(|c| {
                self.origins
                    .get(*c)
                    .is_some_and(|o| o.0.device == meta.device)
            })
            .count();
        let rank = Rank {
            when: meta.when.utc,
            device: meta.device.clone(),
            seq: u32::try_from(seq + 1).unwrap_or(u32::MAX),
            id: meta.id.clone(),
        };
        let origin = (rank, meta.who.clone(), meta.when, meta.person.clone());
        self.origins.insert(meta.id.clone(), origin);
    }

    /// Judges the genesis commit (04 §4.1): every unit of its tree is a
    /// version, written by it.
    pub fn genesis(&mut self, meta: &Meta, tree: &Tree<K>) -> Outcome {
        if let Err((p, why)) = names::portable_tree(tree.keys().map(String::as_str)) {
            return self.set(
                &meta.id,
                Outcome::Refused(format!("{p}: {}", why.code())),
                BTreeSet::new(),
            );
        }
        self.origin(meta, &BTreeSet::new());
        let writes = units(tree)
            .into_iter()
            .map(|(x, v)| (x, Some((v, meta.id.clone()))))
            .collect();
        let event = Event {
            commit: Some(meta.id.clone()),
            past: BTreeSet::new(),
            writes,
            resolves: BTreeSet::new(),
            frozen: None,
            renames: BTreeMap::new(),
        };
        let e = self.add(Key::Commit(meta.id.clone()), event);
        self.set(&meta.id, Outcome::Contribution, BTreeSet::from([e]))
    }

    fn set(&mut self, id: &str, outcome: Outcome, state: BTreeSet<usize>) -> Outcome {
        let info = Info {
            outcome: outcome.clone(),
            state: Arc::new(state),
        };
        self.info.insert(id.to_owned(), info);
        outcome
    }

    /// The state of the accepted commits among `ids`: the union of their
    /// events, or `None` when none of them is accepted.
    pub fn state(&self, ids: &[String]) -> Option<BTreeSet<usize>> {
        let acc: Vec<&Info> = ids
            .iter()
            .filter_map(|p| self.info.get(p))
            .filter(|i| matches!(i.outcome, Outcome::Clean | Outcome::Contribution))
            .collect();
        if acc.is_empty() {
            return None;
        }
        Some(acc.iter().flat_map(|i| i.state.iter().copied()).collect())
    }

    /// The catalogue of the accepted commits among `ids` (05 §5.2): B(c)
    /// for a commit's parents, a device's view for its main's ancestry.
    pub fn catalogue(&mut self, ids: &[String]) -> Arc<View<K>> {
        let state = self.state(ids).unwrap_or_default();
        self.present(state)
    }

    /// Judges commit `meta` with tree `tree` by the folder rules alone, as
    /// the oracle does (04 §4.4 with 05 §5.1): its parents must have been
    /// judged; `may(path, added)` says whether its person may add (`added`)
    /// or change the unit at `path` (04 §4.3). A clean merge is accepted
    /// whoever signs it; a change its person may not make is a proposal.
    pub fn record(
        &mut self,
        meta: &Meta,
        tree: &Tree<K>,
        may: &dyn Fn(&str, bool) -> bool,
    ) -> Outcome {
        if meta.parents.is_empty() {
            return self.genesis(meta, tree);
        }
        let Some(view) = self.base(&meta.parents) else {
            return self.admit(meta, tree, Judgement::Accepted);
        };
        let (old, new) = (units(&view.tree), units(tree));
        let delta = slot_delta(&old, &new);
        let mut meta = meta.clone();
        meta.resolves
            .retain(|p| old.contains_key(&names::fold(p)) && may(p, false));
        if delta.is_empty() && meta.resolves.is_empty() {
            return self.admit(&meta, tree, Judgement::Accepted);
        }
        if let Err((p, why)) = names::portable_tree(tree.keys().map(String::as_str)) {
            let why = Judgement::Refused(format!("{p}: {}", why.code()));
            return self.admit(&meta, tree, why);
        }
        let allowed = delta.iter().all(|x| {
            let v = new.get(*x).or_else(|| old.get(*x));
            v.is_some_and(|v| may(&v.path, !old.contains_key(*x)))
        });
        let judgement = if allowed {
            Judgement::Accepted
        } else {
            Judgement::Proposal
        };
        self.admit(&meta, tree, judgement)
    }

    /// B(c) for a commit with these parents: the catalogue of the accepted
    /// ones, or `None` when none is (rule 1 of 04 §4.4).
    pub fn base(&mut self, parents: &[String]) -> Option<Arc<View<K>>> {
        let state = self.state(parents)?;
        Some(self.present(state))
    }

    /// Records commit `meta`, which the caller judged against [`base`]
    /// (04 §4.4): an accepted commit is clean when its tree is B(c)'s and
    /// it resolves nothing, else a contribution, which freezes every
    /// unsettled slot of B(c) (05 §5.2 step 6). `meta.resolves` must hold
    /// only paths its person may change; those B(c) lacks count for
    /// nothing. Its parents must have been recorded; a commit with none
    /// accepted is refused whatever the judgement.
    ///
    /// [`base`]: Engine::base
    pub fn admit(&mut self, meta: &Meta, tree: &Tree<K>, judgement: Judgement) -> Outcome {
        if meta.parents.is_empty() {
            return match judgement {
                Judgement::Accepted => self.genesis(meta, tree),
                Judgement::Proposal => self.set(&meta.id, Outcome::Proposal, BTreeSet::new()),
                Judgement::Refused(why) => {
                    self.set(&meta.id, Outcome::Refused(why), BTreeSet::new())
                }
            };
        }
        let Some(state) = self.state(&meta.parents) else {
            let why = Outcome::Refused("no-accepted-parent".to_owned());
            return self.set(&meta.id, why, BTreeSet::new());
        };
        match judgement {
            Judgement::Accepted => {}
            Judgement::Proposal => return self.set(&meta.id, Outcome::Proposal, state),
            Judgement::Refused(why) => return self.set(&meta.id, Outcome::Refused(why), state),
        }
        let view = self.present(state.clone());
        let (old, new) = (units(&view.tree), units(tree));
        let delta = slot_delta(&old, &new);
        let resolves: BTreeSet<String> = meta
            .resolves
            .iter()
            .map(|p| names::fold(p))
            .filter(|x| old.contains_key(x))
            .collect();
        if delta.is_empty() && resolves.is_empty() {
            return self.set(&meta.id, Outcome::Clean, state);
        }
        let frozen: BTreeSet<usize> = view
            .unsettled
            .iter()
            .map(|x| self.freeze(x, &view))
            .collect();
        let writes = delta
            .iter()
            .map(|x| {
                (
                    (*x).clone(),
                    new.get(*x).map(|v| (v.clone(), meta.id.clone())),
                )
            })
            .collect();
        let past: BTreeSet<usize> = state.union(&frozen).copied().collect();
        self.origin(meta, &past);
        // A rename counts when it deletes a file of the base and adds one the
        // base lacks, packages aside; it is pure when the content did not
        // change (ADR 0072). It keeps the versions it moved: the events that
        // head the old slot with a version, a pure rename standing for the
        // versions it moved in turn.
        let moved = |x: &str| -> BTreeSet<usize> {
            let hs = view.heads.get(x).into_iter().flatten().copied();
            hs.filter(|&h| matches!(self.writes_of(&view.eff, h).get(x), Some(Some(_))))
                .flat_map(|h| {
                    let mut rs = self.events[h].renames.values();
                    rs.find(|(y, pure, _)| y == x && *pure)
                        .map_or_else(|| BTreeSet::from([h]), |r| r.2.clone())
                })
                .collect()
        };
        let renames = meta
            .renames
            .iter()
            .filter(|(a, b)| names::unit(a) == a && names::unit(b) == b)
            .map(|(a, b)| (names::fold(a), names::fold(b)))
            .filter_map(|(x, y)| {
                let (from, to) = (old.get(&x)?, new.get(&y)?);
                let files = matches!(
                    (&from.content, &to.content),
                    (Content::File(_), Content::File(_))
                );
                let ok = files && !new.contains_key(&x) && !old.contains_key(&y);
                ok.then(|| (moved(&x), (y, from.content == to.content)))
                    .map(|(seen, (y, pure))| (x, (y, pure, seen)))
            })
            .collect();
        let event = Event {
            commit: Some(meta.id.clone()),
            past: past.clone(),
            writes,
            resolves,
            frozen: None,
            renames,
        };
        let e = self.add(Key::Commit(meta.id.clone()), event);
        let mut out = past;
        out.insert(e);
        self.set(&meta.id, Outcome::Contribution, out)
    }

    /// The conflict event that keeps slot `x`'s versions where `view`
    /// showed them (05 §5.2 step 6).
    fn freeze(&mut self, x: &str, view: &View<K>) -> usize {
        let shown = view.shown.get(x).map(Vec::as_slice).unwrap_or_default();
        let mut writes: BTreeMap<String, Write<K>> = shown
            .iter()
            .map(|s| {
                let v = Version {
                    path: s.path.clone(),
                    content: s.version.content.clone(),
                };
                (names::fold(&s.path), Some((v, s.origin.clone())))
            })
            .collect();
        writes.entry(x.to_owned()).or_insert(None);
        // It saw the heads, and the renames that moved them here (ADR 0072).
        let mut heads: BTreeSet<usize> = BTreeSet::new();
        for &h in view.heads.get(x).into_iter().flatten() {
            heads.insert(h);
            if let Some((used, _)) = view.eff.via.get(&(h, x.to_owned())) {
                heads.extend(used.iter().copied());
            }
        }
        let values = writes
            .iter()
            .map(|(y, w)| (y.clone(), w.as_ref().map(|w| w.0.clone())))
            .collect();
        let key = Key::Frozen(x.to_owned(), heads.clone(), values);
        if let Some(&i) = self.keys.get(&key) {
            return i;
        }
        let mut past = BTreeSet::new();
        for &h in &heads {
            past.insert(h);
            past.extend(self.events[h].past.iter().copied());
        }
        let frozen = Some((x.to_owned(), conflicted(shown)));
        let event = Event {
            commit: None,
            past,
            writes,
            resolves: BTreeSet::new(),
            frozen,
            renames: BTreeMap::new(),
        };
        self.add(key, event)
    }

    fn present(&mut self, state: BTreeSet<usize>) -> Arc<View<K>> {
        if let Some(v) = self.memo.get(&state) {
            return Arc::clone(v);
        }
        let view = Arc::new(self.compute(&state));
        if self.memo.len() >= MEMO {
            self.memo.clear(); // a cache: dropping it changes no result
        }
        self.memo.insert(Arc::new(state), Arc::clone(&view));
        view
    }

    fn order(&self, origin: &str) -> Rank {
        self.origins
            .get(origin)
            .map(|o| o.0.clone())
            .unwrap_or_else(|| Rank {
                when: i64::MAX,
                device: String::new(),
                seq: 0,
                id: origin.to_owned(),
            })
    }

    /// An event's writes as `eff` moved them, else its own.
    fn writes_of<'a>(&'a self, eff: &'a Effective<K>, e: usize) -> &'a BTreeMap<String, Write<K>> {
        eff.writes.get(&e).unwrap_or(&self.events[e].writes)
    }

    fn compute(&self, state: &BTreeSet<usize>) -> View<K> {
        let mut eff = self.effective(state);
        let heads = self.heads(state, &mut eff);
        let mut versions: Vec<(Rank, String, Version<K>, String)> = Vec::new();
        for (x, hs) in &heads {
            let mut best: BTreeMap<&Content<K>, (Rank, &Version<K>, &str)> = BTreeMap::new();
            for &e in hs {
                if let Some(Some((v, o))) = self.writes_of(&eff, e).get(x) {
                    let r = self.order(o);
                    let better = best.get(&v.content).is_none_or(|b| (&r, v) < (&b.0, b.1));
                    if better {
                        best.insert(&v.content, (r, v, o));
                    }
                }
            }
            versions.extend(
                best.into_values()
                    .map(|(r, v, o)| (r, x.clone(), v.clone(), o.to_owned())),
            );
        }
        versions.sort();
        let mut spell: BTreeMap<String, String> = BTreeMap::new();
        let mut folders: BTreeSet<String> = BTreeSet::new();
        for (_, x, v, _) in &versions {
            let parts: Vec<&str> = v.path.split('/').collect();
            for i in 0..parts.len().saturating_sub(1) {
                spell
                    .entry(names::fold(&parts[..=i].join("/")))
                    .or_insert_with(|| parts[i].to_owned());
            }
            if matches!(v.content, Content::Package(_)) {
                folders.insert(x.clone());
            }
        }
        folders.extend(spell.keys().cloned());
        let respell = |path: &str| {
            let parts: Vec<&str> = path.split('/').collect();
            let mut out: Vec<String> = (0..parts.len() - 1)
                .map(|i| {
                    spell
                        .get(&names::fold(&parts[..=i].join("/")))
                        .cloned()
                        .unwrap_or_else(|| parts[i].to_owned())
                })
                .collect();
            out.push(parts[parts.len() - 1].to_owned());
            out.join("/")
        };
        // The versions a conflict event placed at one of their slot's copies:
        // that copy holds them, or what a person who saw them saved there.
        // The slot is where the frozen one went, if a rename moved it.
        let copied: BTreeSet<(&str, &str)> = state
            .iter()
            .filter_map(|&e| {
                let ev = &self.events[e];
                let x = ev.frozen.as_ref()?.0.as_str();
                Some((ev, x, eff.fslot.get(&e).map_or(x, String::as_str)))
            })
            .flat_map(|(ev, x, at)| {
                ev.writes
                    .iter()
                    .filter(move |(y, _)| y.as_str() != x)
                    .filter_map(move |(_, w)| Some((at, w.as_ref()?.1.as_str())))
            })
            .collect();
        let mut placed: BTreeSet<String> = BTreeSet::new();
        let mut shown: BTreeMap<String, Vec<Shown<K>>> = BTreeMap::new();
        let mut later = Vec::new();
        for (_, x, v, o) in versions {
            let path = respell(&v.path);
            if placed.contains(&x)
                || (folders.contains(&x) && !matches!(v.content, Content::Package(_)))
            {
                later.push((x, v, o, path));
            } else {
                placed.insert(x.clone());
                shown.entry(x).or_default().push(Shown {
                    version: v,
                    path,
                    origin: o,
                });
            }
        }
        for (x, v, o, path) in later {
            if crate::authority::is_pigeon_path(&x) {
                continue; // pigeon's own files keep their first version (05 §5.2)
            }
            if copied.contains(&(x.as_str(), o.as_str())) {
                continue; // shown at the copy a conflict event gave it (05 §5.2)
            }
            let (who, when) = self
                .origins
                .get(&o)
                .map(|o| (o.1.as_str(), o.2))
                .unwrap_or(("-", Stamp { utc: 0, offset: 0 }));
            let mut n = 1;
            let mut name = names::conflict_name(&path, who, when, n);
            while placed.contains(&names::fold(&name)) || folders.contains(&names::fold(&name)) {
                n += 1;
                name = names::conflict_name(&path, who, when, n);
            }
            placed.insert(names::fold(&name));
            shown.entry(x).or_default().push(Shown {
                version: v,
                path: name,
                origin: o,
            });
        }
        let unsettled = shown
            .iter()
            .filter(|(_, s)| s.len() > 1 || s.first().is_some_and(|f| f.path != f.version.path))
            .map(|(x, _)| x.clone())
            .collect();
        let tree = expand(
            shown
                .values()
                .flatten()
                .map(|s| (s.path.as_str(), &s.version.content)),
        );
        let writers = self.writers(state, &eff);
        View {
            tree,
            shown,
            unsettled,
            writers,
            heads,
            eff,
        }
    }

    /// The persons who wrote each slot in `state`, renames followed: a
    /// write's version's author, a deletion's contribution's.
    fn writers(
        &self,
        state: &BTreeSet<usize>,
        eff: &Effective<K>,
    ) -> BTreeMap<String, BTreeSet<PersonId>> {
        let mut out: BTreeMap<String, BTreeSet<PersonId>> = BTreeMap::new();
        for &e in state {
            let ev = &self.events[e];
            for (x, w) in self.writes_of(eff, e) {
                let origin = match w {
                    Some((_, o)) => Some(o),
                    None => ev.commit.as_ref(),
                };
                if let Some(o) = origin.and_then(|o| self.origins.get(o)) {
                    out.entry(x.clone()).or_default().insert(o.3.clone());
                }
            }
        }
        out
    }

    /// The open conflicts of the accepted commits among `ids` (05 §5.3): a
    /// slot unsettled now, or frozen and not yet written, nor resolved, by
    /// a contribution that saw it.
    pub fn open(&mut self, ids: &[String]) -> BTreeSet<String> {
        let state = self.state(ids).unwrap_or_default();
        let view = self.present(state.clone());
        let mut out: BTreeSet<String> = view
            .unsettled
            .iter()
            .filter(|x| view.shown.get(*x).is_some_and(|s| conflicted(s)))
            .cloned()
            .collect();
        let eff = &view.eff;
        let slots = |e: usize| {
            self.writes_of(eff, e)
                .keys()
                .filter(move |y| !eff.yielded.contains(&(e, (*y).clone())))
        };
        for &f in &state {
            let Some((x, true)) = &self.events[f].frozen else {
                continue;
            };
            let x = eff.fslot.get(&f).unwrap_or(x);
            let frozen: BTreeSet<&String> = slots(f).collect();
            let closed = state.iter().any(|&e| {
                let c = &self.events[e];
                c.commit.is_some()
                    && c.past.contains(&f)
                    && (c.resolves.contains(x) || slots(e).any(|y| frozen.contains(y)))
            });
            if !closed {
                out.insert(x.clone());
            }
        }
        out
    }

    /// Every version a conflict event placed, with the commit it represents
    /// and the path that commit wrote it at: the `authority` law asks that
    /// each be allowed to it where it is, or, if a rename moved it, where
    /// it was written (ADR 0072).
    pub fn copies(&self) -> Vec<(String, String, String)> {
        let wrote = |v: &Version<K>, o: &String| {
            let e = self.keys.get(&Key::Commit(o.clone()))?;
            let mut ws = self.events[*e].writes.values().flatten();
            ws.find(|w| w.0.content == v.content)
                .map(|w| w.0.path.clone())
        };
        self.events
            .iter()
            .filter(|e| e.frozen.is_some())
            .flat_map(|e| e.writes.values().flatten())
            .map(|(v, o)| {
                let at = wrote(v, o).unwrap_or_else(|| v.path.clone());
                (v.path.clone(), o.clone(), at)
            })
            .collect()
    }
}

/// Several versions, or one under another name: a conflict people are
/// told of (05 §5.3).
fn conflicted<K>(shown: &[Shown<K>]) -> bool {
    let last = |p: &str| p.rsplit('/').next().unwrap_or_default().to_owned();
    shown.len() > 1
        || shown
            .first()
            .is_some_and(|s| last(&s.path) != last(&s.version.path))
}
