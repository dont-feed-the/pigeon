//! The steps of a scenario, its conditions and the outcomes it expects.
//!
//! Each operation belongs to the role that may do it: [`LaptopOp`] to a
//! laptop, [`CustodianOp`] to a custodian, [`DeviceOp`] to any device. A
//! step that names a laptop operation for a custodian cannot be built.
//! Conditions are values too: [`Step::Expect`] checks one now and
//! [`Step::Wait`] waits for one until its deadline.

use pigeon_core::headline::Level;
use pigeon_core::state::{self, State};
use serde::{Deserialize, Serialize};

use crate::world::{Content, Custodian, Device, File, Folder, Laptop};

/// What a laptop does: what its person does, and the requests they make.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaptopOp {
    /// The person saves `content` at `file` (creating its directories).
    Write {
        /// The file.
        file: File,
        /// What it holds after.
        content: Content,
    },
    /// Someone outside the group puts a commit on this laptop's `main`: it
    /// adds `file`, holding `forged`, and is signed by a key outside the
    /// roster. No request: git writes it beside pigeond (04 §4.4).
    Forge {
        /// The file it adds.
        file: File,
    },
    /// The person makes a symbolic link, which pigeon does not share.
    Symlink {
        /// The link.
        file: File,
        /// Where it points.
        target: String,
    },
    /// `pigeon free FOLDER`.
    Free {
        /// The folder.
        folder: Folder,
    },
    /// `pigeon keep FOLDER`.
    Keep {
        /// The folder.
        folder: Folder,
    },
    /// `pigeon restore FILE --version N` (numbers from 1, oldest first).
    Restore {
        /// The file.
        file: File,
        /// The version number.
        version: u64,
    },
    /// `pigeon invite`, then the custodian's `pigeon join --backup-machine`
    /// with that invitation.
    Invite {
        /// The spare custodian that joins.
        custodian: Custodian,
    },
    /// `pigeon remove-computer NAME --yes`.
    RemoveComputer {
        /// The custodian removed.
        custodian: Custodian,
    },
    /// `pigeon recovery-sheet`: the sheet is kept for [`LaptopOp::Recover`].
    RecoverySheet,
    /// `pigeon recover SHEET --yes` on this fresh laptop, with the sheet
    /// `from` printed.
    Recover {
        /// The laptop whose recovery sheet is used.
        from: Laptop,
    },
    /// `pigeon invite`, then the spare laptop's `pigeon join INVITATION
    /// FOLDER --name NAME --person PERSON` (09 §9.6).
    Join {
        /// The spare laptop that joins.
        laptop: Laptop,
    },
    /// `pigeon accept ID` for the newest suggestion that changes `file`
    /// (04 §4.5).
    Accept {
        /// The file.
        file: File,
    },
    /// `pigeon decline ID` for the newest suggestion that changes `file`.
    Decline {
        /// The file.
        file: File,
    },
    /// `pigeon resolve FILE --take-group`: the group's version replaces the
    /// person's diverging file (07 §7.6).
    TakeGroup {
        /// The file.
        file: File,
    },
}

/// What only a custodian undergoes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustodianOp {
    /// Its disk is destroyed: the daemon is killed, the image is made anew,
    /// and the daemon starts on it with its configuration but no repository.
    Destroy,
}

/// What any device undergoes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceOp {
    /// Start its daemon.
    Start,
    /// Stop its daemon cleanly.
    Stop,
    /// Kill its daemon with `SIGKILL`.
    Kill,
    /// Restart its daemon with `PIGEON_LAB_CRASH=point:n`; once it crashes
    /// there, its supervisor restarts it without the plan.
    Crash {
        /// The named point.
        at: CrashPoint,
        /// Which crossing crashes, from 1.
        n: u32,
    },
    /// Fill its disk with a ballast file, leaving `leave_mib` MiB free.
    Fill {
        /// Free space left, in MiB.
        leave_mib: u32,
    },
    /// Remove the ballast.
    Unfill,
}

/// The named crash points of `pigeond` (`pigeon_sys::lab::crash_point`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CrashPoint {
    /// Capture hashed and committed, before the ref moves.
    CaptureBeforeRef,
    /// Free journalled `begin`.
    FreeAfterBegin,
    /// Free moved the file aside.
    FreeAfterMoveAside,
    /// Free journalled `proved`.
    FreeAfterProved,
    /// Free wrote the index.
    FreeAfterIndex,
    /// Keep's install journalled `begin`.
    KeepAfterBegin,
    /// Keep's install renamed the file in.
    KeepAfterRenameIn,
}

impl CrashPoint {
    /// Every point, in the order of the loops.
    pub const ALL: [CrashPoint; 7] = [
        CrashPoint::CaptureBeforeRef,
        CrashPoint::FreeAfterBegin,
        CrashPoint::FreeAfterMoveAside,
        CrashPoint::FreeAfterProved,
        CrashPoint::FreeAfterIndex,
        CrashPoint::KeepAfterBegin,
        CrashPoint::KeepAfterRenameIn,
    ];

    /// The name `pigeond` gives it.
    pub fn name(self) -> &'static str {
        match self {
            CrashPoint::CaptureBeforeRef => "capture-before-ref",
            CrashPoint::FreeAfterBegin => "free-after-begin",
            CrashPoint::FreeAfterMoveAside => "free-after-move-aside",
            CrashPoint::FreeAfterProved => "free-after-proved",
            CrashPoint::FreeAfterIndex => "free-after-index",
            CrashPoint::KeepAfterBegin => "keep-after-begin",
            CrashPoint::KeepAfterRenameIn => "keep-after-rename-in",
        }
    }
}

/// The observable kind of a version's state (06 §6.2), as `pigeon status
/// --json` reports it and as [`pigeon_core::state::state`] computes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// No device is known to hold it.
    Missing,
    /// Only this device holds it.
    Local,
    /// Some other device has it, not yet K custodians.
    Published,
    /// Would be protected, but nothing heard recently.
    Uncertain,
    /// K custodians hold it.
    Protected,
}

impl Kind {
    /// The kind of a state of pigeon-core; its order is [`state::rank`]'s.
    pub fn of<D>(s: &State<D>) -> Kind {
        match state::rank(s) {
            0 => Kind::Missing,
            1 => Kind::Local,
            2 => Kind::Published,
            3 => Kind::Uncertain,
            _ => Kind::Protected,
        }
    }
}

/// The headline levels' names in `pigeon status --json` (10 §10.2).
pub mod level_name {
    use pigeon_core::headline::Level;
    use serde::{Deserialize, Deserializer, Serializer};

    /// The name of a level.
    pub fn of(level: Level) -> &'static str {
        match level {
            Level::Safe => "safe",
            Level::Working => "working",
            Level::NotCheckedRecently => "not-checked",
            Level::NotSafeYet => "not-safe",
            Level::NeedsYou => "needs-you",
        }
    }

    /// The level of a name.
    pub fn parse(name: &str) -> Option<Level> {
        [
            Level::Safe,
            Level::Working,
            Level::NotCheckedRecently,
            Level::NotSafeYet,
            Level::NeedsYou,
        ]
        .into_iter()
        .find(|l| of(*l) == name)
    }

    /// Serde: a level as its name.
    pub fn serialize<S: Serializer>(level: &Level, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(of(*level))
    }

    /// Serde: a level from its name.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Level, D::Error> {
        let name = String::deserialize(d)?;
        parse(&name).ok_or_else(|| serde::de::Error::custom(format!("no level {name:?}")))
    }
}

/// Something a scenario observes. Conditions about progress
/// ([`Condition::Transferring`]) and conflict copies
/// ([`Condition::Versions`], [`Condition::Kept`]) are seen by the process
/// executor only; the model skips them, and a wait for one lets no time
/// pass in the model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    /// The laptop reports the file's current version in this state.
    State {
        /// Who looks.
        on: Laptop,
        /// The file.
        file: File,
        /// The expected kind.
        is: Kind,
    },
    /// The laptop's headline is at this level.
    Headline {
        /// Who looks.
        on: Laptop,
        /// The level.
        #[serde(with = "level_name")]
        is: Level,
    },
    /// The file is on the laptop's disk with exactly these bytes (SHA-256).
    Bytes {
        /// Who looks.
        on: Laptop,
        /// The file.
        file: File,
        /// The bytes.
        content: Content,
    },
    /// The file is absent from the laptop's disk, with no pointer in its place.
    Absent {
        /// Who looks.
        on: Laptop,
        /// The file.
        file: File,
    },
    /// No file of the folder is a git-annex pointer file or symbolic link
    /// into `.git/annex` (spec 08 §8.5).
    NoPointers {
        /// Who looks.
        on: Laptop,
        /// The folder.
        folder: Folder,
    },
    /// `pigeon status --not-shared` lists the file.
    NotShared {
        /// Who looks.
        on: Laptop,
        /// The file.
        file: File,
    },
    /// A problem with this code is reported by the device (status, doctor).
    Says {
        /// Who says it.
        on: Device,
        /// The problem code (10 §10.6).
        code: String,
    },
    /// The custodian's repository holds the complete content of the file's
    /// current version (its own git-annex says so).
    Holds {
        /// The custodian.
        on: Custodian,
        /// The file.
        file: File,
    },
    /// A partial object of the file is on its way to the custodian
    /// (`.git/annex/tmp`); observed by the process executor only.
    Transferring {
        /// The receiver.
        to: Custodian,
        /// The file.
        file: File,
    },
    /// The laptop's disk use in steady state is at most `permille` / 1000 of
    /// the content it keeps (13 §13.11).
    DiskWithin {
        /// Whose disk.
        on: Laptop,
        /// The bound, in thousandths of the content kept.
        permille: u32,
    },
    /// The file and its conflict copies (05 §5.3) on the laptop's disk
    /// hold exactly these contents, as a set; observed by the process
    /// executor only, as conflicts are Authority.tla's and the catalogue's.
    Versions {
        /// Who looks.
        on: Laptop,
        /// The file.
        file: File,
        /// The contents.
        contents: Vec<Content>,
    },
    /// The file or one of its conflict copies holds `content`: what the
    /// person saved there was not overwritten; process executor only.
    Kept {
        /// Who looks.
        on: Laptop,
        /// The file.
        file: File,
        /// The content.
        content: Content,
    },
    /// `pigeon suggestions` lists a suggestion that changes the file.
    Suggested {
        /// Who looks.
        on: Laptop,
        /// The file.
        file: File,
    },
    /// `pigeon machines` lists the device: its roster has it.
    Knows {
        /// Who looks.
        on: Laptop,
        /// The device listed.
        device: Device,
    },
    /// The negation.
    Not(Box<Condition>),
    /// Every one of them.
    All(Vec<Condition>),
}

impl Condition {
    /// Whether only the process executor can observe it.
    pub fn real_only(&self) -> bool {
        match self {
            Condition::Transferring { .. }
            | Condition::Versions { .. }
            | Condition::Kept { .. } => true,
            Condition::Not(c) => c.real_only(),
            Condition::All(cs) => cs.iter().any(Condition::real_only),
            _ => false,
        }
    }
}

/// What a request is expected to answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// It succeeds.
    #[default]
    Done,
    /// It fails with this problem code, before it changes anything.
    Refused(String),
}

/// One step of a scenario.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Step {
    /// A laptop does something and the request answers `expect`.
    Laptop {
        /// The laptop.
        on: Laptop,
        /// What it does.
        op: LaptopOp,
        /// The answer expected.
        expect: Outcome,
    },
    /// A custodian undergoes something.
    Custodian {
        /// The custodian.
        on: Custodian,
        /// What it undergoes.
        op: CustodianOp,
    },
    /// Any device undergoes something.
    Device {
        /// The device.
        on: Device,
        /// What it undergoes.
        op: DeviceOp,
    },
    /// The condition holds now.
    Expect(Condition),
    /// The condition holds within `within_s` seconds.
    Wait {
        /// The condition.
        until: Condition,
        /// The deadline, in seconds.
        within_s: u32,
    },
    /// The condition holds now and stays true for `for_s` seconds.
    Hold {
        /// The condition.
        cond: Condition,
        /// How long, in seconds.
        for_s: u32,
    },
    /// These steps run at once; each must meet its own expectation.
    Race(Vec<Step>),
    /// Disk use of the device is recorded under `label` (S11).
    Measure {
        /// The device.
        on: Device,
        /// The label in the report.
        label: String,
    },
}
