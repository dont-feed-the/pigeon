//! Scenarios: a world and a sequence of steps, built with typed handles.
//!
//! [`Steps`] is a monoid under [`Steps::then`], with [`Steps::empty`] as its
//! identity; every interpretation of a scenario is a fold over its steps, so
//! it maps `then` to sequential composition. A scenario is plain data: it is
//! printed with [`fmt::Display`], saved as JSON, compared, generated and
//! shrunk.
//!
//! Laws (`tests/laws.rs`): `then` is associative with identity `empty`;
//! [`validate`] of `a.then(b)` passes exactly when `a` passes and `b` passes
//! from where `a` left the world; [`project`] is a monoid homomorphism into
//! sequences of trace labels; JSON round-trips.

use std::fmt;

use pigeon_core::headline::Level;
use serde::{Deserialize, Serialize};

use crate::step::{Condition, CrashPoint, CustodianOp, DeviceOp, Kind, LaptopOp, Outcome, Step};
use crate::world::{Content, Custodian, Device, File, Folder, Laptop, World, portable};

/// A sequence of steps.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steps(pub Vec<Step>);

impl Steps {
    /// The identity: no step.
    pub fn empty() -> Steps {
        Steps(Vec::new())
    }

    /// `self`, then `next`.
    pub fn then(mut self, next: Steps) -> Steps {
        self.0.extend(next.0);
        self
    }
}

impl From<Vec<Step>> for Steps {
    fn from(steps: Vec<Step>) -> Steps {
        Steps(steps)
    }
}

/// What a scenario asks of the host that runs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Need {
    /// Disks of their declared size, that can fill up (images, not folders).
    SizedDisks,
    /// Files of a GiB or more.
    BigFiles,
}

/// A scenario of the slice (13 §13.12).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scenario {
    /// Its name, such as `S2`.
    pub id: String,
    /// What it asserts, in a line.
    pub title: String,
    /// Its devices and folders.
    pub world: World,
    /// What happens.
    pub steps: Steps,
    /// What the host must offer.
    pub needs: Vec<Need>,
}

/// A device handle: what every device undergoes.
pub trait Handle: Copy + Into<Device> {
    /// Start its daemon.
    fn start(self) -> Step {
        device(self.into(), DeviceOp::Start)
    }
    /// Stop its daemon cleanly.
    fn stop(self) -> Step {
        device(self.into(), DeviceOp::Stop)
    }
    /// Kill its daemon.
    fn kill(self) -> Step {
        device(self.into(), DeviceOp::Kill)
    }
    /// Restart it to crash at the `n`-th crossing of `at`.
    fn crash(self, at: CrashPoint, n: u32) -> Step {
        device(self.into(), DeviceOp::Crash { at, n })
    }
    /// Fill its disk, leaving `leave_mib` MiB.
    fn fill(self, leave_mib: u32) -> Step {
        device(self.into(), DeviceOp::Fill { leave_mib })
    }
    /// Remove the ballast.
    fn unfill(self) -> Step {
        device(self.into(), DeviceOp::Unfill)
    }
    /// It reports a problem with `code`.
    fn says(self, code: &str) -> Condition {
        Condition::Says {
            on: self.into(),
            code: code.to_owned(),
        }
    }
    /// Record its disk use under `label`.
    fn measure(self, label: &str) -> Step {
        Step::Measure {
            on: self.into(),
            label: label.to_owned(),
        }
    }
}

impl Handle for Laptop {}
impl Handle for Custodian {}

fn device(on: Device, op: DeviceOp) -> Step {
    Step::Device { on, op }
}

impl Laptop {
    fn does(self, op: LaptopOp) -> Step {
        Step::Laptop {
            on: self,
            op,
            expect: Outcome::Done,
        }
    }
    /// The person saves `content` at `file`.
    pub fn write(self, file: &File, content: Content) -> Step {
        self.does(LaptopOp::Write {
            file: file.clone(),
            content,
        })
    }
    /// The person saves `text` at `file`.
    pub fn write_text(self, file: &File, text: &str) -> Step {
        self.write(file, Content::Text(text.to_owned()))
    }
    /// A commit adding `file`, signed outside the group, lands on its
    /// `main` beside pigeond.
    pub fn forge(self, file: &File) -> Step {
        self.does(LaptopOp::Forge { file: file.clone() })
    }
    /// The person makes a symbolic link at `file`.
    pub fn symlink(self, file: &File, target: &str) -> Step {
        self.does(LaptopOp::Symlink {
            file: file.clone(),
            target: target.to_owned(),
        })
    }
    /// `pigeon free FOLDER`.
    pub fn free(self, folder: Folder) -> Step {
        self.does(LaptopOp::Free { folder })
    }
    /// `pigeon keep FOLDER`.
    pub fn keep(self, folder: Folder) -> Step {
        self.does(LaptopOp::Keep { folder })
    }
    /// `pigeon restore FILE --version N`.
    pub fn restore(self, file: &File, version: u64) -> Step {
        self.does(LaptopOp::Restore {
            file: file.clone(),
            version,
        })
    }
    /// Invite the spare custodian, which joins.
    pub fn invite(self, custodian: Custodian) -> Step {
        self.does(LaptopOp::Invite { custodian })
    }
    /// `pigeon remove-computer`.
    pub fn remove_computer(self, custodian: Custodian) -> Step {
        self.does(LaptopOp::RemoveComputer { custodian })
    }
    /// `pigeon recovery-sheet`.
    pub fn recovery_sheet(self) -> Step {
        self.does(LaptopOp::RecoverySheet)
    }
    /// This fresh laptop recovers the group with `from`'s sheet.
    pub fn recover_from(self, from: Laptop) -> Step {
        self.does(LaptopOp::Recover { from })
    }
    /// It reports `file`'s current version as `is`.
    pub fn sees(self, file: &File, is: Kind) -> Condition {
        Condition::State {
            on: self,
            file: file.clone(),
            is,
        }
    }
    /// Its headline is `is`.
    pub fn headline(self, is: Level) -> Condition {
        Condition::Headline { on: self, is }
    }
    /// `file` is on its disk with exactly `content`.
    pub fn has(self, file: &File, content: &Content) -> Condition {
        Condition::Bytes {
            on: self,
            file: file.clone(),
            content: content.clone(),
        }
    }
    /// `file` is on its disk with exactly `text`.
    pub fn has_text(self, file: &File, text: &str) -> Condition {
        self.has(file, &Content::Text(text.to_owned()))
    }
    /// `file` is absent from its disk, and no pointer stands in for it.
    pub fn lacks(self, file: &File) -> Condition {
        Condition::Absent {
            on: self,
            file: file.clone(),
        }
    }
    /// No pointer file in `folder`.
    pub fn no_pointers(self, folder: Folder) -> Condition {
        Condition::NoPointers { on: self, folder }
    }
    /// `pigeon status --not-shared` lists `file`.
    pub fn lists_not_shared(self, file: &File) -> Condition {
        Condition::NotShared {
            on: self,
            file: file.clone(),
        }
    }
    /// Steady disk use is at most `permille` / 1000 of the content kept.
    pub fn disk_within(self, permille: u32) -> Condition {
        Condition::DiskWithin { on: self, permille }
    }
}

/// A custodian has no laptop operations: asking one to free a folder does
/// not compile.
///
/// ```compile_fail
/// use pigeon_lab::world::World;
/// let mut w = World::new("g", 2);
/// let c = w.custodian("backup");
/// let docs = w.folder("docs");
/// let _ = c.free(docs);
/// ```
impl Custodian {
    /// Its disk is destroyed and made anew.
    pub fn destroy(self) -> Step {
        Step::Custodian {
            on: self,
            op: CustodianOp::Destroy,
        }
    }
    /// Its repository holds `file`'s current content.
    pub fn holds(self, file: &File) -> Condition {
        Condition::Holds {
            on: self,
            file: file.clone(),
        }
    }
    /// A partial object of `file` is on its way to it.
    pub fn receiving(self, file: &File) -> Condition {
        Condition::Transferring {
            to: self,
            file: file.clone(),
        }
    }
}

impl Step {
    /// The same laptop request, expected to fail with `code`.
    pub fn refused(self, code: &str) -> Step {
        match self {
            Step::Laptop { on, op, .. } => Step::Laptop {
                on,
                op,
                expect: Outcome::Refused(code.to_owned()),
            },
            other => other,
        }
    }
}

/// Wait for `until`, at most `within_s` seconds.
pub fn wait(until: Condition, within_s: u32) -> Step {
    Step::Wait { until, within_s }
}

/// `c` holds now and for `for_s` seconds.
pub fn hold(cond: Condition, for_s: u32) -> Step {
    Step::Hold { cond, for_s }
}

/// Check `c` now.
pub fn expect(c: Condition) -> Step {
    Step::Expect(c)
}

/// Run `steps` at once.
pub fn race(steps: Vec<Step>) -> Step {
    Step::Race(steps)
}

/// The negation of `c`.
pub fn not(c: Condition) -> Condition {
    Condition::Not(Box::new(c))
}

/// Every one of `cs`.
pub fn all(cs: Vec<Condition>) -> Condition {
    Condition::All(cs)
}

/// Who is in the group, as validation follows it through the steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Presence {
    /// Per laptop: in the group.
    pub laptops: Vec<bool>,
    /// Per custodian: in the group.
    pub custodians: Vec<bool>,
    /// Per laptop: has printed a recovery sheet.
    pub sheets: Vec<bool>,
}

impl Presence {
    /// Who is present at the start.
    pub fn initial(world: &World) -> Presence {
        Presence {
            laptops: world.laptops.iter().map(|l| l.present).collect(),
            custodians: world.custodians.iter().map(|c| c.present).collect(),
            sheets: vec![false; world.laptops.len()],
        }
    }
}

/// Why a scenario is malformed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invalid {
    /// The step, from 0.
    pub step: usize,
    /// What is wrong.
    pub why: String,
}

fn check_file(w: &World, f: &File) -> Result<(), String> {
    if (f.folder.0 as usize) >= w.folders.len() {
        return Err(format!("no folder {}", f.folder.0));
    }
    if !portable(&f.name) {
        return Err(format!("{:?} is not a portable name", f.name));
    }
    Ok(())
}

fn check_condition(w: &World, p: &Presence, c: &Condition) -> Result<(), String> {
    let laptop = |l: Laptop| -> Result<(), String> {
        match p.laptops.get(l.0 as usize) {
            Some(true) => Ok(()),
            Some(false) => Err(format!("{} is not in the group", w.name(l.into()))),
            None => Err(format!("no laptop {}", l.0)),
        }
    };
    match c {
        Condition::State { on, file, .. }
        | Condition::Bytes { on, file, .. }
        | Condition::Absent { on, file }
        | Condition::NotShared { on, file }
        | Condition::Versions { on, file, .. }
        | Condition::Kept { on, file, .. }
        | Condition::Suggested { on, file } => laptop(*on).and(check_file(w, file)),
        Condition::Knows { on, device } if w.has(*device) => laptop(*on),
        Condition::Knows { .. } => Err("no such device".into()),
        Condition::Headline { on, .. } | Condition::DiskWithin { on, .. } => laptop(*on),
        Condition::NoPointers { on, folder } => laptop(*on).and(check_file(w, &folder.file("x"))),
        Condition::Says { on, code } => {
            if !w.has(*on) {
                return Err("no such device".into());
            }
            if code.is_empty() {
                return Err("an empty problem code".into());
            }
            Ok(())
        }
        Condition::Transferring { to, file } | Condition::Holds { on: to, file } => {
            if (to.0 as usize) >= w.custodians.len() {
                return Err(format!("no custodian {}", to.0));
            }
            check_file(w, file)
        }
        Condition::Not(c) => check_condition(w, p, c),
        Condition::All(cs) => cs.iter().try_for_each(|c| check_condition(w, p, c)),
    }
}

/// Checks that `by` may invite the spare laptop `l`, which then joins: a
/// new person's laptop is an admin's to invite, a known person's their own
/// or an admin's (09 §9.6).
fn check_join(w: &World, p: &mut Presence, by: Laptop, l: Laptop) -> Result<(), String> {
    let j = l.0 as usize;
    match p.laptops.get(j) {
        None => return Err(format!("no laptop {j}")),
        Some(true) => return Err(format!("{} is already in the group", w.name(l.into()))),
        Some(false) => {}
    }
    let person = |x: Laptop| w.laptops[x.0 as usize].person;
    let known = (0..w.laptops.len()).any(|k| p.laptops[k] && w.laptops[k].person == person(l));
    if !w.is_admin(by) && !(known && person(by) == person(l)) {
        return Err(format!(
            "{} may not invite {}",
            w.name(by.into()),
            w.name(l.into())
        ));
    }
    p.laptops[j] = true;
    Ok(())
}

/// Checks one step from `p`, and moves `p` past it.
pub fn check_step(w: &World, p: &mut Presence, s: &Step) -> Result<(), String> {
    match s {
        Step::Laptop { on, op, .. } => {
            let i = on.0 as usize;
            let Some(&present) = p.laptops.get(i) else {
                return Err(format!("no laptop {i}"));
            };
            if let LaptopOp::Recover { from } = op {
                if present {
                    return Err(format!("{} is already in the group", w.name((*on).into())));
                }
                if !p.sheets.get(from.0 as usize).copied().unwrap_or(false) {
                    return Err(format!(
                        "{} printed no recovery sheet",
                        w.name((*from).into())
                    ));
                }
                p.laptops[i] = true;
                return Ok(());
            }
            if !present {
                return Err(format!("{} is not in the group", w.name((*on).into())));
            }
            match op {
                LaptopOp::Write { file, .. }
                | LaptopOp::Symlink { file, .. }
                | LaptopOp::Forge { file }
                | LaptopOp::Restore { file, .. } => check_file(w, file),
                LaptopOp::Free { folder } | LaptopOp::Keep { folder } => {
                    check_file(w, &folder.file("x"))
                }
                LaptopOp::Invite { custodian } | LaptopOp::RemoveComputer { custodian } => {
                    let j = custodian.0 as usize;
                    let Some(&c) = p.custodians.get(j) else {
                        return Err(format!("no custodian {j}"));
                    };
                    let inviting = matches!(op, LaptopOp::Invite { .. });
                    if c == inviting {
                        let state = if c { "already" } else { "not" };
                        return Err(format!(
                            "{} is {state} in the group",
                            w.name((*custodian).into())
                        ));
                    }
                    p.custodians[j] = inviting;
                    Ok(())
                }
                LaptopOp::RecoverySheet => {
                    p.sheets[i] = true;
                    Ok(())
                }
                LaptopOp::Recover { .. } => Ok(()),
                LaptopOp::Join { laptop } => check_join(w, p, *on, *laptop),
                LaptopOp::Accept { file }
                | LaptopOp::Decline { file }
                | LaptopOp::TakeGroup { file } => check_file(w, file),
            }
        }
        Step::Custodian { on, .. } => match p.custodians.get(on.0 as usize) {
            Some(_) => Ok(()),
            None => Err(format!("no custodian {}", on.0)),
        },
        Step::Device { on, op } => {
            if !w.has(*on) {
                return Err("no such device".into());
            }
            if let DeviceOp::Crash { n: 0, .. } = op {
                return Err("crossings count from 1".into());
            }
            Ok(())
        }
        Step::Expect(c) => check_condition(w, p, c),
        Step::Wait {
            until: c,
            within_s: s,
        }
        | Step::Hold { cond: c, for_s: s } => {
            if *s == 0 {
                return Err("a wait or a hold needs a duration".into());
            }
            check_condition(w, p, c)
        }
        Step::Race(steps) => {
            if steps.len() < 2 {
                return Err("a race needs two steps".into());
            }
            steps.iter().try_for_each(|s| match s {
                Step::Race(_) | Step::Wait { .. } | Step::Hold { .. } => {
                    Err("a race holds requests and writes".into())
                }
                s => check_step(w, p, s),
            })
        }
        Step::Measure { on, label } => {
            if !w.has(*on) || label.is_empty() {
                return Err("a measure needs a device and a label".into());
            }
            Ok(())
        }
    }
}

/// Checks `steps` from `p`, which ends where the steps leave the world.
pub fn validate_from(w: &World, p: &mut Presence, steps: &Steps) -> Result<(), Invalid> {
    for (i, s) in steps.0.iter().enumerate() {
        check_step(w, p, s).map_err(|why| Invalid { step: i, why })?;
    }
    Ok(())
}

/// Checks a scenario: every handle names a device of its world that is in
/// the group when it acts, names are portable, deadlines are positive.
pub fn validate(s: &Scenario) -> Result<(), Invalid> {
    if s.world.admin().is_none() {
        return Err(Invalid {
            step: 0,
            why: "no laptop creates the group".into(),
        });
    }
    validate_from(&s.world, &mut Presence::initial(&s.world), &s.steps)
}

/// A trace label: an action of a TLA+ specification, logged by a device.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label {
    /// The specification.
    pub spec: &'static str,
    /// The action.
    pub action: &'static str,
    /// The device that logs it.
    pub device: Device,
}

/// One item of a projection: a label, or labels in any order (a race).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// This label.
    One(Label),
    /// These labels, in some order.
    AnyOrder(Vec<Label>),
}

fn labels_of(s: &Step) -> Vec<Label> {
    let l = |spec, action, device| Label {
        spec,
        action,
        device,
    };
    match s {
        Step::Laptop { on, op, expect } => {
            let d = Device::Laptop(*on);
            if *expect != Outcome::Done {
                return Vec::new();
            }
            match op {
                // Free journals `begin` before anything can crash.
                LaptopOp::Free { .. } => vec![l("Evict", "Start", d)],
                LaptopOp::Restore { .. } => vec![l("Restore", "Start", d)],
                LaptopOp::TakeGroup { .. } => vec![l("Resolve", "Resolve", d)],
                // The inviter commits the roster that adds the custodian;
                // a removal is a roster commit too.
                LaptopOp::Invite { .. }
                | LaptopOp::RemoveComputer { .. }
                | LaptopOp::Join { .. } => {
                    vec![l("Roster", "Commit", d)]
                }
                // A write is the person's, not pigeond's: two writes before a
                // capture make one version. Keep begins an install, or, when
                // the person's save got there first, releases (07 §7.5).
                // pigeond logs no event of its own for the others yet.
                LaptopOp::Write { .. }
                | LaptopOp::Keep { .. }
                | LaptopOp::Recover { .. }
                | LaptopOp::Symlink { .. }
                | LaptopOp::Forge { .. }
                | LaptopOp::RecoverySheet
                | LaptopOp::Accept { .. }
                | LaptopOp::Decline { .. } => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// The trace labels a run of `steps` must log, in order: the person's
/// actions and the requests that succeed. A crash or a kill is not among
/// them: a plan whose point is never crossed crashes nothing. It maps
/// `then` to concatenation.
pub fn project(steps: &Steps) -> Vec<Item> {
    steps
        .0
        .iter()
        .flat_map(|s| match s {
            Step::Race(inner) => {
                let ls: Vec<Label> = inner.iter().flat_map(labels_of).collect();
                if ls.is_empty() {
                    Vec::new()
                } else {
                    vec![Item::AnyOrder(ls)]
                }
            }
            s => labels_of(s).into_iter().map(Item::One).collect(),
        })
        .collect()
}

/// Whether `trace` contains the projection as a subsequence, the labels of
/// an [`Item::AnyOrder`] in any order.
pub fn embeds(projection: &[Item], trace: &[Label]) -> bool {
    let mut at = 0;
    for item in projection {
        let mut want: Vec<&Label> = match item {
            Item::One(l) => vec![l],
            Item::AnyOrder(ls) => ls.iter().collect(),
        };
        while !want.is_empty() {
            let Some(t) = trace.get(at) else { return false };
            if let Some(k) = want.iter().position(|w| *w == t) {
                want.remove(k);
            }
            at += 1;
        }
    }
    true
}

/// Whether, for each device, its labels of the projection embed in its
/// events of the trace. Each device logs on its own clock (12 §12.3), so
/// only each device's order is checked, never an order across devices.
pub fn embeds_per_device(projection: &[Item], trace: &[Label]) -> bool {
    let devices: std::collections::BTreeSet<Device> = projection
        .iter()
        .flat_map(|i| match i {
            Item::One(l) => vec![l.device],
            Item::AnyOrder(ls) => ls.iter().map(|l| l.device).collect(),
        })
        .collect();
    devices.iter().all(|d| {
        let mine: Vec<Item> = projection
            .iter()
            .filter_map(|i| match i {
                Item::One(l) => (l.device == *d).then(|| i.clone()),
                Item::AnyOrder(ls) => {
                    let ls: Vec<Label> = ls.iter().filter(|l| l.device == *d).cloned().collect();
                    (!ls.is_empty()).then_some(Item::AnyOrder(ls))
                }
            })
            .collect();
        let theirs: Vec<Label> = trace.iter().filter(|l| l.device == *d).cloned().collect();
        embeds(&mine, &theirs)
    })
}

fn condition_text(w: &World, c: &Condition) -> String {
    let n = |l: Laptop| w.name(l.into()).to_owned();
    match c {
        Condition::State { on, file, is } => format!("{} sees {} {is:?}", n(*on), w.path(file)),
        Condition::Headline { on, is } => format!(
            "{}'s headline is {}",
            n(*on),
            crate::step::level_name::of(*is)
        ),
        Condition::Bytes { on, file, content } => {
            format!(
                "{} has {} = {}",
                n(*on),
                w.path(file),
                content_text(content)
            )
        }
        Condition::Absent { on, file } => format!("{} lacks {}, no pointer", n(*on), w.path(file)),
        Condition::NoPointers { on, folder } => {
            format!("{} has no pointer in {}", n(*on), w.folder_name(*folder))
        }
        Condition::NotShared { on, file } => {
            format!("{} lists {} as not shared", n(*on), w.path(file))
        }
        Condition::Says { on, code } => format!("{} says {code}", w.name(*on)),
        Condition::Transferring { to, file } => {
            format!("{} is receiving {}", w.name((*to).into()), w.path(file))
        }
        Condition::Holds { on, file } => format!("{} holds {}", w.name((*on).into()), w.path(file)),
        Condition::DiskWithin { on, permille } => {
            format!(
                "{}'s disk ≤ {}.{:03} × content kept",
                n(*on),
                permille / 1000,
                permille % 1000
            )
        }
        Condition::Versions { on, file, contents } => format!(
            "{} has {} and its copies = {{{}}}",
            n(*on),
            w.path(file),
            contents
                .iter()
                .map(content_text)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Condition::Kept { on, file, content } => format!(
            "{} keeps {} = {} or a copy",
            n(*on),
            w.path(file),
            content_text(content)
        ),
        Condition::Suggested { on, file } => {
            format!("{} lists a suggestion for {}", n(*on), w.path(file))
        }
        Condition::Knows { on, device } => format!("{} knows {}", n(*on), w.name(*device)),
        Condition::Not(c) => format!("not ({})", condition_text(w, c)),
        Condition::All(cs) => cs
            .iter()
            .map(|c| condition_text(w, c))
            .collect::<Vec<_>>()
            .join(" and "),
    }
}

fn content_text(c: &Content) -> String {
    match c {
        Content::Text(t) if t.len() <= 24 => format!("{t:?}"),
        Content::Text(t) => format!("{} bytes of text", t.len()),
        Content::Random { seed, len } => format!("{len} random bytes (seed {seed})"),
    }
}

/// One step in words, its handles named from `w`.
pub fn step_text(w: &World, s: &Step) -> String {
    match s {
        Step::Laptop { on, op, expect } => {
            let who = w.name((*on).into());
            let what = match op {
                LaptopOp::Write { file, content } => {
                    format!("writes {} = {}", w.path(file), content_text(content))
                }
                LaptopOp::Symlink { file, target } => format!("links {} → {target}", w.path(file)),
                LaptopOp::Forge { file } => {
                    format!(
                        "gets a commit adding {}, signed outside the group",
                        w.path(file)
                    )
                }
                LaptopOp::Free { folder } => format!("frees {}", w.folder_name(*folder)),
                LaptopOp::Keep { folder } => format!("keeps {}", w.folder_name(*folder)),
                LaptopOp::Restore { file, version } => {
                    format!("restores {} version {version}", w.path(file))
                }
                LaptopOp::Invite { custodian } => {
                    format!("invites {}, which joins", w.name((*custodian).into()))
                }
                LaptopOp::RemoveComputer { custodian } => {
                    format!("removes {}", w.name((*custodian).into()))
                }
                LaptopOp::RecoverySheet => "prints a recovery sheet".to_owned(),
                LaptopOp::Recover { from } => {
                    format!("recovers with {}'s sheet", w.name((*from).into()))
                }
                LaptopOp::Join { laptop } => {
                    format!("invites {}, which joins", w.name((*laptop).into()))
                }
                LaptopOp::Accept { file } => format!("accepts the suggestion for {}", w.path(file)),
                LaptopOp::Decline { file } => {
                    format!("declines the suggestion for {}", w.path(file))
                }
                LaptopOp::TakeGroup { file } => {
                    format!("takes the group's version of {}", w.path(file))
                }
            };
            match expect {
                Outcome::Done => format!("{who} {what}"),
                Outcome::Refused(code) => format!("{who} {what}: refused with {code}"),
            }
        }
        Step::Custodian {
            on,
            op: CustodianOp::Destroy,
        } => format!("{}'s disk is destroyed", w.name((*on).into())),
        Step::Device { on, op } => {
            let who = w.name(*on);
            match op {
                DeviceOp::Start => format!("{who} starts"),
                DeviceOp::Stop => format!("{who} stops"),
                DeviceOp::Kill => format!("{who} is killed"),
                DeviceOp::Crash { at, n } => {
                    format!("{who} crashes at {} (crossing {n})", at.name())
                }
                DeviceOp::Fill { leave_mib } => format!("{who}'s disk fills, {leave_mib} MiB left"),
                DeviceOp::Unfill => format!("{who}'s disk is emptied of ballast"),
            }
        }
        Step::Expect(c) => format!("expect: {}", condition_text(w, c)),
        Step::Wait { until, within_s } => {
            format!("within {within_s} s: {}", condition_text(w, until))
        }
        Step::Hold { cond, for_s } => format!("for {for_s} s: {}", condition_text(w, cond)),
        Step::Race(steps) => format!(
            "at once: {}",
            steps
                .iter()
                .map(|s| step_text(w, s))
                .collect::<Vec<_>>()
                .join(" | ")
        ),
        Step::Measure { on, label } => format!("measure {}'s disk ({label})", w.name(*on)),
    }
}

impl fmt::Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}: {}", self.id, self.title)?;
        writeln!(f, "  {}", self.world)?;
        for (i, s) in self.steps.0.iter().enumerate() {
            writeln!(f, "  {:>2}. {}", i + 1, step_text(&self.world, s))?;
        }
        Ok(())
    }
}
