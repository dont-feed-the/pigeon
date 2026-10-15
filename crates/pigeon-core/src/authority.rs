//! The authority rule of 04 §4.4 (`Authority.tla`'s `Judge`) for a commit
//! with one accepted parent: who signed it, what it may change, and whether
//! its tree is well formed. Merges, proposals and decisions come with the
//! catalogue merge (M2); rule 5, portable names, with 05 §5.4's function.
//!
//! [`judge`] is a pure function of the commit object, the parent's roster
//! R(p) and both trees. Laws (`tests/authority.rs`): a commit signed by a
//! device of R(p) that changes only what its person may is accepted; one
//! that is unsigned, forged, or signed by a device outside R(p) is refused;
//! a change of a path is allowed exactly as 04 §4.3's table says; a roster
//! change is accepted iff [`roster::may_sign`] allows it and it applies; and
//! [`roster::diff`] is a right inverse of [`roster::apply`].

use std::collections::{BTreeMap, BTreeSet};

use crate::roster::{self, DeviceId, Invalid, Role, Roster, Rule};
use crate::sig::{self, SigError};

/// The rule sets this release judges (ADR 0048).
pub const RULES: u32 = 1;
/// The roster's path.
pub const ROSTER: &str = ".pigeon/roster.toml";
/// The trailer of a Force (04 §4.6).
pub const FORCE: &str = "Pigeon-Force:";

/// One entry of a tree, as `ls-tree -r` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Its mode, such as `100644`.
    pub mode: String,
    /// Its object kind: `blob`, `commit` or `tree`.
    pub kind: String,
    /// Its object ID.
    pub oid: String,
}

/// A tree: path → entry.
pub type Tree = BTreeMap<String, Entry>;

/// Why a commit is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// It has no valid signature (`signer(c)` of 04 §4.4).
    Signature(SigError),
    /// Rule 2: its signer is not a device of R(p).
    UnknownSigner(DeviceId),
    /// Rule 3: its signer's person may not make this change of `path`
    /// (04 §4.3, the `.pigeon/` table, or 04 §4.7 for the roster).
    NotAllowed(String),
    /// Rule 4: the new roster does not parse.
    RosterUnreadable(String),
    /// Rule 4: the new roster is invalid.
    RosterInvalid(Invalid),
    /// Rule 4: the new roster is not R(p) with one change applied whole.
    RosterNotAChange,
    /// Rule 6: `path` is not well formed (07 §7.9), for this reason.
    Malformed(String, &'static str),
}

/// The verdict on one commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Accepted: its signer, and R(c) when it changes the roster.
    Accepted(DeviceId, Option<Roster>),
    /// Refused, and why.
    Refused(Refusal),
    /// Unknown to this release (04 §4.4, ADR 0048): kept as fetched.
    Unknown(&'static str),
}

/// A commit to judge against its one accepted parent `p`.
#[derive(Clone, Copy, Debug)]
pub struct Step<'a> {
    /// The commit object.
    pub raw: &'a [u8],
    /// R(p).
    pub roster: &'a Roster,
    /// tree(p).
    pub before: &'a Tree,
    /// tree(c).
    pub after: &'a Tree,
    /// The bytes of the blobs of `after` that [`delta`] names, by object
    /// ID; a missing one is too large to be a pointer.
    pub blobs: &'a BTreeMap<String, Vec<u8>>,
    /// tree(c)'s roster, parsed, when [`delta`] names it.
    pub new_roster: Option<&'a Result<Roster, Unread>>,
}

/// Why a `.pigeon/` document was not read (ADR 0048).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unread {
    /// A format, field or value this release does not know: the commit is
    /// unknown, never refused, since a newer release may accept it.
    Unknown(String),
    /// Not a document of its format: the commit is refused.
    Malformed(String),
}

impl std::fmt::Display for Unread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(what) => write!(f, "unknown {what}"),
            Self::Malformed(why) => write!(f, "malformed: {why}"),
        }
    }
}

/// Δ for one parent: the paths whose entry differs between the trees.
pub fn delta(before: &Tree, after: &Tree) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = before
        .iter()
        .filter(|(p, e)| after.get(*p) != Some(*e))
        .map(|(p, _)| p.clone())
        .collect();
    out.extend(
        after
            .iter()
            .filter(|(p, e)| before.get(*p) != Some(*e))
            .map(|(p, _)| p.clone()),
    );
    out
}

/// Whether `path` is pigeon's own: under `.pigeon/`, or the root
/// `.gitattributes` (03 §3.3).
pub fn is_pigeon_path(path: &str) -> bool {
    path == ".gitattributes" || path.starts_with(".pigeon/")
}

/// Whether `blob` is a git-annex pointer to a `SHA256E` key with its size.
pub fn is_pointer(blob: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(blob) else {
        return false;
    };
    let line = text.strip_suffix('\n').unwrap_or(text);
    let Some(key) = line.strip_prefix("/annex/objects/") else {
        return false;
    };
    let Some(rest) = key.strip_prefix("SHA256E-s") else {
        return false;
    };
    let Some((size, hash)) = rest.split_once("--") else {
        return false;
    };
    let hex = hash.get(..64).unwrap_or_default();
    let ext = hash.get(64..).unwrap_or_default();
    !size.is_empty()
        && size.bytes().all(|b| b.is_ascii_digit())
        && hex.len() == 64
        && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        && (ext.is_empty() || ext.starts_with('.'))
        && !key.contains(['/', '\n', '\r'])
}

/// Whether the message of a commit object carries the Force trailer.
pub fn forced(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(raw);
    let message = text.split_once("\n\n").map_or("", |(_, m)| m);
    let last = message.trim_end().rsplit("\n\n").next().unwrap_or_default();
    last.lines().any(|l| l.starts_with(FORCE))
}

/// The rule of `path` under `roster`: the folder entry with the longest
/// path prefix of it; with none, add-only with the admins as stewards.
pub fn rule_of<'a>(roster: &'a Roster, path: &str) -> Option<&'a Rule> {
    roster
        .folders
        .iter()
        .filter(|f| path == f.path || path.starts_with(&format!("{}/", f.path)))
        .max_by_key(|f| f.path.len())
        .map(|f| &f.rule)
}

/// Whether person `u` of `roster` may add (`added`) or change `path`
/// under the folder rules (04 §4.3).
pub fn allowed(roster: &Roster, u: &roster::PersonId, path: &str, added: bool) -> bool {
    let admin = roster.persons.get(u).is_some_and(|p| p.admin);
    match rule_of(roster, path) {
        Some(Rule::Owned(owner)) => owner == u,
        Some(Rule::AddOnly(stewards)) if !stewards.is_empty() => added || stewards.contains(u),
        _ => added || admin,
    }
}

fn well_formed(path: &str, e: &Entry, blobs: &BTreeMap<String, Vec<u8>>) -> Option<&'static str> {
    if e.kind != "blob" {
        return Some("not-a-file");
    }
    if e.mode != "100644" && e.mode != "100755" {
        return Some("mode");
    }
    if is_pigeon_path(path) {
        return None;
    }
    match blobs.get(&e.oid) {
        Some(b) if is_pointer(b) => None,
        _ => Some("not-a-pointer"),
    }
}

/// The verdict of 04 §4.4 on `step`'s commit.
pub fn judge(step: &Step<'_>) -> Verdict {
    let r = step.roster;
    if r.rules > RULES {
        return Verdict::Unknown("rules");
    }
    let signer = match sig::verify(step.raw) {
        Ok(s) => s,
        Err(e) => return Verdict::Refused(Refusal::Signature(e)),
    };
    let Some(device) = r.devices.get(&signer) else {
        return Verdict::Refused(Refusal::UnknownSigner(signer));
    };
    let u = &device.person;
    let recovery = device.role == Role::Recovery;
    let admin = !recovery && r.persons.get(u).is_some_and(|p| p.admin);
    let force = admin && forced(step.raw);
    let changed = delta(step.before, step.after);
    for path in &changed {
        if let Some(e) = step.after.get(path)
            && let Some(why) = well_formed(path, e, step.blobs)
        {
            return Verdict::Refused(Refusal::Malformed(path.clone(), why));
        }
    }
    let mut new_roster = None;
    for path in &changed {
        if path == ROSTER {
            match roster_change(step, &signer) {
                Ok(n) => new_roster = Some(n),
                Err(v) => return *v,
            }
        } else if path.starts_with(".pigeon/decisions/") || path.starts_with(".pigeon/records/") {
            if !admin {
                return Verdict::Unknown("decisions");
            }
        } else if path.starts_with(".pigeon/") {
            if !admin {
                return Verdict::Refused(Refusal::NotAllowed(path.clone()));
            }
        } else if recovery || !(allowed(r, u, path, !step.before.contains_key(path)) || force) {
            return Verdict::Refused(Refusal::NotAllowed(path.clone()));
        }
    }
    Verdict::Accepted(signer, new_roster)
}

fn roster_change(step: &Step<'_>, signer: &DeviceId) -> Result<Roster, Box<Verdict>> {
    let refused = |r| Err(Box::new(Verdict::Refused(r)));
    let new = match step.new_roster {
        Some(Ok(n)) if step.after.contains_key(ROSTER) => n,
        Some(Err(Unread::Unknown(_))) => return Err(Box::new(Verdict::Unknown("format"))),
        Some(Err(Unread::Malformed(e))) => return refused(Refusal::RosterUnreadable(e.clone())),
        _ => return refused(Refusal::RosterUnreadable("missing".to_owned())),
    };
    if new.rules > RULES {
        return Err(Box::new(Verdict::Unknown("rules")));
    }
    if new.rules < step.roster.rules {
        return refused(Refusal::RosterNotAChange);
    }
    if let Err(why) = new.validate() {
        return refused(Refusal::RosterInvalid(why));
    }
    let Some(change) = roster::diff(step.roster, new) else {
        return refused(Refusal::RosterNotAChange);
    };
    if !roster::may_sign(step.roster, signer, &change) {
        return refused(Refusal::NotAllowed(ROSTER.to_owned()));
    }
    let applied = roster::apply(step.roster, &change);
    match applied.skipped {
        None if applied.roster == *new => Ok(applied.roster),
        Some(roster::Skip::Invalid(why)) => refused(Refusal::RosterInvalid(why)),
        _ => refused(Refusal::RosterNotAChange),
    }
}

/// The verdict on a genesis commit (04 §4.1): signed by a device of the
/// roster in its own tree.
pub fn judge_genesis(raw: &[u8], roster: &Roster) -> Verdict {
    match sig::verify(raw) {
        Err(e) => Verdict::Refused(Refusal::Signature(e)),
        Ok(s) if roster.devices.contains_key(&s) => Verdict::Accepted(s, None),
        Ok(s) => Verdict::Refused(Refusal::UnknownSigner(s)),
    }
}

/// The parents named by a commit object.
pub fn parents(raw: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(raw);
    text.lines()
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.strip_prefix("parent "))
        .map(str::to_owned)
        .collect()
}

/// The problem code a refusal is reported with (06, ADR 0023).
pub fn code(r: &Refusal) -> &'static str {
    match r {
        Refusal::Signature(SigError::Unsigned) => "commit-unsigned",
        Refusal::Signature(_) => "commit-forged",
        Refusal::UnknownSigner(_) => "commit-unknown-signer",
        Refusal::NotAllowed(_) => "commit-not-allowed",
        Refusal::RosterUnreadable(_)
        | Refusal::RosterInvalid(_)
        | Refusal::RosterNotAChange
        | Refusal::Malformed(..) => "commit-malformed",
    }
}
