//! Every message id the code may name, with its variables, and the problem
//! codes (10 §10.6). The completeness test checks this list against each
//! bundle, so a renderer can check a message with [`crate::check`] in its
//! own tests rather than finding a missing id at run time.

/// One message: its id and the names of the variables it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The Fluent id.
    pub id: &'static str,
    /// The variables, in no particular order.
    pub vars: &'static [&'static str],
}

const fn e(id: &'static str, vars: &'static [&'static str]) -> Entry {
    Entry { id, vars }
}

/// The problem codes of the slice: stable, kebab-case, public (ADR 0023).
/// Each has a message `problem-<code>`, and may have a hint `hint-<code>`.
pub const PROBLEMS: &[&str] = &[
    "not-in-this-build",
    "file-system-not-in-this-build",
    "invite-expired",
    "invite-used",
    "invite-unknown",
    "invite-not-allowed",
    "inviter-offline",
    "not-in-group",
    "not-allowed",
    "needs-confirmation",
    "backup-machine-needs-linux",
    "disk-full",
    "same-domain",
    "independence-unknown",
    "repository-lost",
    "daemon-not-running",
    "version-mismatch",
    "git-annex-too-old",
    "git-annex-untested",
    "update-needed",
    "pigeon-too-old",
    "peer-too-old",
    "peer-too-new",
    "settings-check",
    "receiving",
    "unknown-computer",
    "usage",
    "not-a-pigeon-repository",
    "peer-unreachable",
    "peer-refused",
    "internal-error",
];

/// Every message of the catalogue.
pub const MESSAGES: &[Entry] = &[
    e("state-local", &[]),
    e("state-published", &[]),
    e("state-protected", &[]),
    e("state-uncertain", &["since"]),
    e("state-missing", &[]),
    e("state-erased", &[]),
    e("level-needs-you", &[]),
    e("level-not-safe", &[]),
    e("level-working", &[]),
    e("level-safe", &[]),
    e("level-not-checked", &[]),
    e("headline-safe", &["ago"]),
    e("headline-not-safe", &["files"]),
    e("headline-working", &["files"]),
    e("headline-not-checked", &["when"]),
    e("headline-disk-full", &[]),
    e("headline-machine-missing", &["machine", "days"]),
    e("why-machine-offline", &["machine", "since"]),
    e("why-safe-copies", &["have", "need"]),
    e("next-when-back", &["machine"]),
    e("next-repairing", &["files"]),
    e("time-just-now", &[]),
    e("time-minutes-ago", &["minutes"]),
    e("time-hours-ago", &["hours"]),
    e("time-days-ago", &["days"]),
    e("time-never", &[]),
    e("cmd-create", &[]),
    e("cmd-invite", &[]),
    e("cmd-join", &[]),
    e("cmd-status", &[]),
    e("cmd-machines", &[]),
    e("cmd-history", &[]),
    e("cmd-restore", &[]),
    e("cmd-free", &[]),
    e("cmd-keep", &[]),
    e("cmd-remove-computer", &[]),
    e("cmd-recovery-sheet", &[]),
    e("cmd-recover", &[]),
    e("cmd-doctor", &[]),
    e("cmd-debug-bundle", &[]),
    e("opt-json", &[]),
    e("opt-backup-machine", &[]),
    e("opt-not-shared", &[]),
    e("help-usage", &[]),
    e("help-commands", &[]),
    e("free-result", &["safe", "total"]),
    e("free-waiting", &["files"]),
    e("keep-result", &["folder"]),
    e("undo-free", &["folder"]),
    e("undo-keep", &["folder"]),
    e("restore-result", &["path"]),
    e("not-shared-kind", &["path", "kind"]),
    e("doctor-lab-build", &[]),
    e("doctor-version", &["program", "found", "minimum"]),
    e("doctor-ok", &[]),
    e("created", &["group"]),
    e("invitation", &["code"]),
    e("joined", &["group"]),
    e("removed-computer", &["name"]),
    e("bundle-written", &["path"]),
    e("machine-row", &["name", "kind", "heard"]),
    e("machine-this", &["name"]),
    e("machine-kind-custodian", &[]),
    e("machine-kind-computer", &[]),
    e("version-row", &["number", "saved", "state"]),
    e("waiting-row", &["path", "state"]),
    e("confirm-remove-computer", &["name"]),
    e("confirm-yes", &[]),
    e("problem-code", &["code"]),
    e("problem-not-in-this-build", &["command"]),
    e("hint-not-in-this-build", &[]),
    e("problem-file-system-not-in-this-build", &["fs"]),
    e("hint-file-system-not-in-this-build", &[]),
    e("problem-invite-expired", &["until"]),
    e("hint-invite-expired", &["inviter"]),
    e("problem-invite-used", &[]),
    e("hint-invite-used", &["inviter"]),
    e("problem-invite-unknown", &[]),
    e("problem-invite-not-allowed", &["inviter"]),
    e("problem-inviter-offline", &["inviter"]),
    e("hint-inviter-offline", &["inviter"]),
    e("problem-not-in-group", &[]),
    e("problem-not-allowed", &[]),
    e("problem-needs-confirmation", &[]),
    e("hint-needs-confirmation", &[]),
    e("problem-backup-machine-needs-linux", &[]),
    e("problem-disk-full", &["machine"]),
    e("hint-disk-full", &[]),
    e("problem-same-domain", &["machine", "other"]),
    e("problem-independence-unknown", &["machine"]),
    e("hint-independence-unknown", &[]),
    e("problem-repository-lost", &["machine"]),
    e("problem-daemon-not-running", &[]),
    e("hint-daemon-not-running", &[]),
    e("problem-version-mismatch", &[]),
    e("hint-version-mismatch", &[]),
    e("problem-git-annex-too-old", &["found", "minimum"]),
    e("problem-git-annex-untested", &["found"]),
    e("problem-update-needed", &[]),
    e("problem-pigeon-too-old", &[]),
    e("problem-peer-too-old", &["peer"]),
    e("problem-peer-too-new", &["peer"]),
    e("problem-settings-check", &[]),
    e("problem-receiving", &[]),
    e("hint-receiving", &[]),
    e("problem-unknown-computer", &["name"]),
    e("hint-unknown-computer", &[]),
    e("problem-usage", &["command"]),
    e("hint-usage", &[]),
    e("problem-not-a-pigeon-repository", &[]),
    e("problem-peer-unreachable", &[]),
    e("problem-peer-refused", &[]),
    e("problem-internal-error", &[]),
    e("hint-internal-error", &[]),
];

/// The entry of `id`, if the catalogue has it.
pub fn entry(id: &str) -> Option<&'static Entry> {
    MESSAGES.iter().find(|m| m.id == id)
}

/// The message id of a problem code.
pub fn problem_id(code: &str) -> String {
    format!("problem-{code}")
}

/// The hint id of a problem code, if the catalogue has one.
pub fn hint_id(code: &str) -> Option<&'static str> {
    let id = format!("hint-{code}");
    entry(&id).map(|m| m.id)
}

/// How people see a code: `P-` and the code in capitals, as
/// `P-INVITE-EXPIRED` (ux.md §7).
pub fn display_code(code: &str) -> String {
    format!("P-{}", code.to_uppercase())
}
