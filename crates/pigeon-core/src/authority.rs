//! The authority rule of 04 §4.4 (`Authority.tla`'s `Judge`) for a commit
//! with one accepted parent: who signed it, what it may change, and whether
//! its tree is well formed, with the deciders of a proposal and who may
//! decline or withdraw it (04 §4.5). Merges and adoptions (rule 7) are
//! `commit`'s, on `proposal`; rule 5 is 05 §5.4's `names`.
//!
//! [`judge`] is a pure function of the commit object, the parent's roster
//! R(p), both trees and who wrote each path of the base. Laws (`tests/authority.rs`): a commit signed by a
//! device of R(p) that changes only what its person may is accepted; one
//! that is unsigned, forged, or signed by a device outside R(p) is refused;
//! a change of a path is allowed exactly as 04 §4.3's table says; a roster
//! change is accepted iff [`roster::may_sign`] allows it and it applies; and
//! [`roster::diff`] is a right inverse of [`roster::apply`].

use std::collections::{BTreeMap, BTreeSet};

use crate::names;
use crate::roster::{self, DeviceId, Invalid, PersonId, Role, Roster, Rule};
use crate::sig::{self, SigError};

/// The rule sets this release judges (ADR 0048).
pub const RULES: u32 = 1;
/// The roster's path.
pub const ROSTER: &str = ".pigeon/roster.toml";
/// The trailer of a Force (04 §4.6).
pub const FORCE: &str = "Pigeon-Force:";
/// The trailer of *Keep both* (05 §5.3).
pub const RESOLVED: &str = "Pigeon-Resolved:";

/// One entry of a tree, as `ls-tree -r` lists it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

/// The persons who wrote each slot of a base (05 §5.1), as the catalogue's
/// `View::writers` gives them: a member may change an add-only path that
/// only they wrote (ADR 0070).
pub type Writers = BTreeMap<String, BTreeSet<PersonId>>;

/// Whether `u` alone wrote the slot of `path`, by `writers`.
pub fn sole_writer(writers: &Writers, u: &PersonId, path: &str) -> bool {
    writers
        .get(&names::fold(names::unit(path)))
        .is_some_and(|w| !w.is_empty() && w.iter().all(|p| p == u))
}

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
    /// Who wrote each slot of `before`.
    pub writers: &'a Writers,
    /// The bytes of the blobs of `after` that [`delta`] names, by object
    /// ID; a missing one is too large to be a pointer.
    pub blobs: &'a BTreeMap<String, Vec<u8>>,
    /// tree(c)'s roster, parsed, when [`delta`] names it.
    pub new_roster: Option<&'a Result<Roster, Unread>>,
    /// Each decision [`delta`] adds, by path, parsed (04 §4.5).
    pub decisions: &'a BTreeMap<String, Result<Decision, Unread>>,
}

/// What a decision of `.pigeon/decisions/` says of a proposal (04 §4.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decided {
    /// A decider declined it.
    Rejected,
    /// Its author took it back.
    Withdrawn,
}

/// A decision document, with what its judge needs of the proposal it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    /// The proposal's commit, as the document names it.
    pub proposal: String,
    /// What it says.
    pub verdict: Decided,
    /// The proposal's deciders under R(p) and its author's person; `None`
    /// when this device does not keep that proposal, and the commit waits.
    pub about: Option<(Deciders, roster::PersonId)>,
}

/// The path of the decision on `proposal`.
pub fn decision_path(proposal: &str) -> String {
    format!(".pigeon/decisions/{proposal}.toml")
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

/// The trailer lines of a commit object's message: its last paragraph.
fn trailers(raw: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(raw);
    let message = text.split_once("\n\n").map_or("", |(_, m)| m);
    let last = message.trim_end().rsplit("\n\n").next().unwrap_or_default();
    last.lines().map(str::to_owned).collect()
}

/// Whether the message of a commit object carries the Force trailer.
pub fn forced(raw: &[u8]) -> bool {
    trailers(raw).iter().any(|l| l.starts_with(FORCE))
}

/// The paths a commit object's `Pigeon-Resolved:` trailers name.
pub fn resolved(raw: &[u8]) -> Vec<String> {
    trailers(raw)
        .iter()
        .filter_map(|l| l.strip_prefix(RESOLVED))
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// The author time of a commit object, and its recorded offset in minutes
/// east of UTC (05 §5.3).
pub fn author_time(raw: &[u8]) -> Option<(i64, i32)> {
    let text = String::from_utf8_lossy(raw);
    let line = text
        .lines()
        .take_while(|l| !l.is_empty())
        .find(|l| l.starts_with("author "))?;
    let mut words = line.rsplit(' ');
    let zone = words.next()?;
    let secs: i64 = words.next()?.parse().ok()?;
    let (sign, digits) = match zone.split_at_checked(1)? {
        ("+", d) => (1, d),
        ("-", d) => (-1, d),
        _ => return None,
    };
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = digits.get(..2)?.parse().ok()?;
    let minutes: i32 = digits.get(2..)?.parse().ok()?;
    Some((secs, sign * (hours * 60 + minutes)))
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

/// Whether person `u` of `roster` may add (`added`) or change `path`, on
/// a base whose slots `writers` wrote, under the folder rules (04 §4.3):
/// in an add-only folder, a steward, or the person who alone wrote it
/// (ADR 0070), changes it.
pub fn allowed(
    roster: &Roster,
    writers: &Writers,
    u: &roster::PersonId,
    path: &str,
    added: bool,
) -> bool {
    let admin = roster.persons.get(u).is_some_and(|p| p.admin);
    let mine = || sole_writer(writers, u, path);
    match rule_of(roster, path) {
        Some(Rule::Owned(owner)) => owner == u,
        Some(Rule::AddOnly(stewards)) if !stewards.is_empty() => {
            added || stewards.contains(u) || mine()
        }
        _ => added || admin || mine(),
    }
}

/// The deciders of a proposal whose adoption makes `changes`, each a path
/// and whether it adds it, on a `main` whose slots `writers` wrote
/// (04 §4.5): the persons allowed every change under `roster`; if nobody,
/// the admins, who adopt it with Force (04 §4.6), which the flag says.
pub fn deciders(roster: &Roster, writers: &Writers, changes: &[(&str, bool)]) -> Deciders {
    let all: BTreeSet<roster::PersonId> = roster
        .persons
        .keys()
        .filter(|u| {
            changes
                .iter()
                .all(|(p, a)| allowed(roster, writers, u, p, *a))
        })
        .cloned()
        .collect();
    if !all.is_empty() {
        return (all, false);
    }
    let admins = roster
        .persons
        .iter()
        .filter(|(_, p)| p.admin)
        .map(|(u, _)| u.clone())
        .collect();
    (admins, true)
}

/// The deciders of a proposal, and whether its adoption needs Force.
pub type Deciders = (BTreeSet<roster::PersonId>, bool);

/// A batch of changes split as capture commits them (07 §7.3 steps 5 and
/// 7): those the signer's person may make, and the others by deciders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Split<T> {
    /// The changes for one commit on `main`.
    pub allowed: BTreeMap<String, T>,
    /// The changes for one proposal per set of deciders (04 §4.5).
    pub proposed: BTreeMap<Deciders, BTreeMap<String, T>>,
}

/// Splits `changes` of `before`, whose slots `writers` wrote, by `signer`
/// under `roster`: a change is allowed when 04 §4.3 lets the signer's
/// person make it, else proposed to the deciders of that one change. A
/// device outside the roster, or a recovery device, may make none.
pub fn split<T: Clone>(
    roster: &Roster,
    signer: &DeviceId,
    before: &Tree,
    writers: &Writers,
    changes: &BTreeMap<String, T>,
) -> Split<T> {
    let person = roster
        .devices
        .get(signer)
        .filter(|d| d.role != Role::Recovery)
        .map(|d| d.person.clone());
    let mut out = Split {
        allowed: BTreeMap::new(),
        proposed: BTreeMap::new(),
    };
    for (path, change) in changes {
        let added = !before.contains_key(path);
        if person
            .as_ref()
            .is_some_and(|u| allowed(roster, writers, u, path, added))
        {
            out.allowed.insert(path.clone(), change.clone());
        } else {
            out.proposed
                .entry(deciders(roster, writers, &[(path, added)]))
                .or_default()
                .insert(path.clone(), change.clone());
        }
    }
    out
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
        } else if path.starts_with(".pigeon/decisions/")
            && !path.starts_with(".pigeon/decisions/purge-")
        {
            if let Err(v) = decision(step, path, u, recovery) {
                return *v;
            }
        } else if path.starts_with(".pigeon/decisions/") || path.starts_with(".pigeon/records/") {
            if !admin {
                return Verdict::Unknown("decisions");
            }
        } else if path.starts_with(".pigeon/") {
            if !admin {
                return Verdict::Refused(Refusal::NotAllowed(path.clone()));
            }
        } else if recovery
            || !(allowed(r, step.writers, u, path, !step.before.contains_key(path)) || force)
        {
            return Verdict::Refused(Refusal::NotAllowed(path.clone()));
        }
    }
    Verdict::Accepted(signer, new_roster)
}

/// Whether person `u` may add the decision at `path` (04 §4.3's `.pigeon/`
/// table): a decider of its proposal to reject it, its author to withdraw
/// it. A decision is never changed or removed, and never by a recovery key.
fn decision(
    step: &Step<'_>,
    path: &str,
    u: &roster::PersonId,
    recovery: bool,
) -> Result<(), Box<Verdict>> {
    let refused = |r| Err(Box::new(Verdict::Refused(r)));
    if step.before.contains_key(path) || !step.after.contains_key(path) || recovery {
        return refused(Refusal::NotAllowed(path.to_owned()));
    }
    let d = match step.decisions.get(path) {
        Some(Ok(d)) => d,
        Some(Err(Unread::Unknown(_))) => return Err(Box::new(Verdict::Unknown("format"))),
        Some(Err(Unread::Malformed(_))) | None => {
            return refused(Refusal::Malformed(path.to_owned(), "decision"));
        }
    };
    if decision_path(&d.proposal) != path {
        return refused(Refusal::Malformed(path.to_owned(), "decision"));
    }
    let Some(((deciders, _), author)) = &d.about else {
        return Err(Box::new(Verdict::Unknown("decision")));
    };
    let may = match d.verdict {
        Decided::Rejected => deciders.contains(u),
        Decided::Withdrawn => author == u,
    };
    if may {
        Ok(())
    } else {
        refused(Refusal::NotAllowed(path.to_owned()))
    }
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
