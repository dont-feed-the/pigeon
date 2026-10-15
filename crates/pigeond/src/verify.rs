//! Accept's check (04 §4.4, ADR 0007): the commits a peer's `main` adds to
//! ours are judged parents first by `crate::merge` against the catalogue
//! of their accepted parents; accept takes the last accepted one in
//! topological order. A verdict depends only on a commit's history, so an
//! accepted commit may hold refused ones, the proposals it adopts (04 §4.4
//! rule 7); the first commit not accepted is named unless `theirs` is.

use pigeon_core::authority::{self, Verdict};

use crate::git::Error;
use crate::group::Group;

/// What accept may take of a peer's `main`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judged {
    /// The last commit accepted, if any.
    pub accepted: Option<String>,
    /// The first commit not accepted, and its verdict.
    pub stopped: Option<(String, Verdict)>,
}

/// Judges the commits of `theirs` that `ours` lacks, oldest first.
pub async fn judge(g: &Group, ours: Option<&str>, theirs: &str) -> Result<Judged, Error> {
    let mut tips = vec![theirs.to_owned()];
    tips.extend(ours.map(str::to_owned));
    crate::merge::judge_history(g, &tips).await?;
    let range = match ours {
        Some(o) => format!("{o}..{theirs}"),
        None => theirs.to_owned(),
    };
    let list = g
        .git
        .run(&["rev-list", "--reverse", "--topo-order", &range])
        .await?;
    let mut out = Judged {
        accepted: None,
        stopped: None,
    };
    let k = g.lock();
    for c in list.lines().filter(|l| !l.is_empty()) {
        if k.engine.accepted(c) {
            out.accepted = Some(c.to_owned());
        } else if out.stopped.is_none() {
            let v = k.verdicts.get(c).cloned();
            out.stopped = Some((c.to_owned(), v.unwrap_or(Verdict::Unknown("parent"))));
        }
    }
    if out.accepted.as_deref() == Some(theirs) {
        out.stopped = None;
    }
    Ok(out)
}

/// The problem code of a verdict that stops accept.
pub fn code(v: &Verdict) -> &'static str {
    match v {
        Verdict::Refused(r) => authority::code(r),
        Verdict::Unknown(_) | Verdict::Accepted(..) => "commit-waits",
    }
}
