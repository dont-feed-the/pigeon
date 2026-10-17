//! The judge of a commit with any parents (04 §4.4 rules 2–7,
//! `Authority.tla`'s `Judge`): [`judge`] composes [`authority::judge`] under
//! the roster R(p) of each accepted parent with rule 7 for each parent
//! outside A(c), a proposal the commit adopts (04 §4.5), and rule 5's
//! portable names (05 §5.4); [`about`] is what the judge of a decision
//! needs of the proposal it names. Pure functions of the commit object,
//! B(c), the trees and the rosters, which pigeond's `merge` reads from git
//! and its catalogue engine. With no change one roster that accepts the
//! signer suffices, otherwise every one must. Traces of [`judge`] on
//! random histories with proposals, decisions and adoptions are checked
//! against `Judge` (`tests/authority_trace.rs`, `AuthorityTrace.tla`).

use std::collections::BTreeMap;

use crate::authority::{self, Deciders, Decision, Refusal, Step, Tree, Unread, Verdict, Writers};
use crate::names;
use crate::proposal;
use crate::roster::{DeviceId, PersonId, Roster};

/// A parent outside A(c), which the commit must adopt (rule 7).
#[derive(Clone, Copy, Debug)]
pub struct Adopted<'a> {
    /// Its verdict judged as one commit on its base (04 §4.5): a proposal
    /// is refused for folder rules alone.
    pub verdict: &'a Verdict,
    /// The tree of its base, the accepted commit it builds on.
    pub base: &'a Tree,
    /// Its own tree.
    pub tree: &'a Tree,
}

/// A commit to judge against its accepted parents A(c).
#[derive(Clone, Copy, Debug)]
pub struct Commit<'a> {
    /// The commit object.
    pub raw: &'a [u8],
    /// R(p) for each accepted parent p.
    pub rosters: &'a [Roster],
    /// B(c), the catalogue of the accepted parents' histories.
    pub before: &'a Tree,
    /// tree(c).
    pub after: &'a Tree,
    /// Who wrote each slot of B(c) (ADR 0070).
    pub writers: &'a Writers,
    /// As [`Step::blobs`].
    pub blobs: &'a BTreeMap<String, Vec<u8>>,
    /// As [`Step::new_roster`].
    pub new_roster: Option<&'a Result<Roster, Unread>>,
    /// As [`Step::decisions`].
    pub decisions: &'a BTreeMap<String, Result<Decision, Unread>>,
    /// Parsed purge additions and their verified historical scope.
    pub purges: &'a BTreeMap<String, Result<authority::PurgeCheck, Unread>>,
    /// Independently checked immutable proof documents.
    pub witnesses: &'a BTreeMap<String, Result<(), Unread>>,
    /// Each parent outside A(c).
    pub adopted: &'a [Adopted<'a>],
}

/// The verdict of 04 §4.4 on `c`.
pub fn judge(c: &Commit<'_>) -> Verdict {
    if c.rosters.is_empty() {
        return Verdict::Unknown("parent");
    }
    for x in c.adopted {
        if !matches!(x.verdict, Verdict::Refused(Refusal::NotAllowed(_))) {
            return Verdict::Unknown("parent");
        }
        // Approval can happen long after a purge request. Its sole
        // permitted metadata adjustment extends the same scope's wait;
        // admission independently checked those original blob identities.
        let mut after = c.after.clone();
        for (path, check) in c.purges {
            if path.starts_with(".pigeon/decisions/purge-")
                && let (Ok(check), Some(original)) = (check, x.tree.get(path))
                && check.adoption_blobs.contains(&original.oid)
            {
                after.insert(path.clone(), original.clone());
            }
        }
        if let Err(path) = proposal::adopts(x.base, x.tree, c.before, &after) {
            return Verdict::Refused(Refusal::NotAllowed(path));
        }
    }
    let changed = authority::delta(c.before, c.after);
    for path in changed.iter().filter(|p| c.after.contains_key(*p)) {
        if let Err(why) = names::portable(path) {
            return Verdict::Refused(Refusal::Malformed(path.clone(), why.code()));
        }
    }
    if let Some(f) = names::clashes(c.after.keys().map(String::as_str))
        .into_iter()
        .next()
    {
        return Verdict::Refused(Refusal::Malformed(f, "collision"));
    }
    let verdicts = c
        .rosters
        .iter()
        .map(|roster| {
            authority::judge(&Step {
                raw: c.raw,
                roster,
                before: c.before,
                after: c.after,
                writers: c.writers,
                blobs: c.blobs,
                new_roster: c.new_roster,
                decisions: c.decisions,
                purges: c.purges,
                witnesses: c.witnesses,
            })
        })
        .collect();
    combine(verdicts, changed.is_empty())
}

/// With no change, the first acceptance; else the first verdict that is
/// not one, or the first acceptance when all accept.
fn combine(verdicts: Vec<Verdict>, clean: bool) -> Verdict {
    let first_ok = verdicts
        .iter()
        .position(|v| matches!(v, Verdict::Accepted(..)));
    let first_not = verdicts
        .iter()
        .position(|v| !matches!(v, Verdict::Accepted(..)));
    let pick = match (clean, first_ok, first_not) {
        (true, Some(i), _) | (false, Some(i), None) => i,
        (_, _, Some(i)) => i,
        (_, None, None) => return Verdict::Unknown("no-parent"),
    };
    verdicts
        .into_iter()
        .nth(pick)
        .unwrap_or(Verdict::Unknown("no-parent"))
}

/// What the judge of a decision on `main`, B(c), whose slots `writers`
/// wrote, needs of the proposal it names (04 §4.5): its deciders under
/// `roster`, R(p) of the decision's parent, on the changes its adoption on
/// `main` makes; and its author, the person of its `signer` under `at`, the
/// roster of its base. `None` when `at` does not know the signer.
pub fn about(
    roster: &Roster,
    at: &Roster,
    signer: &DeviceId,
    base: &Tree,
    tree: &Tree,
    main: &Tree,
    writers: &Writers,
) -> Option<(Deciders, PersonId)> {
    let author = at.devices.get(signer)?.person.clone();
    let adoption = proposal::adoption_changes(base, tree, main);
    let changes: Vec<(&str, bool)> = adoption.iter().map(|(p, a)| (p.as_str(), *a)).collect();
    Some((authority::deciders(roster, writers, &changes), author))
}
