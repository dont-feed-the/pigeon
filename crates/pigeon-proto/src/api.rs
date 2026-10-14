//! The requests of pigeond's internal API and their responses (10 §10.1).
//!
//! The safety slice (13 §13.12) carries the requests of its commands and the
//! three of the transport helper (09 §9.3); the others are answered with the
//! problem `not-in-this-build`. Text for people is a [`Message`]: an id of
//! the Fluent bundles and its arguments (ADR 0028).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One request, named as in 10 §10.1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
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
    },
    /// `pigeon invite [--admin] [--backup-machine] PERSON`.
    Invite {
        /// Who is invited.
        person: String,
        /// As an admin.
        #[serde(default)]
        admin: bool,
        /// As a backup machine (custodian).
        #[serde(default)]
        custodian: bool,
        /// The new backup machine's name in the roster, with `custodian`;
        /// its person is then the inviter's (09 §9.6).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// `pigeon join INVITATION [FOLDER]`.
    Join {
        /// The `pgn1-` invitation.
        invitation: String,
        /// Where the folder goes; none on a backup machine.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
        /// Keep the files on this computer.
        #[serde(default)]
        keep: bool,
    },
    /// `pigeon keep PATH`.
    Keep {
        /// A folder or file.
        folder: String,
    },
    /// `pigeon free PATH`.
    Evict {
        /// A folder or file.
        folder: String,
    },
    /// `pigeon history PATH`.
    History {
        /// A file.
        path: String,
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
    /// `pigeon remove-computer NAME`.
    RemoveDevice {
        /// The computer's name in the roster.
        name: String,
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
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
    },
    /// The answer to `History`.
    History {
        /// Newest first.
        versions: Vec<VersionStatus>,
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// The id, such as `problem-invite-expired`.
    pub id: String,
    /// Named arguments.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Arg>,
}

/// An argument of a [`Message`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Reached a custodian, not yet K domains.
    Published,
    /// Safe: on K failure domains.
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
}

/// A path that is not shared, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotShared {
    /// The path in the folder.
    pub path: String,
    /// Why, such as a symbolic link.
    pub reason: Message,
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
    /// When it was last heard of, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heard_at: Option<String>,
    /// Whether it is this computer.
    #[serde(default)]
    pub this_device: bool,
}
