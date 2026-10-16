//! The model interpreter: the slice as pigeon-core computes it, without
//! processes, disks or time.
//!
//! Devices are values; a [`crate::step::Step::Wait`] runs the loops to a fixed
//! point ([`Model::settle`]) and evaluates its condition there. States come
//! from [`pigeon_core::state::state`], levels from
//! [`pigeon_core::headline::version_level`], install and free run through
//! [`pigeon_core::journal`], whose recovery a crash point exercises. Joins
//! and suggestions follow the default root rule (`people`).
//!
//! Laws (`tests/laws.rs`): a run is a fold, so running `a.then(b)` equals
//! running `b` from where `a` left the model; [`Model::settle`] is
//! idempotent; a crash at any named point, followed by the supervisor's
//! restart, ends where the run without it ends; the model's trace embeds the
//! scenario's projection.

mod loops;
mod people;

pub use people::Suggestion;

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::headline::{Level, version_level};
use pigeon_core::journal::Record;
use pigeon_core::state::{self, Settings, State, Version};
use pigeon_core::time::{Span, Time};

use crate::run::{Interpreter, Measure, Verdict};
use crate::scenario::{Item, Label};
use crate::step::{Condition, CrashPoint, CustodianOp, DeviceOp, Kind, LaptopOp, Outcome, Step};
use crate::world::{Content, Device, File, Folder, Key, Laptop, World};

/// A MiB.
pub const MIB: u64 = 1 << 20;

/// What is at a path of a laptop's folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnDisk {
    /// A regular file with this content.
    File(Key),
    /// A symbolic link, not shared.
    Link,
}

/// One version of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ver {
    /// Its content's key.
    pub key: Key,
    /// Its size.
    pub len: u64,
    /// When it was captured.
    pub saved: Time,
    /// The person who wrote it (ADR 0070).
    pub by: u8,
}

/// What a laptop last heard from one custodian.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// When.
    pub heard: Option<Time>,
    /// The keys it held.
    pub holds: BTreeSet<Key>,
    /// Per file, how many versions' commits it had.
    pub known: BTreeMap<File, usize>,
    /// Its repository was lost.
    pub lost: bool,
    /// What its status report says needs a person (06 §6.3): `disk-full`
    /// or `no-capacity`.
    pub says: BTreeSet<String>,
}

/// A daemon's life.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Daemon {
    /// It runs.
    pub running: bool,
    /// The armed crash plan: the point and the crossings left before it.
    pub plan: Option<(CrashPoint, u32)>,
    /// It crashed; its supervisor restarts it at the next turn.
    pub crashed: bool,
}

/// A laptop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaptopM {
    /// In the group.
    pub present: bool,
    /// Its daemon.
    pub daemon: Daemon,
    /// Its folders' paths.
    pub disk: BTreeMap<File, OnDisk>,
    /// Content it holds, with sizes (a file on disk is its own content).
    pub content: BTreeMap<Key, u64>,
    /// The group's history it knows, per file, oldest first.
    pub history: BTreeMap<File, Vec<Ver>>,
    /// Folders it frees.
    pub freed: BTreeSet<Folder>,
    /// Per custodian, its last report.
    pub views: Vec<View>,
    /// `.git/pigeon/install.ndjson`.
    pub journal: Vec<Record>,
    /// Asides and staged files, by name, with their content.
    pub asides: BTreeMap<String, Key>,
    /// Staged files.
    pub stages: BTreeMap<String, Key>,
    /// Its disk, in bytes, and the ballast filling it.
    pub disk_bytes: u64,
    /// The ballast.
    pub ballast: u64,
    /// It printed a recovery sheet.
    pub sheet: bool,
    /// Its `main` holds a commit signed outside the group, which the
    /// custodians refuse with all that follows it.
    pub forged: bool,
    /// Problem codes it reports.
    pub says: BTreeSet<String>,
    /// Its person is an admin.
    pub admin: bool,
    /// Its person.
    pub person: u8,
    /// Per file, the content it last captured or installed there: a file
    /// on disk that holds it is unchanged.
    pub synced: BTreeMap<File, Key>,
    /// Files whose bytes are the person's own suggestion, which no install
    /// replaces.
    pub diverged: BTreeSet<File>,
}

/// A custodian.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustodianM {
    /// In the group (invited and not removed).
    pub present: bool,
    /// Its daemon.
    pub daemon: Daemon,
    /// Keys it holds, with sizes.
    pub holds: BTreeMap<Key, u64>,
    /// Per file, how many versions' commits it has.
    pub known: BTreeMap<File, usize>,
    /// Its repository was destroyed.
    pub lost: bool,
    /// Its disk, in bytes, and the ballast.
    pub disk_bytes: u64,
    /// The ballast.
    pub ballast: u64,
    /// Problem codes it reports.
    pub says: BTreeSet<String>,
}

/// The whole model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// The present time: one second per turn of the loops.
    pub now: Time,
    /// The custody settings, with the world's K.
    pub settings: Settings,
    /// The laptops.
    pub laptops: Vec<LaptopM>,
    /// The custodians.
    pub custodians: Vec<CustodianM>,
    /// Bodies of the content written, by key.
    pub bodies: BTreeMap<Key, Content>,
    /// The group's history, per file, oldest first; devices know prefixes.
    pub group: BTreeMap<File, Vec<Ver>>,
    /// The next journal sequence number.
    pub seq: u64,
    /// The labels the devices log.
    pub trace: Vec<Label>,
    /// The disk measures.
    pub measures: Vec<Measure>,
    /// The suggestions waiting for a decision, oldest first.
    pub suggestions: Vec<Suggestion>,
    /// The names of devices, for measures.
    names: Vec<String>,
}

/// The grace period before *local* or *published* is *not safe yet*.
pub const GRACE: Span = Span::minutes(10);

impl Model {
    /// The model of `world` at its start: present devices run.
    pub fn new(world: &World) -> Model {
        let views = vec![View::default(); world.custodians.len()];
        let laptops = world
            .laptops
            .iter()
            .enumerate()
            .map(|(i, l)| LaptopM {
                admin: world.is_admin(Laptop(i as u8)),
                person: l.person,
                present: l.present,
                daemon: Daemon {
                    running: l.present,
                    ..Daemon::default()
                },
                views: views.clone(),
                disk_bytes: u64::from(l.disk_mib) * MIB,
                ..LaptopM::default()
            })
            .collect();
        let custodians = world
            .custodians
            .iter()
            .map(|c| CustodianM {
                present: c.present,
                daemon: Daemon {
                    running: c.present,
                    ..Daemon::default()
                },
                disk_bytes: u64::from(c.disk_mib) * MIB,
                ..CustodianM::default()
            })
            .collect();
        Model {
            now: Time(1_000_000),
            settings: Settings {
                k: world.k as usize,
                ..Settings::default()
            },
            laptops,
            custodians,
            bodies: BTreeMap::new(),
            group: BTreeMap::new(),
            seq: 0,
            trace: Vec::new(),
            measures: Vec::new(),
            suggestions: Vec::new(),
            names: world
                .devices()
                .into_iter()
                .map(|d| world.name(d).to_owned())
                .collect(),
        }
    }

    pub(crate) fn log(&mut self, spec: &'static str, action: &'static str, device: Device) {
        self.trace.push(Label {
            spec,
            action,
            device,
        });
    }

    fn laptop(&mut self, l: Laptop) -> &mut LaptopM {
        &mut self.laptops[l.0 as usize]
    }

    /// The pigeon-core view of `file`'s current version from laptop `l`.
    pub fn version(&self, l: Laptop, file: &File) -> Option<(Version<u8>, &Ver)> {
        let lm = &self.laptops[l.0 as usize];
        let vers = lm.history.get(file)?;
        let i = vers.len().checked_sub(1)?;
        let ver = &vers[i];
        let custodians = self
            .custodians
            .iter()
            .enumerate()
            .filter(|(_, c)| c.present)
            .map(|(j, _)| {
                let v = &lm.views[j];
                state::Custodian {
                    device: j as u8,
                    heard: v.heard,
                    holds: v.holds.contains(&ver.key),
                    has_commit: v.known.get(file).copied().unwrap_or(0) > i,
                    checking: false,
                    repository_lost: v.lost,
                }
            })
            .collect();
        let version = Version {
            custodians,
            other_device_has_commit: false,
            this_device_holds: lm.content.contains_key(&ver.key),
            erased: false,
        };
        Some((version, ver))
    }

    /// The state laptop `l` reports for `file`'s current version.
    pub fn state_of(&self, l: Laptop, file: &File) -> Option<State<u8>> {
        self.version(l, file)
            .map(|(v, _)| state::state(&v, &self.settings, self.now))
    }

    /// Laptop `l`'s headline.
    pub fn headline(&self, l: Laptop) -> Level {
        let lm = &self.laptops[l.0 as usize];
        let mut level = Level::Safe;
        if !lm.says.is_empty()
            || lm
                .views
                .iter()
                .zip(&self.custodians)
                .any(|(v, c)| c.present && (v.lost || !v.says.is_empty()))
        {
            level = Level::NeedsYou;
        }
        for file in lm.history.keys() {
            if let Some((v, ver)) = self.version(l, file) {
                let s = state::state(&v, &self.settings, self.now);
                let age = self.now.since(ver.saved);
                level = level.join(version_level(&s, age, GRACE, &self.settings, self.now));
            }
        }
        level
    }

    /// Bytes laptop `l` uses: its content, each key once, and its ballast.
    pub fn used(&self, l: Laptop) -> u64 {
        let lm = &self.laptops[l.0 as usize];
        lm.content.values().sum::<u64>() + lm.ballast
    }

    fn condition(&self, c: &Condition) -> Verdict {
        let yes = |b: bool, why: &str| {
            if b {
                Verdict::Pass
            } else {
                Verdict::Fail(why.to_owned())
            }
        };
        match c {
            Condition::State { on, file, is } => match self.state_of(*on, file) {
                None => Verdict::Fail("no version of it".into()),
                Some(s) => yes(Kind::of(&s) == *is, &format!("the model says {s:?}")),
            },
            Condition::Headline { on, is } => {
                let h = self.headline(*on);
                yes(h == *is, &format!("the model says {h:?}"))
            }
            Condition::Bytes { on, file, content } => {
                let at = self.laptops[on.0 as usize].disk.get(file);
                yes(
                    at == Some(&OnDisk::File(content.key())),
                    &format!("the model has {at:?}"),
                )
            }
            Condition::Absent { on, file } => {
                let at = self.laptops[on.0 as usize].disk.get(file);
                yes(at.is_none(), &format!("the model has {at:?}"))
            }
            Condition::NoPointers { .. } => Verdict::Pass,
            Condition::NotShared { on, file } => {
                let at = self.laptops[on.0 as usize].disk.get(file);
                yes(at == Some(&OnDisk::Link), "not a link in the model")
            }
            Condition::Says { on, code } => {
                let says = match on {
                    Device::Laptop(l) => {
                        let lm = &self.laptops[l.0 as usize];
                        let reported = lm.views.iter().flat_map(|v| v.says.iter());
                        lm.says.iter().chain(reported).cloned().collect()
                    }
                    Device::Custodian(c) => self.custodians[c.0 as usize].says.clone(),
                };
                yes(says.contains(code), &format!("the model says {says:?}"))
            }
            Condition::Holds { on, file } => {
                let cm = &self.custodians[on.0 as usize];
                let key = self
                    .laptops
                    .iter()
                    .find_map(|l| l.history.get(file)?.last().map(|v| v.key.clone()));
                yes(
                    key.is_some_and(|k| cm.holds.contains_key(&k)),
                    "the model's custodian lacks it",
                )
            }
            Condition::Transferring { .. }
            | Condition::Versions { .. }
            | Condition::Kept { .. } => {
                Verdict::Skip("observed by the process executor only".into())
            }
            Condition::Suggested { file, .. } => {
                yes(self.suggested(file), "no suggestion for it in the model")
            }
            Condition::Knows { device, .. } => {
                let present = match device {
                    Device::Laptop(l) => self.laptops[l.0 as usize].present,
                    Device::Custodian(c) => self.custodians[c.0 as usize].present,
                };
                yes(present, "not in the model's roster")
            }
            Condition::DiskWithin { on, permille } => {
                let lm = &self.laptops[on.0 as usize];
                let kept: u64 = lm
                    .disk
                    .values()
                    .filter_map(|d| match d {
                        OnDisk::File(k) => lm.content.get(k).copied(),
                        OnDisk::Link => None,
                    })
                    .sum();
                let used = self.used(*on) - lm.ballast;
                yes(
                    used * 1000 <= kept * u64::from(*permille),
                    &format!("{used} bytes for {kept} kept"),
                )
            }
            Condition::Not(c) => match self.condition(c) {
                Verdict::Pass => Verdict::Fail("it holds".into()),
                Verdict::Fail(_) => Verdict::Pass,
                skip => skip,
            },
            Condition::All(cs) => cs
                .iter()
                .map(|c| self.condition(c))
                .find(|v| *v != Verdict::Pass)
                .unwrap_or(Verdict::Pass),
        }
    }

    fn laptop_op(&mut self, on: Laptop, op: &LaptopOp) -> Outcome {
        let d = Device::Laptop(on);
        if !matches!(
            op,
            LaptopOp::Write { .. } | LaptopOp::Symlink { .. } | LaptopOp::Forge { .. }
        ) && !matches!(op, LaptopOp::Recover { .. })
            && !self.laptops[on.0 as usize].daemon.running
        {
            return Outcome::Refused("daemon-not-running".into());
        }
        match op {
            LaptopOp::Write { file, content } => {
                let key = content.key();
                self.bodies.insert(key.clone(), content.clone());
                let lm = self.laptop(on);
                lm.disk.insert(file.clone(), OnDisk::File(key.clone()));
                lm.content.insert(key, content.len());
            }
            LaptopOp::Symlink { file, .. } => {
                self.laptop(on).disk.insert(file.clone(), OnDisk::Link);
            }
            LaptopOp::Free { folder } => {
                self.laptop(on).freed.insert(*folder);
                self.log("Evict", "Start", d);
            }
            LaptopOp::Keep { folder } => {
                self.laptop(on).freed.remove(folder);
                self.log("Evict", "Start", d);
            }
            LaptopOp::Restore { file, version } => {
                let ver = self.laptops[on.0 as usize]
                    .history
                    .get(file)
                    .and_then(|v| v.get((*version as usize).wrapping_sub(1)))
                    .cloned();
                let Some(ver) = ver else {
                    return Outcome::Refused("no-such-version".into());
                };
                let lm = &self.laptops[on.0 as usize];
                let here = lm.content.contains_key(&ver.key);
                if !here && lm.used_with(0) + ver.len > lm.disk_bytes {
                    self.laptop(on).says.insert("disk-full".into());
                    return Outcome::Refused("disk-full".into());
                }
                if !here
                    && !self
                        .custodians
                        .iter()
                        .any(|c| c.present && c.daemon.running && c.holds.contains_key(&ver.key))
                {
                    return Outcome::Refused("content-unreachable".into());
                }
                let lm = self.laptop(on);
                lm.content.insert(ver.key.clone(), ver.len);
                lm.disk.insert(file.clone(), OnDisk::File(ver.key.clone()));
                self.log("Restore", "Start", d);
            }
            LaptopOp::Invite { custodian } => {
                let c = &mut self.custodians[custodian.0 as usize];
                c.present = true;
                c.daemon.running = true;
                self.log("Roster", "Commit", d);
            }
            LaptopOp::RemoveComputer { custodian } => {
                self.custodians[custodian.0 as usize].present = false;
                self.log("Roster", "Commit", d);
            }
            LaptopOp::RecoverySheet => self.laptop(on).sheet = true,
            LaptopOp::Forge { .. } => self.laptop(on).forged = true,
            LaptopOp::Recover { .. } => {
                let lm = self.laptop(on);
                lm.present = true;
                lm.daemon.running = true;
                self.log("Roster", "Merge", d);
            }
            LaptopOp::Join { laptop } => return self.join(on, *laptop),
            LaptopOp::Accept { file } => return self.decide(on, file, true),
            LaptopOp::Decline { file } => return self.decide(on, file, false),
            LaptopOp::TakeGroup { file } => return self.take_group(on, file),
        }
        Outcome::Done
    }

    fn device_op(&mut self, on: Device, op: &DeviceOp) {
        let (daemon, disk, ballast, used) = match on {
            Device::Laptop(l) => {
                let used = self.laptops[l.0 as usize].used_with(0);
                let lm = &mut self.laptops[l.0 as usize];
                (&mut lm.daemon, lm.disk_bytes, &mut lm.ballast, used)
            }
            Device::Custodian(c) => {
                let cm = &mut self.custodians[c.0 as usize];
                let used = cm.holds.values().sum::<u64>();
                (&mut cm.daemon, cm.disk_bytes, &mut cm.ballast, used)
            }
        };
        match op {
            DeviceOp::Start => daemon.running = true,
            DeviceOp::Stop | DeviceOp::Kill => daemon.running = false,
            DeviceOp::Crash { at, n } => {
                daemon.running = true;
                daemon.plan = Some((*at, *n));
            }
            DeviceOp::Fill { leave_mib } => {
                *ballast = disk.saturating_sub(used + u64::from(*leave_mib) * MIB);
            }
            DeviceOp::Unfill => *ballast = 0,
        }
        if let (Device::Laptop(l), DeviceOp::Start | DeviceOp::Crash { .. }) = (on, op) {
            self.recover(l);
        }
    }

    fn measure(&mut self, on: Device, label: &str, w: &World) {
        let mut parts = BTreeMap::new();
        match on {
            Device::Laptop(l) => {
                let lm = &self.laptops[l.0 as usize];
                parts.insert("folders".to_owned(), lm.content.values().sum());
                parts.insert("ballast".to_owned(), lm.ballast);
            }
            Device::Custodian(c) => {
                let cm = &self.custodians[c.0 as usize];
                parts.insert("annex".to_owned(), cm.holds.values().sum());
                parts.insert("ballast".to_owned(), cm.ballast);
            }
        }
        self.measures.push(Measure {
            device: w.name(on).to_owned(),
            label: label.to_owned(),
            parts,
        });
    }

    /// Performs one step.
    pub fn perform(&mut self, w: &World, s: &Step) -> Verdict {
        match s {
            Step::Laptop { on, op, expect } => {
                let got = self.laptop_op(*on, op);
                if got == *expect {
                    Verdict::Pass
                } else {
                    Verdict::Fail(format!("the model answers {got:?}"))
                }
            }
            Step::Custodian {
                on,
                op: CustodianOp::Destroy,
            } => {
                let cm = &mut self.custodians[on.0 as usize];
                cm.holds.clear();
                cm.known.clear();
                cm.lost = true;
                cm.daemon.running = true;
                Verdict::Pass
            }
            Step::Device { on, op } => {
                self.device_op(*on, op);
                Verdict::Pass
            }
            Step::Expect(c) => self.condition(c),
            Step::Wait { until, .. } => {
                if until.real_only() {
                    return Verdict::Skip("observed by the process executor only".into());
                }
                self.settle();
                self.condition(until)
            }
            Step::Hold { cond, .. } => match self.condition(cond) {
                Verdict::Pass => {
                    self.settle();
                    self.condition(cond)
                }
                other => other,
            },
            Step::Race(steps) => steps
                .iter()
                .map(|s| self.perform(w, s))
                .find(|v| *v != Verdict::Pass)
                .unwrap_or(Verdict::Pass),
            Step::Measure { on, label } => {
                self.settle();
                self.measure(*on, label, w);
                Verdict::Pass
            }
        }
    }

    /// What every laptop shows of every file: its bytes on disk and its
    /// state's kind, for comparing runs.
    pub fn observation(&self) -> Vec<(u8, File, Option<OnDisk>, Option<Kind>)> {
        let mut out = Vec::new();
        for (i, lm) in self.laptops.iter().enumerate() {
            let files: BTreeSet<&File> = lm.disk.keys().chain(lm.history.keys()).collect();
            for f in files {
                let kind = self.state_of(Laptop(i as u8), f).map(|s| Kind::of(&s));
                out.push((i as u8, f.clone(), lm.disk.get(f).cloned(), kind));
            }
        }
        out
    }

    /// The device names, laptops first.
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

impl LaptopM {
    /// Bytes used, plus `extra`.
    pub fn used_with(&self, extra: u64) -> u64 {
        self.content.values().sum::<u64>() + self.ballast + extra
    }
}

/// The model as an [`Interpreter`].
#[derive(Debug, Default)]
pub struct ModelRun {
    /// The model, once begun.
    pub model: Option<Model>,
}

impl Interpreter for ModelRun {
    fn name(&self) -> String {
        "model".into()
    }

    fn begin(&mut self, world: &World) -> Result<(), String> {
        self.model = Some(Model::new(world));
        Ok(())
    }

    fn step(&mut self, world: &World, step: &Step) -> Verdict {
        match self.model.as_mut() {
            Some(m) => m.perform(world, step),
            None => Verdict::Fail("not begun".into()),
        }
    }

    fn measures(&mut self) -> Vec<Measure> {
        self.model
            .as_ref()
            .map(|m| m.measures.clone())
            .unwrap_or_default()
    }

    fn trace(&mut self, _wanted: &[Item]) -> Option<Vec<Label>> {
        self.model.as_ref().map(|m| m.trace.clone())
    }

    fn end(&mut self) {}
}

/// Runs `steps` of `world` from a fresh model, and returns the model.
pub fn model_after(world: &World, steps: &[Step]) -> Model {
    let mut m = Model::new(world);
    for s in steps {
        m.perform(world, s);
    }
    m
}
