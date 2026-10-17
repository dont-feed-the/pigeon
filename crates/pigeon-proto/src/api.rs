//! The requests of pigeond's internal API and their responses (10 §10.1).
//!
//! The safety slice (13 §13.12) carries the requests of its commands and the
//! three of the transport helper (09 §9.3); the others are answered with the
//! problem `not-in-this-build`. Text for people is a [`Message`]: an id of
//! the Fluent bundles and its arguments (ADR 0028).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One request, named as in 10 §10.1.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    /// Explicit erasure review, readiness, confirmation and cancellation.
    Erase {
        /// Operation on the selected/default group.
        operation: crate::erase::Operation,
    },
    /// View direct rules, or the effective rule of one local folder.
    FolderRules {
        /// Local folder, or all rules of the default group.
        folder: Option<String>,
    },
    /// Admin configuration of a signed folder rule (04 §4.3).
    SetFolderRule {
        /// The local folder.
        folder: String,
        /// New rule or inheritance.
        rule: crate::policy::Change,
        /// The person confirmed raising the group's `rules` (ADR 0048).
        #[serde(default)]
        confirmed: bool,
    },
    /// Group health from accepted state and signed reports (ADR 0025).
    Health,
    /// Read-only distinct-key storage need and signed disk observations.
    Capacity,
    /// Local file operations for the Nextcloud bridge, serialized by the daemon.
    Bridge {
        /// The operation on this daemon's default group.
        operation: crate::bridge::Operation,
    },
    /// The first request of a connection: the client's protocol version.
    Hello {
        /// The client's [`crate::PROTOCOL_VERSION`].
        version: u32,
    },
    /// `pigeon status [PATH] [--not-shared]`.
    Status {
        /// A folder or file, else every folder.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        /// List what is not shared (07 §7.9).
        #[serde(default)]
        not_shared: bool,
    },
    /// `pigeon create NAME`.
    Create {
        /// The group's name.
        name: String,
        /// The folder to share.
        folder: String,
        /// Copies on distinct custodians before a version is safe.
        k: u32,
        /// This computer's name in the roster.
        device_name: String,
        /// The creator's name as the others see it (`--person`); absent
        /// from an older client, and then the daemon's `$USER`, else the
        /// computer's name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        person: Option<String>,
    },
    /// `pigeon invite`: the line to share, which joins the group (09 §9.6).
    Invite,
    /// `pigeon join INVITATION [FOLDER] [--person NAME] [--name NAME]
    /// [--backup-machine]`.
    Join {
        /// The `pgn2-` invitation.
        invitation: String,
        /// Where the folder goes; none on a backup machine.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
        /// Keep the files on this computer.
        #[serde(default)]
        keep: bool,
        /// This computer's name in the roster, else `<host name>-<6 hex
        /// digits of its device ID>`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// The name its person gives, else the account's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        person: Option<String>,
        /// Whether it may join the person of the group with that name.
        #[serde(default)]
        existing: bool,
        /// As a backup machine (custodian).
        #[serde(default)]
        custodian: bool,
    },
    /// `pigeon keep PATH`.
    Keep {
        /// A folder or file.
        folder: String,
    },
    /// `pigeon free PATH` (08 §8.5). While no backup machine has been
    /// heard from recently, it answers the problem `backups-quiet` unless
    /// `confirmed`.
    Evict {
        /// A folder or file.
        folder: String,
        /// The person was told that the backup machines have not been
        /// heard from recently, and frees anyway.
        #[serde(default)]
        confirmed: bool,
    },
    /// What *Free up space* would do, for its confirmation (08 §8.5):
    /// answered with `FreePreview`.
    FreePreview {
        /// A folder or file.
        folder: String,
    },
    /// `pigeon free --without-checking PATH` (08 §8.7): says the risk, and
    /// drops the content even if it is the last copy once `confirmed`.
    Drop {
        /// A folder or file.
        path: String,
        /// The person saw the risk and confirmed.
        #[serde(default)]
        confirmed: bool,
    },
    /// `pigeon view PATH [--version N | --group]` (07 §7.6): a read-only
    /// copy of a version, outside the folder.
    View {
        /// A file.
        path: String,
        /// Version N of the history, from 1, the oldest; the current one
        /// when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<u64>,
        /// The group's version rather than the person's.
        #[serde(default)]
        group: bool,
    },
    /// A read-only copy of one changed file in a pending suggestion.
    ViewSuggestion {
        /// The pending suggestion's ID (unambiguous prefix allowed).
        proposal: String,
        /// A changed path, relative to the group or absolute within it.
        path: String,
    },
    /// `pigeon history PATH`.
    History {
        /// A file.
        path: String,
    },
    /// The start of the accepted current version of a file, as text, for
    /// the web's folder page (a README shown under its files). Read only
    /// from content already on this computer; never fetched.
    ReadText {
        /// A file of the group, as an absolute path.
        path: String,
        /// At most this many bytes, which the daemon caps.
        limit: u64,
    },
    /// `pigeon restore PATH [--version N | --at TIME]`.
    Restore {
        /// A file or folder.
        path: String,
        /// A version number from `history`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<u64>,
        /// A time, RFC 3339.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<String>,
    },
    /// `pigeon machines`.
    Machines,
    /// `pigeon people`: the group's members, with their computers (04 §4.2).
    People,
    /// `pigeon open`: a link to the web interface holding a single-use
    /// token (10 §10.3).
    OpenWeb,
    /// The group's folders, for the web's Files page (ux.md §6.2).
    Folders,
    /// `pigeon sync [PATH]`: every folder, under one if given, and whether
    /// this computer keeps it (08 §8.5).
    Sync {
        /// Only this folder and those in it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    /// `pigeon sync PATH --default`: forgets this computer's choices of a
    /// folder and inside it, which then follows its default again.
    ForgetChoice {
        /// The folder.
        folder: String,
    },
    /// The files under a folder, for the web's folder page (ux.md §6.2).
    Files {
        /// The folder.
        folder: String,
    },
    /// `pigeon deleted [PATH]`: the files deleted from the group, for the
    /// web's Deleted files (ux.md §6.2).
    Deleted {
        /// Only those under this folder.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    /// `pigeon settings [KEY]`: this computer's settings (11 §11.2).
    GetSettings {
        /// Only this one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    /// `pigeon settings KEY VALUE`: changes one setting in `config.toml`.
    SetSettings {
        /// The key, such as `language`.
        key: String,
        /// Its new value, as written after `=` without quotes.
        value: String,
    },
    /// `pigeon pause [DURATION]`: all but capture waits until the time
    /// ends, or until `Resume` without one (15 §15.6).
    Pause {
        /// Such as `45m`, `2h` or `3d`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration: Option<String>,
    },
    /// `pigeon resume`.
    Resume,
    /// The pause in force, for the banner of every web page (15 §15.6):
    /// `Said` with its words, or `Done` while sharing.
    Paused,
    /// `pigeon remove-computer NAME`.
    RemoveDevice {
        /// The computer's name in the roster.
        name: String,
    },
    /// Removes exactly this signing key without resolving a computer label.
    /// Already absent keys succeed, so bridge revocation can safely retry.
    RemoveDeviceId {
        /// The computer's device ID in the roster.
        device: String,
    },
    /// `pigeon held`: the held mass deletions and names (07 §7.4).
    Held,
    /// `pigeon held ID --share | --bring-back | --rename`.
    ReleaseHeld {
        /// The batch's ID, from `Held` (`id` is the envelope's).
        batch: String,
        /// What the person chose.
        choice: HeldChoice,
    },
    /// `pigeon leave`: this computer signs its own removal and forgets the
    /// group once another machine has it (09 §9.7).
    Leave,
    /// `pigeon refound [REPOSITORY]`: signs a new genesis with `main`'s
    /// tree and this device's person as the only admin (09 §9.9, ADR 0047).
    Refound {
        /// The group's repository; the default group when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        repository: Option<String>,
    },
    /// `pigeon recovery-sheet`.
    RecoverySheet,
    /// `pigeon recover SHEET [FOLDER] [--from DEVICE]`.
    Recover {
        /// The recovery sheet's text.
        sheet: String,
        /// Where the folder goes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
        /// The custodian to recover from.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
    /// `pigeon suggestions`: the pending proposals (04 §4.5).
    Proposals,
    /// `pigeon accept ID [--override REASON]`: adopt a proposal.
    Accept {
        /// The proposal's commit, or a prefix of it (`id` names the
        /// envelope's request).
        proposal: String,
        /// The Force reason (04 §4.6), when nobody may make the change.
        #[serde(default, rename = "override", skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// `pigeon decline ID [--note TEXT]`: reject a proposal.
    Reject {
        /// The proposal's commit, or a prefix of it.
        proposal: String,
        /// Why, for its author.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// `pigeon accept ID PATH…` · `pigeon decline ID PATH… [--note TEXT]`:
    /// one decision on every file of a proposal, the files named accepted
    /// and the others declined, or the reverse (ADR 0077). A new request,
    /// so that an older `pigeond` refuses it rather than deciding the
    /// whole proposal.
    DecideFiles {
        /// The proposal's commit, or a prefix of it.
        proposal: String,
        /// Files of the proposal, relative to the group or absolute within
        /// it; a file renamed counts with its old name.
        paths: Vec<String>,
        /// Whether the files named are accepted (and the others declined),
        /// or declined (and the others accepted).
        accept: bool,
        /// Why some are declined, for its author.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        /// The Force reason (04 §4.6), when nobody may make a change it
        /// accepts.
        #[serde(default, rename = "override", skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// `pigeon conflicts`: the paths whose file differs from the group's
    /// version on this computer (07 §7.6).
    Conflicts,
    /// `pigeon resolve PATH --take-group | --keep-copy DEST |
    /// --suggest-again` (07 §7.6).
    Resolve {
        /// The diverging file.
        path: String,
        /// What the person chose.
        choice: Choice,
    },
    /// `pigeon doctor [--details]`.
    Doctor {
        /// Show every check.
        #[serde(default)]
        details: bool,
    },
    /// `pigeon debug bundle [--from NAME…]`.
    DebugBundle {
        /// Peers asked for their part.
        #[serde(default)]
        peers: Vec<String>,
    },
    /// Allow one diagnostic bundle to this peer during the next ten minutes.
    AllowDebug {
        /// The peer's device ID or current name.
        peer: String,
    },
    /// The helper's `address` mode: this device's address for git-annex.
    Address {
        /// The repository the helper runs for (09 §9.2).
        repository: String,
    },
    /// The helper's connect mode: a stream to `device` for this repository.
    Connect {
        /// The repository the helper runs for (09 §9.2).
        repository: String,
        /// The peer's device ID, 64 hex digits.
        device: String,
    },
    /// The helper's listen mode: serve incoming peers on git-annex's socket.
    Listen {
        /// The repository the helper runs for (09 §9.2).
        repository: String,
        /// remotedaemon's unix socket.
        socket: String,
    },
}

/// One response; a request has one result, or progress then a result.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    /// Erasure review or explicit action result.
    Erasure {
        /// Shared typed result for the CLI and browser.
        answer: Box<crate::erase::Answer>,
    },
    /// Folder rules with human names and explicit edit authority.
    FolderRules(Box<crate::policy::Report>),
    /// Local group health, without an optional telemetry collector.
    Health(Box<crate::health::HealthReport>),
    /// Required storage and each backup machine's reported capacity.
    Capacity(Box<crate::capacity::Report>),
    /// A bridge operation's result; never sent over the peer protocol.
    Bridge {
        /// The result, or a normal `Problem` on failure.
        result: crate::bridge::Reply,
    },
    /// The answer to `Hello`.
    Hello {
        /// `pigeond`'s [`crate::PROTOCOL_VERSION`].
        version: u32,
    },
    /// Done, with nothing more to say.
    Done,
    /// Done, with something to tell the person.
    Said {
        /// What to say.
        message: Message,
    },
    /// Not done (10 §10.6).
    Problem(Problem),
    /// Progress of a long request.
    Progress {
        /// What is happening.
        message: Message,
        /// Bytes or items done, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        done: Option<u64>,
        /// Out of this many, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
    },
    /// The answer to `Status`.
    Status(StatusReport),
    /// The answer to `Machines`.
    Machines {
        /// The roster's devices.
        machines: Vec<Machine>,
        /// The backup machines a version needs to be safe (the roster's
        /// K); absent from an older daemon.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        copies: Option<u32>,
    },
    /// The answer to `Held`.
    Held {
        /// Oldest first.
        held: Vec<HeldEntry>,
    },
    /// The answer to `People`.
    People {
        /// The roster's persons.
        people: Vec<PersonEntry>,
    },
    /// The answer to `History`.
    History {
        /// Newest first.
        versions: Vec<VersionStatus>,
    },
    /// The answer to `ReadText`.
    Text {
        /// The first bytes of the file, as UTF-8 (invalid sequences
        /// replaced), cut at a character boundary.
        text: String,
        /// The whole file's size in bytes.
        size: u64,
        /// Whether `text` is only the start of the file.
        truncated: bool,
    },
    /// The answer to `Proposals`.
    Proposals {
        /// Oldest first.
        proposals: Vec<Suggestion>,
        /// This person's suggestions decided lately, newest first (ADR
        /// 0077).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        decided: Vec<Outcome>,
        /// The group's folder on this computer, which the paths are in.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        root: String,
    },
    /// The answer to `Folders`.
    Folders {
        /// The group's folder on this computer.
        root: String,
        /// Its top-level folders, by path.
        folders: Vec<Folder>,
    },
    /// The answer to `FreePreview`.
    FreePreview(FreePreview),
    /// The answer to `Sync`.
    Sync {
        /// The group's folder on this computer.
        root: String,
        /// The folders, parents before their children.
        folders: Vec<SyncFolder>,
    },
    /// The answer to `Files`.
    Files {
        /// The group's folder on this computer.
        root: String,
        /// The files under the folder, by path.
        files: Vec<FileEntry>,
    },
    /// The answer to `Deleted`.
    Deleted {
        /// The group's folder on this computer.
        root: String,
        /// Newest deletion first.
        files: Vec<DeletedFile>,
    },
    /// The answer to `GetSettings`.
    Settings {
        /// The settings a person may change, in the order shown.
        settings: Vec<Setting>,
    },
    /// The answer to `Conflicts`.
    Conflicts {
        /// By path.
        conflicts: Vec<Divergence>,
    },
    /// The answer to `Address`.
    Address {
        /// This device's ID, which git-annex prefixes with `iroh+`.
        device: String,
    },
    /// The answer to `Connect`: raw bytes of the peer's git-annex follow.
    Connected,
    /// The answer to `Listen`: the connection stays open while registered.
    Listening,
}

/// Text for people: a message id of the Fluent bundles, and its arguments.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// The id, such as `problem-person-exists`.
    pub id: String,
    /// Named arguments.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Arg>,
}

/// An argument of a [`Message`].
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    /// A number, formatted for the person's language.
    Number(i64),
    /// A name, a path or a time in RFC 3339.
    Text(String),
}

/// An error that reaches a person (10 §10.6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    /// Stable, kebab-case, documented in `docs/problems.md`.
    pub code: String,
    /// What went wrong.
    pub message: Message,
    /// What to do about it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<Message>,
    /// For a debug bundle, never shown first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// The state of a version (06 §6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Only on this computer.
    Local,
    /// Reached a custodian, not yet K of them.
    Published,
    /// Safe: on K custodians.
    Protected,
    /// No device is known to hold its content.
    Missing,
    /// Released by a purge.
    Erased,
}

/// One version of a file, with its state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionStatus {
    /// The path in the folder.
    pub path: String,
    /// The number `history` and `restore` use.
    pub number: u64,
    /// When it was saved, RFC 3339.
    pub saved_at: String,
    /// Its state.
    pub state: State,
    /// When that state was last observed, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    /// Why it is not yet protected (06 §6.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<Message>,
}

/// The level of a headline (06 §6.5), which scripts read from `--json`
/// (10 §10.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// Something only the person can do.
    NeedsYou,
    /// A version is not protected, and nothing is under way to fix it.
    NotSafe,
    /// The group has not been reached recently.
    NotChecked,
    /// Versions are on their way to protection.
    Working,
    /// Every version is protected.
    Safe,
}

/// The answer to `Status`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    /// The one sentence that sums it up (06 §6.5).
    pub headline: Message,
    /// Its level.
    pub level: Level,
    /// The worst state among the versions listed.
    pub state: State,
    /// Versions not yet protected, and those asked for.
    pub versions: Vec<VersionStatus>,
    /// Paths not shared, with why (07 §7.9).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_shared: Vec<NotShared>,
    /// The pause in force, said even when the headline is about something
    /// else (15 §15.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused: Option<Message>,
}

/// A path that is not shared, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotShared {
    /// The path in the folder.
    pub path: String,
    /// Why, such as a symbolic link.
    pub reason: Message,
}

/// A person of the roster.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonEntry {
    /// Their name in the roster.
    pub name: String,
    /// Whether they are an admin.
    pub admin: bool,
    /// Whether this device belongs to them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub this_person: bool,
    /// An admin with one signing computer and no paper key.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recovery_recommended: bool,
    /// Their paper keys name a different set of backup machines.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recovery_refresh_recommended: bool,
    /// The names of their computers and backup machines.
    pub machines: Vec<String>,
}

/// A device of the roster.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
    /// Its name in the roster.
    pub name: String,
    /// Its device ID.
    pub device: String,
    /// Whether it is a custodian (backup machine).
    pub custodian: bool,
    /// A paper recovery key; it never holds content or appears offline.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recovery: bool,
    /// When it was last heard of, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heard_at: Option<String>,
    /// Whether it is this computer.
    #[serde(default)]
    pub this_device: bool,
}

/// What removing a device of the roster does, for the question asked
/// before it is done (09 §9.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    /// A computer: its files stay on it but are shared no more.
    Computer,
    /// A backup machine: `left` stay, and versions need `needed` to be
    /// safe.
    Backup {
        /// The backup machines left after it.
        left: u32,
        /// The roster's K.
        needed: u32,
    },
    /// A paper recovery sheet, revoked.
    Sheet,
}

impl Removal {
    /// The removal of `name`, from the answers to `Machines`; a computer
    /// when they do not say otherwise. K is 1 from an older daemon that
    /// does not tell it, so that removing the last backup machine still
    /// warns.
    pub fn of(answers: &[Response], name: &str) -> Removal {
        for answer in answers {
            let Response::Machines { machines, copies } = answer else {
                continue;
            };
            let Some(m) = machines.iter().find(|m| m.name == name) else {
                continue;
            };
            if m.recovery {
                return Removal::Sheet;
            }
            if m.custodian {
                let others = machines
                    .iter()
                    .filter(|o| o.custodian && !o.recovery && o.name != name)
                    .count();
                return Removal::Backup {
                    left: u32::try_from(others).unwrap_or(u32::MAX),
                    needed: copies.unwrap_or(1),
                };
            }
        }
        Removal::Computer
    }
}

/// A top-level folder of the group (ux.md §6.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Its path in the group's folder.
    pub path: String,
    /// The files under it.
    pub files: u64,
    /// Those not safe yet.
    pub not_safe: u64,
    /// The least safe state of its files.
    pub state: State,
    /// Whether its space is freed on this computer (or is being freed).
    pub freed: bool,
    /// Requested local files that have not arrived or been installed yet.
    #[serde(default)]
    pub arriving: u64,
    /// The size in bytes of its files' current versions, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// A file of the group, with the state of its current version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Its path in the group's folder.
    pub path: String,
    /// The state of its current version.
    pub state: State,
    /// The size in bytes of its current version, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// When the copy on this computer last changed, RFC 3339; absent when
    /// the file is not on this computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    /// Its space is freed: it is not on this computer (08 §8.5).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub freed: bool,
}

/// What freeing a folder's space would do (08 §8.5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreePreview {
    /// The files under it.
    pub files: u64,
    /// Those safe on the backup machines, which leave this computer.
    pub safe: u64,
    /// The names of the group's backup machines.
    pub backups: Vec<String>,
    /// When a backup machine was last heard from, RFC 3339; none if never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heard_at: Option<String>,
    /// Whether that is too long ago to confirm their copies now; freeing
    /// then asks again (`Evict {confirmed}`).
    pub quiet: bool,
}

/// A folder and whether this computer keeps it (08 §8.5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncFolder {
    /// Its path in the group's folder.
    pub path: String,
    /// Whether its files are on this computer (or are coming).
    pub kept: bool,
    /// Whether this computer chose for this folder itself, rather than
    /// following the folder holding it or the default.
    pub chosen: bool,
    /// Another person's folder of a per-person folder, which this computer
    /// does not keep unless it chooses to: its owner's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub others: Option<String>,
}

/// A file deleted from the group, which `Restore` brings back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedFile {
    /// Its path in the group's folder.
    pub path: String,
    /// When it was deleted, RFC 3339.
    pub deleted_at: String,
}

/// One setting of `config.toml` (11 §11.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    /// Its key, such as `language`.
    pub key: String,
    /// Its value.
    pub value: String,
    /// The values it may take.
    pub choices: Vec<String>,
}

/// A pending proposal (04 §4.5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    /// Its commit.
    pub id: String,
    /// Its author's name.
    pub author: String,
    /// The name of the computer it was saved on.
    pub device: String,
    /// When it was saved, RFC 3339.
    pub saved_at: String,
    /// The paths it changes.
    pub paths: Vec<String>,
    /// Whether this computer's person decides it.
    #[serde(default)]
    pub decide: bool,
    /// Whether adopting it needs an admin's override (04 §4.6).
    #[serde(default)]
    pub force: bool,
    /// Whether it is this person's.
    #[serde(default)]
    pub mine: bool,
    /// What it changes, file by file: a rename is one file (ADR 0077).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SuggestedFile>,
}

/// What a suggestion changes in one file (ADR 0077).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestedFile {
    /// Its path in the group's folder; for a rename, the new one.
    pub path: String,
    /// How it changes.
    pub change: FileChange,
    /// For a rename, the path it had.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Its size before, in bytes; absent for a new file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<u64>,
    /// Its size after, in bytes; absent for a deletion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    /// For a small text file whose two versions this computer holds, the
    /// lines added and the lines removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<(u64, u64)>,
}

/// How a suggestion changes a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileChange {
    /// A new file.
    Added,
    /// Its content changes.
    Changed,
    /// It is deleted.
    Deleted,
    /// It moves to a new name, its content unchanged.
    Renamed,
}

/// A suggestion of this person that was decided (ADR 0077): which of its
/// files were accepted and which declined, by whom, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// The proposal's commit.
    pub id: String,
    /// The name of the person who decided.
    pub by: String,
    /// When, RFC 3339.
    pub decided_at: String,
    /// The files now in the group's version.
    #[serde(default)]
    pub accepted: Vec<String>,
    /// The files declined.
    #[serde(default)]
    pub declined: Vec<String>,
    /// The decider's note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A held batch of deletions (07 §7.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldEntry {
    /// Its ID, for `ReleaseHeld`.
    pub id: String,
    /// Why it is held, in words.
    pub reason: Message,
    /// How many files.
    pub files: u64,
    /// Since when, RFC 3339.
    pub since: String,
    /// Its first paths, at most ten.
    pub paths: Vec<String>,
    /// For a non-portable name, the name pigeon suggests; the choice is
    /// then `Rename`, or the person renames it themselves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested: Option<String>,
}

/// What the person does with a held batch (07 §7.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeldChoice {
    /// *Delete them for everyone*.
    Share,
    /// *Bring them back*.
    BringBack,
    /// *Rename to the suggested name*, for a non-portable name (05 §5.4).
    Rename,
}

/// A resolution of a divergence (07 §7.6, ux.md §8.8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Choice {
    /// *Take the group's version*.
    TakeGroup,
    /// *Keep mine as a copy* at `dest`, then take the group's version.
    KeepCopy {
        /// Where the copy goes.
        dest: String,
    },
    /// *Suggest again*: a new proposal on the current `main`.
    SuggestAgain,
}

/// A path whose file on this computer differs from the group's version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Divergence {
    /// The path in the folder.
    pub path: String,
    /// `suggestion`, `concurrent` or `recovered`.
    pub kind: String,
    /// `waiting` (for the deciders) or `conflict` (the person chooses).
    pub state: String,
    /// When it started, RFC 3339.
    pub since: String,
}
