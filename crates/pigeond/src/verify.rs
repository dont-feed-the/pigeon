//! Accept's check (04 §4.4, ADR 0007): the commits a peer's `main` adds to
//! ours are judged oldest first, each against its one parent, by
//! `pigeon_core::authority`; accept moves `main` to the last accepted one,
//! and the first refused commit, with every commit after it, waits.
//!
//! The parent's roster R(p) is read from its tree; the blobs of the paths a
//! commit changes outside `.pigeon/` are read to check that each is a
//! git-annex pointer. A merge waits as unknown: merges and proposals come
//! with the catalogue merge (M2).

use std::collections::BTreeMap;

use pigeon_core::authority::{self, ROSTER, Step, Tree, Unread, Verdict};
use pigeon_core::roster::Roster;

use crate::git::{Error, Git};

/// What accept may take of a peer's `main`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judged {
    /// The last commit accepted, if any.
    pub accepted: Option<String>,
    /// The first commit not accepted, and its verdict.
    pub stopped: Option<(String, Verdict)>,
}

async fn roster_at(git: &Git, tree: &Tree) -> Option<Result<Roster, Unread>> {
    let entry = tree.get(ROSTER)?;
    let blob = git.object(&entry.oid).await.ok()?;
    Some(crate::roster_file::parse(&String::from_utf8_lossy(&blob)).map(|d| d.roster))
}

/// Judges the commits of `theirs` that `ours` lacks, oldest first.
pub async fn judge(git: &Git, ours: Option<&str>, theirs: &str) -> Result<Judged, Error> {
    let range = match ours {
        Some(o) => format!("{o}..{theirs}"),
        None => theirs.to_owned(),
    };
    let list = git
        .run(&["rev-list", "--reverse", "--topo-order", "--parents", &range])
        .await?;
    let mut out = Judged {
        accepted: None,
        stopped: None,
    };
    for line in list.lines() {
        let mut ids = line.split(' ').map(str::to_owned);
        let Some(c) = ids.next() else {
            continue;
        };
        let parents: Vec<String> = ids.collect();
        let verdict = judge_one(git, &c, &parents).await?;
        match verdict {
            Verdict::Accepted(..) => out.accepted = Some(c),
            v => {
                out.stopped = Some((c, v));
                break;
            }
        }
    }
    Ok(out)
}

async fn judge_one(git: &Git, c: &str, parents: &[String]) -> Result<Verdict, Error> {
    let raw = git.object(c).await?;
    let after = git.tree(c).await?;
    let p = match parents {
        [] => {
            return Ok(match roster_at(git, &after).await {
                Some(Ok(r)) => authority::judge_genesis(&raw, &r),
                Some(Err(Unread::Unknown(_))) => Verdict::Unknown("format"),
                _ => Verdict::Refused(authority::Refusal::RosterUnreadable("genesis".into())),
            });
        }
        [p] => p,
        _ => return Ok(Verdict::Unknown("merge")),
    };
    let before = git.tree(p).await?;
    let roster = match roster_at(git, &before).await {
        Some(Ok(r)) => r,
        Some(Err(Unread::Unknown(_))) => return Ok(Verdict::Unknown("format")),
        _ => {
            return Ok(Verdict::Refused(authority::Refusal::RosterUnreadable(
                "parent".into(),
            )));
        }
    };
    let changed = authority::delta(&before, &after);
    let oids: Vec<String> = changed
        .iter()
        .filter(|path| !authority::is_pigeon_path(path))
        .filter_map(|path| after.get(path))
        .filter(|e| e.kind == "blob")
        .map(|e| e.oid.clone())
        .collect();
    let found = git.objects(&oids).await?;
    let blobs: BTreeMap<String, Vec<u8>> = oids
        .into_iter()
        .zip(found)
        .filter_map(|(oid, b)| Some((oid, b?.1)))
        .collect();
    let new_roster = if changed.contains(ROSTER) {
        roster_at(git, &after).await
    } else {
        None
    };
    Ok(authority::judge(&Step {
        raw: &raw,
        roster: &roster,
        before: &before,
        after: &after,
        blobs: &blobs,
        new_roster: new_roster.as_ref(),
    }))
}

/// The problem code of a verdict that stops accept.
pub fn code(v: &Verdict) -> &'static str {
    match v {
        Verdict::Refused(r) => authority::code(r),
        Verdict::Unknown(_) | Verdict::Accepted(..) => "commit-waits",
    }
}
