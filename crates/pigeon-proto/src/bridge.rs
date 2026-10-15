//! Local bridge operations. File bodies use private staging files, rather than
//! the bounded JSON socket. Revisions cover the peer's visible files, including
//! its captured suggestions and conflicts; apply is a compare-and-swap on that
//! revision under the same lock as capture and install.

use serde::{Deserialize, Serialize};

/// Maximum files in one catalogue page; the byte budget may shorten it further.
pub const MAX_PAGE_FILES: usize = 1000;
/// Maximum encoded page reply, including the socket envelope and cursor.
pub const MAX_PAGE_BYTES: usize = 256 * 1024;

/// A position in one revision. A missing path starts at the beginning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    /// The revision required for this page; a change returns status 412.
    pub revision: String,
    /// Continue strictly after this path, rather than an unstable numeric offset.
    pub after: Option<String>,
}

/// An operation on the daemon's default group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Operation {
    /// Advisory direct addresses of co-hosted peers. Admission still requires
    /// each remote's cryptographic identity and the group's signed roster.
    LearnPeers {
        /// Device IDs and directly reachable socket addresses.
        peers: std::collections::BTreeMap<String, Vec<String>>,
    },
    /// The identity used when provisioning another device of the same person.
    Info,
    /// Registers the pool with an existing node so newly received custodian
    /// objects reuse the same immutable bytes as the client peers.
    RegisterPool {
        /// The exact existing group.
        group: String,
        /// An absolute private pool directory on the node's filesystem.
        store: String,
    },
    /// Pins a verified immutable version from an existing storage node in a pool.
    ExportContent {
        /// The exact group; never inferred from the provider's default group.
        group: String,
        /// A SHA256 or SHA256E annex content key.
        key: String,
        /// Absolute shared pool directory, private to the same operating-system user.
        store: String,
    },
    /// One page of files visible to this peer, including pending suggestions.
    Snapshot {
        /// None starts a new listing. A cursor pins all pages to one revision.
        cursor: Option<Cursor>,
        /// Between 1 and `MAX_PAGE_FILES`; replies also obey `MAX_PAGE_BYTES`.
        limit: usize,
    },
    /// Pins a verified version at a new caller-owned staging path. Shared
    /// storage links immutable bytes; ordinary working peers copy them.
    Read {
        /// A relative, portable file path.
        path: String,
        /// The version returned by `Snapshot`.
        etag: String,
        /// A new absolute filename; must not exist.
        destination: String,
    },
    /// Records an explicit save or batch of changes, then captures it.
    Apply {
        /// The revision returned by `Snapshot`.
        revision: String,
        /// Optional per-path versions for a single-file save. An unrelated
        /// catalogue change may be rebased only when these still match.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected: Option<std::collections::BTreeMap<String, Option<String>>>,
        /// Changes captured together, so moves retain rename information.
        changes: Vec<Change>,
    },
}

/// One explicit save or deletion. All paths are relative to the shared tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Change {
    /// Replace a file with the complete contents of a private staging file.
    Put {
        /// A portable path in the shared tree.
        path: String,
        /// An absolute path readable by the daemon, outside the shared tree.
        staged: String,
    },
    /// Delete a file, retaining its captured content in history.
    Remove {
        /// The path to delete.
        path: String,
    },
}

/// A file in this peer's visible tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    /// Relative path, never Pigeon or Git internals.
    pub path: String,
    /// Opaque version token, without HTTP quotes.
    pub etag: String,
    /// Content size in bytes.
    pub size: u64,
    /// Whether the version is this peer's unresolved divergence.
    pub diverging: bool,
}

/// One bounded page of a consistent view of the peer's tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Changes whenever a visible file's path, version or divergence changes.
    pub revision: String,
    /// Sorted by relative path.
    pub files: Vec<File>,
    /// None means the listing is complete, including for an empty tree.
    pub next: Option<Cursor>,
}

/// Results of local bridge operations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Reply {
    /// Advisory direct addresses were recorded.
    Learned,
    /// A verified immutable copy is retained at this absolute path.
    Content {
        /// The canonical content file in the requested pool.
        path: String,
    },
    /// A file operation failed without claiming HTTP success.
    Failure {
        /// HTTP status appropriate for the operation.
        status: u16,
        /// Explanation for the bridge user.
        message: String,
    },
    /// The actual, roster-checked identity of this daemon.
    Info {
        /// Group ID.
        group: String,
        /// The owning person's immutable roster ID.
        person: String,
        /// This peer's signing and transport public key.
        device: String,
        /// This peer's distinct annex storage identity.
        annex_uuid: String,
        /// Absolute working-tree path, for local conflict resolution requests.
        root: String,
        /// The endpoint's direct addresses, used for co-hosted peer discovery.
        addresses: Vec<String>,
        /// Whether this peer uses the shared pool and sparse client overlays.
        #[serde(default)]
        shared: bool,
    },
    /// One page of a consistent visible tree.
    Snapshot(Snapshot),
    /// A verified copy is ready at the requested destination.
    Read,
    /// Every change was captured locally; replication is asynchronous.
    Applied {
        /// The resulting revision. Fetch the catalogue in bounded pages.
        revision: String,
        /// How many changed paths became suggestions awaiting a decision.
        suggested: usize,
    },
}
