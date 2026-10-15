//! The process executor: each device a real `pigeond`, each request a real
//! `pigeon --json` (13 §13.12).
//!
//! Devices live in [`host::Space`]s: loop-mounted ext4 images and mount
//! namespaces on Linux, APFS images on macOS, plain directories elsewhere.
//! Every device has its own home and XDG directories, and its daemon is
//! supervised: a daemon that dies unasked, as a crash plan makes it, is
//! restarted without the plan, as systemd or launchd would. Waits poll their
//! condition until the deadline. At the end the devices' trace logs are
//! merged for the projection check and for TLC.
//! Lab devices use one-second rounds; scenario deadlines still bound waits.

pub mod cli;
mod forge;
pub mod host;
pub mod observe;
mod people;
pub mod trace;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use host::{Isolation, Space};

use crate::run::{Interpreter, Measure, Verdict};
use crate::scenario::{Label, Need, Scenario};
use crate::step::{Condition, CustodianOp, DeviceOp, LaptopOp, Outcome, Step};
use crate::world::{Content, Device, File, Laptop, World};

/// Where the executor finds its binaries and puts its files.
#[derive(Clone, Debug)]
pub struct Config {
    /// The directory of `pigeond`, `pigeon` and `git-annex-p2p-iroh`, built
    /// with `--features lab,tla-trace`.
    pub bin: PathBuf,
    /// Scratch space for the devices' disks.
    pub work: PathBuf,
    /// Reports, merged traces and daemon logs.
    pub out: PathBuf,
    /// How devices are kept apart.
    pub isolation: Isolation,
}

/// Why this host cannot run `s`, if it cannot.
pub fn unrunnable(s: &Scenario, isolation: Isolation) -> Option<String> {
    if s.needs.contains(&Need::SizedDisks) && !isolation.sized() {
        return Some(format!("needs sized disks; {} has none", isolation.name()));
    }
    None
}

#[derive(Debug)]
struct Daemon {
    child: Child,
    pid: Option<u32>,
}

#[derive(Debug)]
struct Dev {
    name: String,
    space: Space,
    env: Vec<(String, String)>,
    daemon: Option<Daemon>,
    up: bool,
    plan: Option<String>,
    tree: PathBuf,
    restarts: u32,
    peak: u64,
}

/// The executor of one scenario.
#[derive(Debug)]
pub struct Exec {
    cfg: Config,
    id: String,
    devs: Vec<Dev>,
    laptops: usize,
    written: BTreeMap<File, Vec<Content>>,
    shas: BTreeMap<Content, String>,
    measures: Vec<Measure>,
    sheets: BTreeMap<u8, PathBuf>,
}

const MIB: u64 = 1 << 20;

impl Exec {
    /// An executor for the scenario `id`.
    pub fn new(cfg: Config, id: &str) -> Exec {
        Exec {
            cfg,
            id: id.to_owned(),
            devs: Vec::new(),
            laptops: 0,
            written: BTreeMap::new(),
            shas: BTreeMap::new(),
            measures: Vec::new(),
            sheets: BTreeMap::new(),
        }
    }

    fn ix(&self, d: Device) -> usize {
        match d {
            Device::Laptop(l) => l.0 as usize,
            Device::Custodian(c) => self.laptops + c.0 as usize,
        }
    }

    fn dev(&mut self, d: Device) -> &mut Dev {
        let i = self.ix(d);
        &mut self.devs[i]
    }

    fn out_dir(&self) -> PathBuf {
        self.cfg.out.join(&self.id)
    }

    fn pigeon(&self, d: Device, args: &[String]) -> Result<cli::Answer, String> {
        let dev = &self.devs[self.ix(d)];
        let program = self.cfg.bin.join("pigeon");
        cli::answer(&mut dev.space.command(&program, args, &dev.env))
    }

    fn sha(&mut self, c: &Content) -> String {
        if let Some(s) = self.shas.get(c) {
            return s.clone();
        }
        let s = c.sha256();
        self.shas.insert(c.clone(), s.clone());
        s
    }

    fn path(&self, l: Laptop, f: &File, w: &World) -> PathBuf {
        self.devs[l.0 as usize].tree.join(w.path(f))
    }

    fn start(&mut self, d: Device) -> Result<(), String> {
        let i = self.ix(d);
        let bin = self.cfg.bin.clone();
        let dev = &mut self.devs[i];
        if dev.daemon.is_some() {
            return Ok(());
        }
        let pidfile = dev.space.dir.join("pigeond.pid");
        let _ = fs::remove_file(&pidfile);
        let log = dev.space.dir.join("pigeond.log");
        // A backup machine's pigeond is the system service (09 §9.6).
        let role = if i >= self.laptops { " --service" } else { "" };
        let dev = &mut self.devs[i];
        let script = format!(
            "echo $$ > '{}'; exec '{}'{role} >> '{}' 2>&1",
            pidfile.display(),
            bin.join("pigeond").display(),
            log.display()
        );
        let mut env = dev.env.clone();
        let planned = dev.plan.take();
        if let Some(plan) = planned.clone() {
            env.push((s("PIGEON_LAB_CRASH"), plan));
        }
        let child = dev
            .space
            .command(Path::new("/bin/sh"), &[s("-c"), script], &env)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("start {}: {e}", dev.name))?;
        dev.daemon = Some(Daemon { child, pid: None });
        dev.up = true;
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            let pid = fs::read_to_string(&pidfile)
                .ok()
                .and_then(|t| t.trim().parse().ok());
            if let Some(dm) = self.devs[i].daemon.as_mut() {
                dm.pid = pid;
            }
            if pid.is_some()
                && let Ok(a) = self.pigeon(d, &cli::status(None, false))
                && a.problem.as_deref() != Some("daemon-not-running")
                && (a.ok || a.problem.is_some())
            {
                return Ok(());
            }
            let exited = self.devs[i]
                .daemon
                .as_mut()
                .and_then(|dm| dm.child.try_wait().ok().flatten());
            if let Some(status) = exited {
                self.devs[i].daemon = None;
                // The armed crash came in the first round, before the first
                // answer: it is the crash asked for; restart as supervise does.
                if planned.is_some() && matches!(status.signal(), Some(6 | 9)) {
                    self.devs[i].restarts += 1;
                    return self.start(d);
                }
                return Err(format!(
                    "pigeond of {} exited at start; see {}",
                    self.devs[i].name,
                    log.display()
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(format!(
            "pigeond of {} did not answer within 30 s",
            self.devs[i].name
        ))
    }

    fn signal(&mut self, d: Device, sig: &str, wait_s: u64) {
        let i = self.ix(d);
        let dev = &mut self.devs[i];
        dev.up = false;
        let Some(mut dm) = dev.daemon.take() else {
            return;
        };
        if let Some(pid) = dm.pid {
            let _ = Command::new("kill")
                .arg(format!("-{sig}"))
                .arg(pid.to_string())
                .status();
        }
        let deadline = Instant::now() + Duration::from_secs(wait_s);
        while Instant::now() < deadline {
            if dm.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if let Some(pid) = dm.pid {
            let _ = Command::new("kill")
                .arg("-KILL")
                .arg(pid.to_string())
                .status();
        }
        let _ = dm.child.wait();
    }

    /// Whether device `i`'s daemon died unasked, or does within 5 s.
    fn died(&mut self, i: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let gone = self.devs[i]
                .daemon
                .as_mut()
                .is_some_and(|dm| dm.child.try_wait().ok().flatten().is_some());
            if gone || !self.devs[i].up || Instant::now() >= deadline {
                return gone && self.devs[i].up;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Restarts every daemon that died unasked.
    fn supervise(&mut self) {
        for i in 0..self.devs.len() {
            let died = self.devs[i]
                .daemon
                .as_mut()
                .is_some_and(|dm| dm.child.try_wait().ok().flatten().is_some());
            if died && self.devs[i].up {
                self.devs[i].daemon = None;
                self.devs[i].restarts += 1;
                let d = self.device_of(i);
                let _ = self.start(d);
            }
        }
        for i in 0..self.devs.len() {
            let total: u64 = observe::usage(&self.devs[i].space.root)
                .iter()
                .filter(|(k, _)| k.as_str() != "ballast")
                .map(|(_, v)| v)
                .sum();
            self.devs[i].peak = self.devs[i].peak.max(total);
        }
    }

    fn device_of(&self, i: usize) -> Device {
        if i < self.laptops {
            Device::Laptop(Laptop(i as u8))
        } else {
            Device::Custodian(crate::world::Custodian((i - self.laptops) as u8))
        }
    }

    fn join_custodian(&mut self, by: Laptop, c: Device, w: &World) -> Result<Outcome, String> {
        self.start(c)?;
        let invite = self.pigeon(Device::Laptop(by), &cli::invite_custodian(w.name(c)))?;
        if !invite.ok {
            return Ok(invite.outcome());
        }
        let ticket = invite
            .json
            .as_ref()
            .and_then(|v| cli::find_string(v, "pgn1-"))
            .or_else(|| {
                invite
                    .stdout
                    .split_whitespace()
                    .find(|t| t.starts_with("pgn1-"))
                    .map(s)
            })
            .ok_or_else(|| format!("no invitation in {:?}", invite.stdout))?;
        Ok(self.pigeon(c, &cli::join_custodian(&ticket))?.outcome())
    }

    fn laptop_op(&mut self, on: Laptop, op: &LaptopOp, w: &World) -> Result<Outcome, String> {
        let d = Device::Laptop(on);
        Ok(match op {
            LaptopOp::Write { file, content } => {
                let path = self.path(on, file, w);
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                let mut f =
                    fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let mut err = None;
                content.for_each_chunk(|c| {
                    if err.is_none() {
                        err = f.write_all(c).err();
                    }
                });
                if let Some(e) = err {
                    return Err(format!("{}: {e}", path.display()));
                }
                self.written
                    .entry(file.clone())
                    .or_default()
                    .push(content.clone());
                Outcome::Done
            }
            LaptopOp::Forge { file } => {
                let dev = &self.devs[on.0 as usize];
                forge::forge(&dev.space, &dev.env, &dev.tree, &w.path(file))?;
                Outcome::Done
            }
            LaptopOp::Symlink { file, target } => {
                let path = self.path(on, file, w);
                std::os::unix::fs::symlink(target, &path)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                Outcome::Done
            }
            LaptopOp::Free { folder } | LaptopOp::Keep { folder } => {
                let verb = if matches!(op, LaptopOp::Free { .. }) {
                    "free"
                } else {
                    "keep"
                };
                let path = self.devs[on.0 as usize].tree.join(w.folder_name(*folder));
                self.pigeon(d, &cli::free_or_keep(verb, &path))?.outcome()
            }
            LaptopOp::Restore { file, version } => {
                let path = self.path(on, file, w);
                let got = self.pigeon(d, &cli::restore(&path, *version))?.outcome();
                if got == Outcome::Done {
                    let old = self
                        .written
                        .get(file)
                        .and_then(|v| v.get((*version as usize).wrapping_sub(1)))
                        .cloned();
                    if let Some(c) = old {
                        self.written.entry(file.clone()).or_default().push(c);
                    }
                }
                got
            }
            LaptopOp::Invite { custodian } => {
                self.join_custodian(on, Device::Custodian(*custodian), w)?
            }
            LaptopOp::RemoveComputer { custodian } => self
                .pigeon(
                    d,
                    &cli::remove_computer(w.name(Device::Custodian(*custodian))),
                )?
                .outcome(),
            LaptopOp::RecoverySheet => {
                let a = self.pigeon(d, &cli::recovery_sheet())?;
                let text = a
                    .json
                    .as_ref()
                    .and_then(longest_string)
                    .unwrap_or_else(|| a.stdout.clone());
                let file = self.devs[on.0 as usize]
                    .space
                    .dir
                    .join("recovery-sheet.txt");
                fs::write(&file, text).map_err(|e| e.to_string())?;
                self.sheets.insert(on.0, file);
                a.outcome()
            }
            LaptopOp::Recover { from } => {
                let sheet = self
                    .sheets
                    .get(&from.0)
                    .cloned()
                    .ok_or("no recovery sheet")?;
                self.start(d)?;
                let tree = self.devs[on.0 as usize].tree.clone();
                self.pigeon(d, &cli::recover(&sheet, &tree))?.outcome()
            }
            LaptopOp::Join { laptop } => self.join_laptop(on, *laptop, w)?,
            LaptopOp::Accept { file } => self.decide(on, file, true, w)?,
            LaptopOp::Decline { file } => self.decide(on, file, false, w)?,
            LaptopOp::TakeGroup { file } => self.take_group(on, file, w)?,
        })
    }

    fn device_op(&mut self, on: Device, op: &DeviceOp) -> Result<(), String> {
        match op {
            DeviceOp::Start => self.start(on),
            DeviceOp::Stop => {
                self.signal(on, "TERM", 15);
                Ok(())
            }
            DeviceOp::Kill => {
                self.signal(on, "KILL", 5);
                Ok(())
            }
            DeviceOp::Crash { at, n } => {
                self.signal(on, "TERM", 15);
                self.dev(on).plan = Some(format!("{}:{n}", at.name()));
                self.start(on)
            }
            DeviceOp::Fill { leave_mib } => {
                let dev = self.dev(on);
                let free = dev.space.free_bytes()?;
                let mut left = free.saturating_sub(u64::from(*leave_mib) * MIB);
                let path = dev.space.root.join("ballast");
                // Allocated, not written, where `fallocate` exists (Linux):
                // the image behind the disk stays sparse.
                let had = fs::metadata(&path).map_or(0, |m| m.len());
                let allocated = Command::new("fallocate")
                    .arg("-l")
                    .arg((had + left).to_string())
                    .arg(&path)
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                if allocated {
                    return Ok(());
                }
                let mut f = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .map_err(|e| e.to_string())?;
                let zeros = vec![0u8; 8 << 20];
                while left > 0 {
                    let n = left.min(zeros.len() as u64) as usize;
                    if f.write_all(&zeros[..n]).is_err() {
                        break;
                    }
                    left -= n as u64;
                }
                let _ = f.sync_all();
                Ok(())
            }
            DeviceOp::Unfill => {
                let _ = fs::remove_file(self.dev(on).space.root.join("ballast"));
                Ok(())
            }
        }
    }

    fn fields(&mut self, f: &File) -> Option<String> {
        let c = self.written.get(f)?.last()?.clone();
        let sha = self.sha(&c);
        Some(observe::key_fields(c.len(), &sha))
    }

    fn condition(&mut self, c: &Condition, w: &World) -> Verdict {
        let yes = |b: bool, why: String| if b { Verdict::Pass } else { Verdict::Fail(why) };
        let status = |me: &Exec,
                      l: Laptop,
                      path: Option<&Path>,
                      ns: bool|
         -> Result<serde_json::Value, String> {
            let a = me.pigeon(Device::Laptop(l), &cli::status(path, ns))?;
            a.json.ok_or_else(|| {
                format!("status --json printed {:?} {:?}", a.stdout, a.stderr.trim())
            })
        };
        match c {
            Condition::State { on, file, is } => {
                let path = self.path(*on, file, w);
                match status(self, *on, Some(&path), false) {
                    Err(e) => Verdict::Fail(e),
                    Ok(v) => match cli::version_kind(&v, &w.path(file)) {
                        None => Verdict::Fail(format!("no state for it in {v}")),
                        Some(k) => yes(k == *is, format!("pigeon says {k:?}")),
                    },
                }
            }
            Condition::Headline { on, is } => match status(self, *on, None, false) {
                Err(e) => Verdict::Fail(e),
                Ok(v) => match cli::level(&v) {
                    None => Verdict::Skip("status --json carries no level".into()),
                    Some(l) => yes(l == *is, format!("pigeon says {l:?}")),
                },
            },
            Condition::Bytes { on, file, content } => {
                let want = self.sha(content);
                let got = observe::sha256_settled(&self.path(*on, file, w));
                yes(
                    got.as_deref() == Some(want.as_str()),
                    format!("sha256 {got:?}"),
                )
            }
            Condition::Absent { on, file } => {
                let p = self.path(*on, file, w);
                yes(
                    fs::symlink_metadata(&p).is_err(),
                    format!("{} exists", p.display()),
                )
            }
            Condition::NoPointers { on, folder } => {
                let dir = self.devs[on.0 as usize].tree.join(w.folder_name(*folder));
                let ps = observe::pointers(&dir);
                yes(ps.is_empty(), format!("pointers: {ps:?}"))
            }
            Condition::NotShared { on, file } => match status(self, *on, None, true) {
                Err(e) => Verdict::Fail(e),
                Ok(v) => yes(
                    cli::not_shared(&v, &w.path(file)),
                    format!("not listed in {v}"),
                ),
            },
            Condition::Says { on, code } => {
                let mut found = false;
                for args in [cli::status(None, false), cli::plain("doctor")] {
                    if let Ok(a) = self.pigeon(*on, &args) {
                        found |= a.json.as_ref().is_some_and(|v| cli::has_code(v, code))
                            || a.problem.as_deref() == Some(code);
                    }
                }
                yes(found, format!("no problem {code} in status or doctor"))
            }
            Condition::Holds { on, file } | Condition::Transferring { to: on, file } => {
                let partial = matches!(c, Condition::Transferring { .. });
                let Some(fields) = self.fields(file) else {
                    return Verdict::Fail("the scenario wrote no such file".into());
                };
                let root = self.devs[self.ix(Device::Custodian(*on))]
                    .space
                    .root
                    .clone();
                yes(
                    observe::object(&root, &fields, partial),
                    "no such object".into(),
                )
            }
            Condition::DiskWithin { on, permille } => {
                let dev = &self.devs[on.0 as usize];
                let used: u64 = observe::usage(&dev.space.root)
                    .iter()
                    .filter(|(k, _)| k.as_str() != "ballast")
                    .map(|(_, v)| v)
                    .sum();
                let kept = observe::kept(&dev.tree);
                yes(
                    used * 1000 <= kept * u64::from(*permille),
                    format!("{used} bytes on disk for {kept} kept"),
                )
            }
            Condition::Versions { .. }
            | Condition::Kept { .. }
            | Condition::Suggested { .. }
            | Condition::Knows { .. } => self.people_condition(c, w),
            Condition::Not(c) => match self.condition(c, w) {
                Verdict::Pass => Verdict::Fail("it holds".into()),
                Verdict::Fail(_) => Verdict::Pass,
                skip => skip,
            },
            Condition::All(cs) => {
                for c in cs {
                    let v = self.condition(c, w);
                    if v != Verdict::Pass {
                        return v;
                    }
                }
                Verdict::Pass
            }
        }
    }

    fn perform(&mut self, w: &World, step: &Step) -> Result<Verdict, String> {
        self.supervise();
        Ok(match step {
            Step::Laptop { on, op, expect } => {
                let mut got = self.laptop_op(*on, op, w)?;
                // A request that met its daemon killed by a crash plan, as
                // one past a wait for what the daemon did just before the
                // crash can, is made again once the daemon is restarted.
                if got == Outcome::Refused(s("daemon-not-running")) && self.died(on.0 as usize) {
                    self.supervise();
                    got = self.laptop_op(*on, op, w)?;
                }
                if got == *expect {
                    Verdict::Pass
                } else {
                    Verdict::Fail(format!("pigeon answered {got:?}"))
                }
            }
            Step::Custodian {
                on,
                op: CustodianOp::Destroy,
            } => {
                let d = Device::Custodian(*on);
                self.signal(d, "KILL", 5);
                for repo in observe::annexed_repositories(&self.dev(d).space.root) {
                    let dir = if repo.ends_with(".git") {
                        repo.parent().map(Path::to_path_buf).unwrap_or(repo)
                    } else {
                        repo
                    };
                    host::remove_tree(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                }
                self.start(d)?;
                Verdict::Pass
            }
            Step::Device { on, op } => {
                self.device_op(*on, op)?;
                Verdict::Pass
            }
            Step::Expect(c) => self.condition(c, w),
            Step::Wait { until, within_s } => {
                let deadline = Instant::now() + Duration::from_secs(u64::from(*within_s));
                loop {
                    let v = self.condition(until, w);
                    if v == Verdict::Pass
                        || matches!(v, Verdict::Skip(_))
                        || Instant::now() >= deadline
                    {
                        break v;
                    }
                    std::thread::sleep(Duration::from_millis(500));
                    self.supervise();
                }
            }
            Step::Hold { cond, for_s } => {
                let deadline = Instant::now() + Duration::from_secs(u64::from(*for_s));
                loop {
                    let v = self.condition(cond, w);
                    if v != Verdict::Pass || Instant::now() >= deadline {
                        break v;
                    }
                    std::thread::sleep(Duration::from_millis(1000));
                    self.supervise();
                }
            }
            Step::Race(steps) => self.race(w, steps)?,
            Step::Measure { on, label } => {
                let parts = observe::usage(&self.dev(*on).space.root);
                self.measures.push(Measure {
                    device: w.name(*on).to_owned(),
                    label: label.clone(),
                    parts,
                });
                Verdict::Pass
            }
        })
    }

    /// Requests start together; the person's writes land while they run.
    fn race(&mut self, w: &World, steps: &[Step]) -> Result<Verdict, String> {
        let mut pending = Vec::new();
        for st in steps {
            if let Step::Laptop {
                on,
                op: op @ (LaptopOp::Free { folder } | LaptopOp::Keep { folder }),
                expect,
            } = st
            {
                let verb = if matches!(op, LaptopOp::Free { .. }) {
                    "free"
                } else {
                    "keep"
                };
                let path = self.devs[on.0 as usize].tree.join(w.folder_name(*folder));
                let dev = &self.devs[on.0 as usize];
                let child = dev
                    .space
                    .command(
                        &self.cfg.bin.join("pigeon"),
                        &cli::free_or_keep(verb, &path),
                        &dev.env,
                    )
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                pending.push((child, expect.clone()));
            }
        }
        let mut verdict = Verdict::Pass;
        for st in steps {
            if !matches!(
                st,
                Step::Laptop {
                    op: LaptopOp::Free { .. } | LaptopOp::Keep { .. },
                    ..
                }
            ) {
                let v = self.perform(w, st)?;
                if v != Verdict::Pass {
                    verdict = v;
                }
            }
        }
        for (child, expect) in pending {
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            let ok = out.status.success();
            let stderr = String::from_utf8_lossy(&out.stderr);
            let code = cli::answer_code(&stderr);
            let got = if ok {
                Outcome::Done
            } else {
                Outcome::Refused(code.unwrap_or_default())
            };
            if got != expect {
                verdict = Verdict::Fail(format!("a racing request answered {got:?}"));
            }
        }
        Ok(verdict)
    }
}

fn s(x: impl AsRef<str>) -> String {
    x.as_ref().to_owned()
}

/// A short directory name for scenario `id`: the paths under it stay short,
/// as git-annex's and pigeond's unix sockets live there and a socket's path
/// holds at most 104 bytes (a join fails with `inviter-offline` past it).
fn short_name(id: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in id.bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

fn longest_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(t) => Some(t.clone()),
        serde_json::Value::Array(xs) => {
            xs.iter().filter_map(longest_string).max_by_key(String::len)
        }
        serde_json::Value::Object(m) => m
            .values()
            .filter_map(longest_string)
            .max_by_key(String::len),
        _ => None,
    }
}

impl Interpreter for Exec {
    fn name(&self) -> String {
        format!("exec {}", self.cfg.isolation.name())
    }

    fn begin(&mut self, world: &World) -> Result<(), String> {
        let work = self.cfg.work.join(short_name(&self.id));
        let _ = host::remove_tree(&work);
        fs::create_dir_all(self.out_dir()).map_err(|e| e.to_string())?;
        self.laptops = world.laptops.len();
        let specs: Vec<(String, u32, bool, Option<String>)> = world
            .laptops
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let person = world.person_of(Laptop(i as u8)).map(|p| p.name.clone());
                (l.name.clone(), l.disk_mib, false, person)
            })
            .chain(
                world
                    .custodians
                    .iter()
                    .map(|c| (c.name.clone(), c.disk_mib, true, None)),
            )
            .collect();
        for (name, mib, custodian, person) in specs {
            let space = Space::create(self.cfg.isolation, &work, &name, mib, custodian)?;
            let root = space.root.clone();
            let home = root.join("home");
            let mut env = vec![
                (s("HOME"), home.display().to_string()),
                (
                    s("XDG_CONFIG_HOME"),
                    home.join(".config").display().to_string(),
                ),
                (
                    s("XDG_STATE_HOME"),
                    home.join(".local/state").display().to_string(),
                ),
                (
                    s("XDG_CACHE_HOME"),
                    home.join(".cache").display().to_string(),
                ),
                (
                    s("XDG_DATA_HOME"),
                    home.join(".local/share").display().to_string(),
                ),
                (s("XDG_RUNTIME_DIR"), root.join("run").display().to_string()),
                (s("PIGEON_TRACE"), s("1")),
                (s("PIGEON_NAME"), name.clone()),
                // Direct addresses and mDNS only (13 §13.12).
                (s("PIGEON_RELAYS"), s("off")),
                (s("RUST_LOG"), s("info")),
            ];
            // A laptop's person is its `$USER`: the creator's names them.
            if let Some(user) = person {
                env.push((s("USER"), user));
            }
            // `PIGEON_LOG=debug` for the run gives every daemon's debug log.
            if let Ok(level) = std::env::var("PIGEON_LOG") {
                env.push((s("PIGEON_LOG"), level));
            }
            if custodian && self.cfg.isolation.service_seam() {
                env.push((
                    s("PIGEON_LAB_SERVICE_ROOT"),
                    root.join("service").display().to_string(),
                ));
            }
            // A short socket path (a Unix socket's is at most 104 bytes), in
            // a directory of the device's own: pigeond links remotedaemon's
            // sockets beside its own.
            let sockets = std::env::temp_dir().join(format!(
                "pgl-{}-{}",
                std::process::id(),
                self.devs.len()
            ));
            let _ = host::remove_tree(&sockets);
            fs::create_dir_all(&sockets).map_err(|e| e.to_string())?;
            let socket = sockets.join("pigeond.sock");
            env.push((s("PIGEON_SOCKET"), socket.display().to_string()));
            // git-annex finds `git-annex-p2p-iroh` on the PATH.
            let path = std::env::var("PATH").unwrap_or_default();
            env.push((s("PATH"), format!("{}:{path}", self.cfg.bin.display())));
            for d in [home.clone(), root.join("run")] {
                fs::create_dir_all(&d).map_err(|e| e.to_string())?;
            }
            // As in pigeond/tests/common: exercise real rounds without
            // waiting for the normal 30-second idle interval at every write.
            // A service reads the directory bound into its namespace; a
            // person's config follows dirs::config_dir on each platform.
            let config = if custodian {
                root.join("service/var/lib/pigeon")
            } else if cfg!(target_os = "macos") {
                home.join("Library/Application Support/pigeon")
            } else {
                home.join(".config/pigeon")
            };
            fs::create_dir_all(&config).map_err(|e| e.to_string())?;
            fs::write(
                config.join("config.toml"),
                "[loops]\ninterval_seconds = 1\n",
            )
            .map_err(|e| e.to_string())?;
            let tree = home.join(format!("pigeon-{}", world.group));
            self.devs.push(Dev {
                name,
                space,
                env,
                daemon: None,
                up: false,
                plan: None,
                tree,
                restarts: 0,
                peak: 0,
            });
        }
        let admin = world.admin().ok_or("no admin")?;
        self.start(Device::Laptop(admin))?;
        let tree = self.devs[admin.0 as usize].tree.clone();
        fs::create_dir_all(&tree).map_err(|e| e.to_string())?;
        for f in &world.folders {
            fs::create_dir_all(tree.join(f)).map_err(|e| e.to_string())?;
        }
        let created = self.pigeon(
            Device::Laptop(admin),
            &cli::create(
                &world.group,
                &tree,
                world.k,
                &world.laptops[admin.0 as usize].name,
            ),
        )?;
        if !created.ok {
            return Err(format!("create: {:?}", created.outcome()));
        }
        for (j, c) in world.custodians.iter().enumerate() {
            if c.present {
                let d = Device::Custodian(crate::world::Custodian(j as u8));
                match self.join_custodian(admin, d, world)? {
                    Outcome::Done => {}
                    other => return Err(format!("{} could not join: {other:?}", c.name)),
                }
            }
        }
        Ok(())
    }

    fn step(&mut self, world: &World, step: &Step) -> Verdict {
        self.perform(world, step).unwrap_or_else(Verdict::Fail)
    }

    fn measures(&mut self) -> Vec<Measure> {
        let mut out = self.measures.clone();
        for d in &self.devs {
            out.push(Measure {
                device: d.name.clone(),
                label: "peak".into(),
                parts: BTreeMap::from([
                    (s("total"), d.peak),
                    (s("restarts"), u64::from(d.restarts)),
                ]),
            });
        }
        out
    }

    fn trace(&mut self) -> Option<Vec<Label>> {
        let admin = (0..self.laptops).find(|i| self.devs[*i].daemon.is_some())?;
        let machines = self
            .pigeon(Device::Laptop(Laptop(admin as u8)), &cli::plain("machines"))
            .ok()?;
        let mut ids = BTreeMap::new();
        for (name, id) in machines
            .json
            .as_ref()
            .map(cli::machines)
            .unwrap_or_default()
        {
            if let Some(i) = self.devs.iter().position(|d| d.name == name) {
                ids.insert(id, self.device_of(i));
            }
        }
        let files: Vec<PathBuf> = self
            .devs
            .iter()
            .flat_map(|d| trace::files(&d.space.root))
            .collect();
        let lines = trace::merged(&files);
        let _ = trace::write(&lines, &self.out_dir().join("trace.ndjson"));
        if lines.is_empty() {
            return None;
        }
        Some(trace::labels(&lines, &ids))
    }

    fn end(&mut self) {
        for i in 0..self.devs.len() {
            let d = self.device_of(i);
            self.signal(d, "TERM", 10);
            let log = self.devs[i].space.dir.join("pigeond.log");
            let _ = fs::copy(
                &log,
                self.out_dir().join(format!("{}.log", self.devs[i].name)),
            );
            for f in trace::logs(&self.devs[i].space.root) {
                if let Some(base) = f.file_name() {
                    let to = format!("{}-{}", self.devs[i].name, base.to_string_lossy());
                    let _ = fs::copy(&f, self.out_dir().join(to));
                }
            }
            self.devs[i].space.teardown();
        }
    }
}
