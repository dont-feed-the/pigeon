//! Renames in the catalogue (ADR 0072, spec 05 §5.2): where each write
//! goes once the renames it did not see are followed, and which writers of
//! each slot nothing replaced. The oracle's `effective` and `heads`
//! (models/catalogue/catalogue.py).

use std::collections::{BTreeMap, BTreeSet};

use super::{Effective, Engine, Rank};

/// A rename: its contribution's rank, the contribution, the old slot and
/// the new.
type Renamed<'a> = (Rank, usize, &'a str, &'a str);

impl<K: Clone + Ord> Engine<K> {
    /// The renames of `state`, in representative order.
    fn renames_of(&self, state: &BTreeSet<usize>) -> Vec<Renamed<'_>> {
        let mut out: Vec<Renamed<'_>> = state
            .iter()
            .flat_map(|&r| {
                let ev = &self.events[r];
                let rank = self.order(ev.commit.as_deref().unwrap_or_default());
                ev.renames
                    .iter()
                    .map(move |(x, (y, ..))| (rank.clone(), r, x.as_str(), y.as_str()))
            })
            .collect();
        out.sort();
        out
    }

    /// Where event `e`'s write of slot `x` goes, with the renames it
    /// follows, among `renames` (those of a past `within`, if given): to
    /// the new slot of the earliest rename of `x` that `e` did not see nor
    /// was seen by, when the two build on one version of `x` (an event both
    /// saw, or a rename the write followed, wrote it), then on from there;
    /// it stops where `e` saw a rename of the slot, or writes the new slot,
    /// or saw an event write it. A rename's deletion of its old slot stays.
    fn target<'a>(
        &'a self,
        renames: &[Renamed<'a>],
        e: usize,
        x: &'a str,
        within: Option<&BTreeSet<usize>>,
    ) -> (&'a str, Vec<usize>) {
        let ev = |i: usize| &self.events[i];
        if renames.is_empty() || ev(e).renames.contains_key(x) {
            return (x, Vec::new());
        }
        let past = &ev(e).past;
        let conc = |r: usize| r != e && !past.contains(&r) && !ev(r).past.contains(&e);
        let saw = |y: &str| past.iter().any(|&p| ev(p).writes.contains_key(y));
        let knew = |x: &str| past.iter().any(|&p| ev(p).renames.contains_key(x));
        let shared = |r: usize, x: &str, used: &[usize]| {
            ev(r)
                .past
                .iter()
                .any(|&w| (past.contains(&w) || used.contains(&w)) && ev(w).writes.contains_key(x))
        };
        let (mut at, mut used, mut seen) = (x, Vec::new(), BTreeSet::from([x]));
        while !knew(at) {
            let next = |n: &&Renamed<'a>| {
                n.2 == at
                    && within.is_none_or(|p| p.contains(&n.1))
                    && conc(n.1)
                    && shared(n.1, at, &used)
            };
            let Some(&(_, r, _, y)) = renames.iter().find(next) else {
                break;
            };
            if seen.contains(y) || ev(e).writes.contains_key(y) || saw(y) {
                break;
            }
            seen.insert(y);
            at = y;
            used.push(r);
        }
        (at, used)
    }

    /// The writes of `state`'s events with renames followed (the oracle's
    /// `effective`): each goes to its [`target`](Engine::target).
    pub(super) fn effective(&self, state: &BTreeSet<usize>) -> Effective<K> {
        let mut out = Effective::default();
        let renames = self.renames_of(state);
        if renames.is_empty() {
            return out;
        }
        for &e in state {
            let ev = &self.events[e];
            let mut moved = false;
            let mut writes = BTreeMap::new();
            for (x, w) in &ev.writes {
                let (at, used) = self.target(&renames, e, x, None);
                let mut w = w.clone();
                if let Some(&last) = used.last() {
                    moved = true;
                    if let (Some((v, _)), Some(Some((to, _)))) =
                        (w.as_mut(), self.events[last].writes.get(at))
                    {
                        v.path.clone_from(&to.path);
                    }
                    let followed = (used.into_iter().collect(), x.clone());
                    out.via.insert((e, at.to_owned()), followed);
                }
                if ev.frozen.as_ref().is_some_and(|f| f.0 == *x) {
                    out.fslot.insert(e, at.to_owned());
                }
                writes.insert(at.to_owned(), w);
            }
            if moved {
                out.writes.insert(e, writes);
            }
        }
        out
    }

    /// Step 2 with renames (the oracle's `heads`): the writers of each slot
    /// that nothing replaced. An event replaces a write it saw if it wrote
    /// the slot where that write was in its own view, where the renames it
    /// saw took it: so a version people saw and changed never comes back,
    /// wherever later renames take it. Then a pure rename's write of its
    /// new slot yields to a version that followed it there and saw every
    /// version it moved, so that a concurrent edit wins over a mere move
    /// and loses nothing.
    pub(super) fn heads(
        &self,
        state: &BTreeSet<usize>,
        eff: &mut Effective<K>,
    ) -> BTreeMap<String, Vec<usize>> {
        let renames = self.renames_of(state);
        let mut writers: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut raw: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for &e in state {
            for x in self.writes_of(eff, e).keys() {
                writers.entry(x.clone()).or_default().push(e);
            }
            for x in self.events[e].writes.keys() {
                raw.entry(x.as_str()).or_default().push(e);
            }
        }
        // The events that wrote a slot a write of `s` may reach by renames.
        let by = |s: &str| -> Vec<usize> {
            let mut slots = vec![s.to_owned()];
            let mut i = 0;
            while i < slots.len() {
                for n in &renames {
                    if n.2 == slots[i] && !slots.iter().any(|t| t == n.3) {
                        slots.push(n.3.to_owned());
                    }
                }
                i += 1;
            }
            let at = |t: &String| raw.get(t.as_str()).into_iter().flatten().copied();
            slots.iter().flat_map(at).collect()
        };
        // w saw e's write of slot s, and wrote where it saw it.
        let over = |w: usize, e: usize, s: &str| {
            let ev = &self.events[w];
            ev.past.contains(&e)
                && (ev.writes).contains_key(self.target(&renames, e, s, Some(&ev.past)).0)
        };
        let none = (BTreeSet::new(), String::new());
        let via = |e: usize, x: &str| eff.via.get(&(e, x.to_owned())).unwrap_or(&none);
        let heads_of = |ws: &[usize], x: &str| -> Vec<usize> {
            let kept = |e: &usize| {
                let s = eff.via.get(&(*e, x.to_owned())).map_or(x, |v| v.1.as_str());
                !by(s).into_iter().any(|w| w != *e && over(w, *e, s))
            };
            ws.iter().copied().filter(kept).collect()
        };
        let mut yielded = BTreeSet::new();
        let mut heads = BTreeMap::new();
        for (x, ws) in &writers {
            let mut hs = heads_of(ws, x);
            // e moved here through the pure rename r and saw every version r moved.
            let yields = |r: usize, e: usize| {
                let saw = |s: &usize| self.events[e].past.contains(s);
                via(e, x).0.contains(&r)
                    && matches!(self.writes_of(eff, e).get(x), Some(Some(_)))
                    && (self.events[r].renames.values())
                        .any(|(y, pure, seen)| y == x && *pure && seen.iter().all(saw))
            };
            let gone: Vec<usize> = hs
                .iter()
                .copied()
                .filter(|&r| hs.iter().any(|&e| yields(r, e)))
                .collect();
            if !gone.is_empty() {
                let rest: Vec<usize> = ws.iter().copied().filter(|e| !gone.contains(e)).collect();
                hs = heads_of(&rest, x);
                yielded.extend(gone.into_iter().map(|e| (e, x.clone())));
            }
            if !hs.is_empty() {
                heads.insert(x.clone(), hs);
            }
        }
        eff.yielded = yielded;
        heads
    }
}
