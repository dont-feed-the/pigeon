//! The loops of the model (08 §8.1): the supervisor, capture, place,
//! history, observe, free and keep, run in turns to a fixed point, and
//! recovery from the install journal (07 §7.5) after a crash.

use std::collections::BTreeMap;

use pigeon_core::capacity;
use pigeon_core::filter::Role;
use pigeon_core::journal::{self, Begin, End, Found, Record, Stat, Step as JStep};
use pigeon_core::state::State;
use pigeon_core::time::Time;

use super::{Daemon, Model, OnDisk, Ver, View};
use crate::step::CrashPoint;
use crate::world::{Device, File, Folder, Key, Laptop};

/// Crosses `point`: whether the armed plan crashes the daemon here.
fn cross(d: &mut Daemon, point: CrashPoint) -> bool {
    match d.plan {
        Some((p, n)) if p == point => {
            if n <= 1 {
                d.plan = None;
                d.running = false;
                d.crashed = true;
                true
            } else {
                d.plan = Some((p, n - 1));
                false
            }
        }
        _ => false,
    }
}

fn path_of(f: &File) -> String {
    format!("{}/{}", f.folder.0, f.name)
}

fn file_of(path: &str) -> Option<File> {
    let (folder, name) = path.split_once('/')?;
    Some(Folder(folder.parse().ok()?).file(name))
}

impl Model {
    /// The model with times erased, for detecting a fixed point.
    fn timeless(&self) -> Model {
        let mut m = self.clone();
        m.now = Time(0);
        for l in &mut m.laptops {
            for v in &mut l.views {
                v.heard = v.heard.map(|_| Time(0));
            }
        }
        m.trace.clear();
        m
    }

    /// Runs turns of the loops until one changes nothing but the time (at
    /// most 500). A turn that changes nothing is not kept, so settling a
    /// settled model leaves it as it is.
    pub fn settle(&mut self) {
        for _ in 0..500 {
            let mut next = self.clone();
            next.turn();
            if next.timeless() == self.timeless() {
                return;
            }
            *self = next;
        }
    }

    /// One turn of every loop.
    fn turn(&mut self) {
        self.now = Time(self.now.0 + 1);
        self.supervise();
        for i in 0..self.laptops.len() {
            self.capture(Laptop(i as u8));
        }
        self.place();
        self.learn();
        self.observe();
        for i in 0..self.laptops.len() {
            let l = Laptop(i as u8);
            self.free(l);
            self.keep(l);
        }
    }

    /// The supervisor restarts a crashed daemon, which recovers first.
    fn supervise(&mut self) {
        for i in 0..self.laptops.len() {
            let d = &mut self.laptops[i].daemon;
            if d.crashed {
                d.crashed = false;
                d.running = true;
                self.recover(Laptop(i as u8));
                self.log("CaptureInstall", "Crash", Device::Laptop(Laptop(i as u8)));
            }
        }
        for c in &mut self.custodians {
            if c.daemon.crashed {
                c.daemon.crashed = false;
                c.daemon.running = true;
            }
        }
    }

    fn up(&self, l: Laptop) -> bool {
        let lm = &self.laptops[l.0 as usize];
        lm.present && lm.daemon.running
    }

    /// Capture: each regular file whose bytes are not what the laptop last
    /// captured or installed there. A change of a file that exists, which
    /// the person may not make, is a suggestion instead (04 §4.5).
    fn capture(&mut self, l: Laptop) {
        if !self.up(l) {
            return;
        }
        let i = l.0 as usize;
        let files: Vec<(File, Key)> = self.laptops[i]
            .disk
            .iter()
            .filter_map(|(f, d)| match d {
                OnDisk::File(k) => Some((f.clone(), k.clone())),
                OnDisk::Link => None,
            })
            .collect();
        let may_change = self.may_change(l);
        for (f, k) in files {
            let lm = &mut self.laptops[i];
            let current = lm.history.get(&f).and_then(|v| v.last()).map(|v| &v.key);
            if lm.synced.get(&f) == Some(&k) || current == Some(&k) {
                lm.synced.insert(f, k);
                continue;
            }
            if cross(&mut lm.daemon, CrashPoint::CaptureBeforeRef) {
                return;
            }
            let len = lm.content.get(&k).copied().unwrap_or(0);
            lm.synced.insert(f.clone(), k.clone());
            let ver = Ver {
                key: k,
                len,
                saved: self.now,
            };
            if !may_change && lm.history.get(&f).is_some_and(|v| !v.is_empty()) {
                self.suggest(l, f, ver);
                continue;
            }
            lm.history.entry(f.clone()).or_default().push(ver.clone());
            let n = lm.history[&f].len();
            let group = self.group.entry(f).or_default();
            group.truncate(n - 1);
            group.push(ver);
            self.log("CaptureInstall", "Capture", Device::Laptop(l));
        }
    }

    /// Place and repair: running custodians take every version's commit
    /// from running laptops and each other, then its content, if it fits
    /// in the room above the reserve; a key that does not waits, and the
    /// custodian says `no-capacity`.
    fn place(&mut self) {
        let mut known: BTreeMap<File, usize> = BTreeMap::new();
        let mut sources: BTreeMap<Key, u64> = BTreeMap::new();
        let mut forged = false;
        for (i, lm) in self.laptops.iter().enumerate() {
            if self.up(Laptop(i as u8)) && lm.forged {
                // Accept stops at the forged commit (04 §4.4): no later
                // history of this laptop reaches the custodians.
                forged = true;
                sources.extend(lm.content.iter().map(|(k, n)| (k.clone(), *n)));
            } else if self.up(Laptop(i as u8)) {
                for (f, v) in &lm.history {
                    let n = known.entry(f.clone()).or_default();
                    *n = (*n).max(v.len());
                }
                sources.extend(lm.content.iter().map(|(k, n)| (k.clone(), *n)));
            }
        }
        for c in &self.custodians {
            if c.present && c.daemon.running && !c.lost {
                for (f, n) in &c.known {
                    let m = known.entry(f.clone()).or_default();
                    *m = (*m).max(*n);
                }
                sources.extend(c.holds.iter().map(|(k, n)| (k.clone(), *n)));
            }
        }
        for c in &mut self.custodians {
            if !(c.present && c.daemon.running && !c.lost) {
                continue;
            }
            if forged {
                c.says.insert("commit-unknown-signer".into());
            }
            for (f, n) in &known {
                let m = c.known.entry(f.clone()).or_default();
                *m = (*m).max(*n);
            }
            for (f, n) in &c.known {
                for ver in self
                    .group
                    .get(f)
                    .map(|v| &v[..(*n).min(v.len())])
                    .unwrap_or(&[])
                {
                    if c.holds.contains_key(&ver.key) || !sources.contains_key(&ver.key) {
                        continue;
                    }
                    // Admission (08 §8.9): a key fits in the room above
                    // the reserve; below the reserve the disk is full.
                    let used: u64 = c.holds.values().sum::<u64>() + c.ballast;
                    let reserve = capacity::reserve(Role::Custodian, c.disk_bytes);
                    let free = c.disk_bytes.saturating_sub(used);
                    if free < reserve {
                        c.says.insert("disk-full".into());
                    }
                    if ver.len > free.saturating_sub(reserve) {
                        c.says.insert("no-capacity".into());
                        continue;
                    }
                    c.holds.insert(ver.key.clone(), ver.len);
                }
            }
        }
    }

    /// A laptop learns the group's history from running custodians.
    fn learn(&mut self) {
        for i in 0..self.laptops.len() {
            if !self.up(Laptop(i as u8)) {
                continue;
            }
            for c in &self.custodians {
                if !(c.present && c.daemon.running && !c.lost) {
                    continue;
                }
                for (f, n) in &c.known {
                    let have = self.laptops[i].history.get(f).map_or(0, Vec::len);
                    if have < *n {
                        let vers = self.group.get(f).map(|v| v[..(*n).min(v.len())].to_vec());
                        self.laptops[i]
                            .history
                            .insert(f.clone(), vers.unwrap_or_default());
                    }
                }
            }
        }
    }

    /// Status reports (06 §6.3): a running laptop hears every running
    /// custodian, and says what its report says needs a person.
    fn observe(&mut self) {
        for i in 0..self.laptops.len() {
            if !self.up(Laptop(i as u8)) {
                continue;
            }
            for (j, c) in self.custodians.iter().enumerate() {
                if c.present && c.daemon.running {
                    self.laptops[i].views[j] = View {
                        heard: Some(self.now),
                        holds: c.holds.keys().cloned().collect(),
                        known: c.known.clone(),
                        lost: c.lost,
                        says: c
                            .says
                            .iter()
                            .filter(|s| ["disk-full", "no-capacity"].contains(&s.as_str()))
                            .cloned()
                            .collect(),
                    };
                }
            }
        }
    }

    /// Free: each protected file of a freed folder is hidden, then its
    /// content dropped (08 §8.5); the others wait until they are protected.
    fn free(&mut self, l: Laptop) {
        let i = l.0 as usize;
        if !self.up(l) {
            return;
        }
        let lm = &self.laptops[i];
        let todo: Vec<(File, Key)> = lm
            .disk
            .iter()
            .filter(|(f, _)| lm.freed.contains(&f.folder))
            .filter_map(|(f, d)| match d {
                OnDisk::File(k) => Some((f.clone(), k.clone())),
                OnDisk::Link => None,
            })
            .filter(|(f, k)| lm.history.get(f).and_then(|v| v.last()).map(|v| &v.key) == Some(k))
            .filter(|(f, _)| matches!(self.state_of(l, f), Some(State::Protected(_))))
            .collect();
        for (f, k) in todo {
            if !self.hide(l, &f, &k) {
                return;
            }
        }
    }

    fn hide(&mut self, l: Laptop, f: &File, k: &Key) -> bool {
        let seq = self.seq;
        self.seq += 1;
        let aside = format!("aside-{seq}");
        let lm = &mut self.laptops[l.0 as usize];
        lm.journal.push(Record::Begin(Begin {
            seq,
            path: path_of(f),
            old_key: Some(k.0.clone()),
            new_key: None,
            stat: Some(Stat::default()),
            stage: None,
            aside: aside.clone(),
        }));
        if cross(&mut lm.daemon, CrashPoint::FreeAfterBegin) {
            return false;
        }
        lm.disk.remove(f);
        lm.asides.insert(aside.clone(), k.clone());
        if cross(&mut lm.daemon, CrashPoint::FreeAfterMoveAside) {
            return false;
        }
        lm.journal.push(Record::Proved(vec![seq]));
        if cross(&mut lm.daemon, CrashPoint::FreeAfterProved) {
            return false;
        }
        if cross(&mut lm.daemon, CrashPoint::FreeAfterIndex) {
            return false;
        }
        lm.journal.push(Record::End(seq, End::Done));
        self.log("CaptureInstall", "Done", Device::Laptop(l));
        self.release(l);
        true
    }

    /// Keep: each file of a kept folder that is absent, or unchanged since
    /// the laptop last captured or installed it and older than its current
    /// version, is fetched, then installed through the journal. A file that
    /// holds its current version is in step again.
    fn keep(&mut self, l: Laptop) {
        let i = l.0 as usize;
        if !self.up(l) {
            return;
        }
        let lm = &mut self.laptops[i];
        let current: Vec<(File, Key)> = lm
            .history
            .iter()
            .filter_map(|(f, v)| Some((f.clone(), v.last()?.key.clone())))
            .filter(|(f, k)| lm.disk.get(f) == Some(&OnDisk::File(k.clone())))
            .collect();
        for (f, k) in current {
            lm.diverged.remove(&f);
            lm.synced.insert(f, k);
        }
        let lm = &self.laptops[i];
        let todo: Vec<(File, Ver)> = lm
            .history
            .iter()
            .filter(|(f, _)| !lm.freed.contains(&f.folder) && !lm.diverged.contains(*f))
            .filter_map(|(f, v)| v.last().map(|v| (f.clone(), v.clone())))
            .filter(|(f, v)| match lm.disk.get(f) {
                None => true,
                Some(OnDisk::File(k)) => *k != v.key && lm.synced.get(f) == Some(k),
                Some(OnDisk::Link) => false,
            })
            .collect();
        for (f, ver) in todo {
            let reachable = self.laptops[i].content.contains_key(&ver.key)
                || self
                    .custodians
                    .iter()
                    .any(|c| c.present && c.daemon.running && c.holds.contains_key(&ver.key));
            if !reachable {
                continue;
            }
            if !self.install(l, &f, &ver) {
                return;
            }
        }
    }

    fn install(&mut self, l: Laptop, f: &File, ver: &Ver) -> bool {
        let seq = self.seq;
        self.seq += 1;
        let lm = &mut self.laptops[l.0 as usize];
        if !lm.content.contains_key(&ver.key) {
            if lm.used_with(ver.len) > lm.disk_bytes {
                lm.says.insert("disk-full".into());
                return true;
            }
            lm.content.insert(ver.key.clone(), ver.len);
        }
        let stage = format!("stage-{seq}");
        lm.stages.insert(stage.clone(), ver.key.clone());
        lm.journal.push(Record::Begin(Begin {
            seq,
            path: path_of(f),
            old_key: None,
            new_key: Some(ver.key.0.clone()),
            stat: None,
            stage: Some(stage.clone()),
            aside: format!("aside-{seq}"),
        }));
        if cross(&mut lm.daemon, CrashPoint::KeepAfterBegin) {
            return false;
        }
        lm.journal.push(Record::Proved(vec![seq]));
        lm.disk.insert(f.clone(), OnDisk::File(ver.key.clone()));
        lm.synced.insert(f.clone(), ver.key.clone());
        lm.stages.remove(&stage);
        if cross(&mut lm.daemon, CrashPoint::KeepAfterRenameIn) {
            return false;
        }
        lm.journal.push(Record::End(seq, End::Done));
        self.log("CaptureInstall", "Done", Device::Laptop(l));
        self.release(l);
        true
    }

    /// Releases the asides of ended entries, dropping a hidden file's
    /// content unless a path still uses it, and truncates the journal.
    fn release(&mut self, l: Laptop) {
        let lm = &mut self.laptops[l.0 as usize];
        let entries = journal::entries(&lm.journal);
        for (seq, e) in &entries {
            if e.end.is_none() || e.released {
                continue;
            }
            if let Some(k) = lm.asides.remove(&e.begin.aside) {
                let used = lm.disk.values().any(|d| *d == OnDisk::File(k.clone()));
                if !used && e.begin.new_key.is_none() && e.end == Some(End::Done) {
                    lm.content.remove(&k);
                }
            }
            lm.journal.push(Record::Released(*seq));
        }
        if journal::can_truncate(&journal::entries(&lm.journal)) {
            lm.journal.clear();
        }
    }

    /// Recovery at start: every entry without an end is decided by
    /// [`journal::recover`] from what is on disk; orphan stages go.
    pub(crate) fn recover(&mut self, l: Laptop) {
        let lm = &mut self.laptops[l.0 as usize];
        let entries = journal::entries(&lm.journal);
        for (seq, e) in &entries {
            let Some(f) = file_of(&e.begin.path) else {
                continue;
            };
            let stage = e.begin.stage.clone().unwrap_or_default();
            let found = Found {
                path: lm.disk.contains_key(&f),
                aside: lm.asides.contains_key(&e.begin.aside),
                stage: lm.stages.contains_key(&stage),
                aside_is_old: lm.asides.get(&e.begin.aside).map(|k| &k.0)
                    == e.begin.old_key.as_ref(),
            };
            for step in journal::recover(e, found) {
                match step {
                    JStep::RenameIn => {
                        if let Some(k) = lm.stages.remove(&stage) {
                            lm.disk.insert(f.clone(), OnDisk::File(k));
                        }
                    }
                    JStep::Restage => {}
                    JStep::PutAsideBack => {
                        if let Some(k) = lm.asides.remove(&e.begin.aside) {
                            lm.disk.insert(f.clone(), OnDisk::File(k));
                        }
                        lm.journal.push(Record::Released(*seq));
                    }
                    JStep::Append(end) => lm.journal.push(Record::End(*seq, end)),
                    JStep::RemoveStage => {
                        lm.stages.remove(&stage);
                    }
                }
            }
        }
        let named: Vec<String> = journal::entries(&lm.journal)
            .values()
            .filter_map(|e| e.begin.stage.clone())
            .collect();
        lm.stages.retain(|s, _| named.contains(s));
        self.release(l);
    }
}
