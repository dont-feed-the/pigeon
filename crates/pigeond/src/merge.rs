//! Accept's judge over histories with merges (04 §4.4, 05 §5.2): every
//! commit of the histories it is shown, parents first, is judged once
//! against B(c), the catalogue of its accepted parents, and recorded in the
//! group's `catalogue::Engine`; and [`merge`] writes the merge of two
//! accepted tips, whose tree is the catalogue of both (P5).
//!
//! A commit with several accepted parents is judged under each parent's
//! roster R(p): with Δ = ∅ one roster that accepts its signer suffices;
//! otherwise every one must accept it (rules 2 and 3). Until M2 brings them,
//! a merge of parents whose rosters differ (a roster fork, 04 §4.8) and a
//! commit with a parent that is not accepted (an adoption, 04 §4.5) wait as
//! unknown. Rule 5 refuses a tree whose names Δ adds are not portable.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::authority::{self, Entry, ROSTER, Refusal, Step, Tree, Unread, Verdict};
use pigeon_core::catalogue::{Judgement, Meta, Outcome};
use pigeon_core::names::{self, Stamp};
use pigeon_core::roster::Roster;

use crate::git::{Error, Git};
use crate::group::Group;

async fn roster_at(git: &Git, tree: &Tree) -> Option<Result<Roster, Unread>> {
    let entry = tree.get(ROSTER)?;
    let blob = git.object(&entry.oid).await.ok()?;
    Some(crate::roster_file::parse(&String::from_utf8_lossy(&blob)).map(|d| d.roster))
}

/// Judges every commit of the histories of `tips` that the group's engine
/// has not judged, parents first.
pub async fn judge_history(g: &Group, tips: &[String]) -> Result<(), Error> {
    if tips.is_empty() {
        return Ok(());
    }
    let mut args = vec!["rev-list", "--reverse", "--topo-order", "--parents"];
    args.extend(tips.iter().map(String::as_str));
    let list = g.git.run(&args).await?;
    for line in list.lines() {
        let mut ids = line.split(' ').map(str::to_owned);
        let Some(c) = ids.next() else {
            continue;
        };
        if g.lock().engine.outcome(&c).is_some() {
            continue;
        }
        let parents: Vec<String> = ids.collect();
        judge_commit(g, &c, parents).await?;
    }
    Ok(())
}

/// The verdict of a commit judged under each roster in `steps`: with no
/// change, one acceptance is enough; with one, all must accept.
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

async fn judge_commit(g: &Group, c: &str, parents: Vec<String>) -> Result<(), Error> {
    let git = &g.git;
    let raw = git.object(c).await?;
    let after = git.tree(c).await?;
    let (secs, offset) = authority::author_time(&raw).unwrap_or((0, 0));
    let mut meta = Meta {
        id: c.to_owned(),
        parents: parents.clone(),
        device: String::new(),
        who: String::new(),
        when: Stamp { utc: secs, offset },
        resolves: Vec::new(),
    };
    let verdict = if parents.is_empty() {
        match roster_at(git, &after).await {
            Some(Ok(r)) => {
                let v = authority::judge_genesis(&raw, &r);
                name(&mut meta, &v, &[&r]);
                v
            }
            Some(Err(Unread::Unknown(_))) => Verdict::Unknown("format"),
            _ => Verdict::Refused(Refusal::RosterUnreadable("genesis".into())),
        }
    } else {
        judge_child(g, &raw, &after, &mut meta).await?
    };
    let judgement = match &verdict {
        Verdict::Accepted(..) => Judgement::Accepted,
        Verdict::Refused(r) => Judgement::Refused(authority::code(r).to_owned()),
        Verdict::Unknown(why) => Judgement::Refused(format!("unknown-{why}")),
    };
    let mut k = g.lock();
    let outcome = k.engine.admit(&meta, &after, judgement);
    if !matches!(outcome, Outcome::Clean | Outcome::Contribution) {
        let v = match verdict {
            Verdict::Accepted(..) => Verdict::Unknown("parent"),
            v => v,
        };
        k.verdicts.insert(c.to_owned(), v);
    }
    Ok(())
}

/// Sets the signer's device and display name in `meta` from the first
/// roster that knows it.
fn name(meta: &mut Meta, v: &Verdict, rosters: &[&Roster]) {
    let Verdict::Accepted(signer, _) = v else {
        return;
    };
    meta.device = signer.0.clone();
    for r in rosters {
        if let Some(d) = r.devices.get(signer) {
            meta.who = r
                .persons
                .get(&d.person)
                .map_or_else(|| d.person.0.clone(), |p| p.name.clone());
            return;
        }
    }
}

async fn judge_child(
    g: &Group,
    raw: &[u8],
    after: &Tree,
    meta: &mut Meta,
) -> Result<Verdict, Error> {
    let git = &g.git;
    let (accepted, others): (Vec<String>, Vec<String>) = {
        let k = g.lock();
        meta.parents
            .iter()
            .cloned()
            .partition(|p| k.engine.accepted(p))
    };
    if accepted.is_empty() {
        return Ok(Verdict::Unknown("parent"));
    }
    if !others.is_empty() {
        return Ok(Verdict::Unknown("proposal"));
    }
    let Some(before) = g.lock().engine.base(&accepted).map(|v| v.tree.clone()) else {
        return Ok(Verdict::Unknown("parent"));
    };
    let mut rosters = Vec::new();
    for p in &accepted {
        match roster_at(git, &git.tree(p).await?).await {
            Some(Ok(r)) => rosters.push(r),
            Some(Err(Unread::Unknown(_))) => return Ok(Verdict::Unknown("format")),
            _ => return Ok(Verdict::Refused(Refusal::RosterUnreadable("parent".into()))),
        }
    }
    if rosters.windows(2).any(|w| w[0] != w[1]) {
        return Ok(Verdict::Unknown("roster-fork"));
    }
    let verdict = judge_step(g, raw, &before, after, &rosters).await?;
    let refs: Vec<&Roster> = rosters.iter().collect();
    name(meta, &verdict, &refs);
    if let Verdict::Accepted(signer, _) = &verdict {
        let person = rosters
            .iter()
            .find_map(|r| r.devices.get(signer))
            .map(|d| d.person.clone());
        meta.resolves = authority::resolved(raw)
            .into_iter()
            .filter(|p| {
                person
                    .as_ref()
                    .is_some_and(|u| rosters.iter().all(|r| authority::allowed(r, u, p, false)))
            })
            .collect();
    }
    Ok(verdict)
}

/// The verdict of rules 2–6 on `raw`, whose tree is `after`, against
/// `before` under each of `rosters`, with rule 5's portable names.
async fn judge_step(
    g: &Group,
    raw: &[u8],
    before: &Tree,
    after: &Tree,
    rosters: &[Roster],
) -> Result<Verdict, Error> {
    let git = &g.git;
    let changed = authority::delta(before, after);
    for path in changed.iter().filter(|p| after.contains_key(*p)) {
        if let Err(why) = names::portable(path) {
            return Ok(Verdict::Refused(Refusal::Malformed(
                path.clone(),
                why.code(),
            )));
        }
    }
    if let Some(f) = names::clashes(after.keys().map(String::as_str))
        .into_iter()
        .next()
    {
        return Ok(Verdict::Refused(Refusal::Malformed(f, "collision")));
    }
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
        roster_at(git, after).await
    } else {
        None
    };
    let verdicts: Vec<Verdict> = rosters
        .iter()
        .map(|roster| {
            authority::judge(&Step {
                raw,
                roster,
                before,
                after,
                blobs: &blobs,
                new_roster: new_roster.as_ref(),
            })
        })
        .collect();
    Ok(combine(verdicts, changed.is_empty()))
}

/// The verdict on the proposal `raw`, whose tree is `after`, against
/// `base`, the accepted commit it builds on (04 §4.5): its change judged
/// under R(`base`) as if it were one commit on it.
pub async fn judge_proposal(
    g: &Group,
    raw: &[u8],
    base: &str,
    after: &Tree,
) -> Result<Verdict, Error> {
    let Some(before) = g
        .lock()
        .engine
        .base(&[base.to_owned()])
        .map(|v| v.tree.clone())
    else {
        return Ok(Verdict::Unknown("parent"));
    };
    let roster = match roster_at(&g.git, &g.git.tree(base).await?).await {
        Some(Ok(r)) => r,
        Some(Err(Unread::Unknown(_))) => return Ok(Verdict::Unknown("format")),
        _ => return Ok(Verdict::Refused(Refusal::RosterUnreadable("parent".into()))),
    };
    judge_step(g, raw, &before, after, &[roster]).await
}

/// Whether `theirs` brings a contribution that `ours` lacks (05 §5.2):
/// else it is merely an equivalent `main`, which needs no merge.
pub async fn brings(g: &Group, ours: &str, theirs: &str) -> Result<bool, Error> {
    let range = format!("{ours}..{theirs}");
    let list = g.git.run(&["rev-list", &range]).await?;
    let k = g.lock();
    Ok(list
        .lines()
        .any(|c| k.engine.outcome(c) == Some(&Outcome::Contribution)))
}

/// Writes `tree` as git tree objects, folders first: its root's ID.
pub async fn write_tree(git: &Git, tree: &Tree) -> Result<String, Error> {
    let mut dirs: BTreeMap<String, Vec<(String, String, String, String)>> = BTreeMap::new();
    dirs.entry(String::new()).or_default();
    for (path, e) in tree {
        let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
        let mut d = dir.to_owned();
        dirs.entry(d.clone()).or_default().push((
            e.mode.clone(),
            e.kind.clone(),
            e.oid.clone(),
            file.to_owned(),
        ));
        while !d.is_empty() {
            let parent = d
                .rsplit_once('/')
                .map_or(String::new(), |(p, _)| p.to_owned());
            dirs.entry(parent.clone()).or_default();
            d = parent;
        }
    }
    let mut written: BTreeMap<String, String> = BTreeMap::new();
    let order: Vec<String> = dirs.keys().rev().cloned().collect();
    for d in order {
        let mut entries = dirs.get(&d).cloned().unwrap_or_default();
        let children: BTreeSet<&String> = written
            .keys()
            .filter(|w| w.rsplit_once('/').map_or("", |(p, _)| p) == d && !w.is_empty())
            .collect();
        for child in children {
            let name = child.rsplit_once('/').map_or(child.as_str(), |(_, n)| n);
            entries.push((
                "040000".into(),
                "tree".into(),
                written[child].clone(),
                name.to_owned(),
            ));
        }
        let mut input = Vec::new();
        for (mode, kind, oid, file) in &entries {
            input.extend_from_slice(format!("{mode} {kind} {oid}\t{file}").as_bytes());
            input.push(0);
        }
        let oid = git
            .run_in(&["mktree", "-z"], &input)
            .await?
            .trim()
            .to_owned();
        written.insert(d, oid);
    }
    Ok(written.get("").cloned().unwrap_or_default())
}

/// Writes and judges the merge of `ours` and `theirs`, both accepted: its
/// tree is the catalogue of both (05 §5.2), it is signed with the device
/// key, and it must be accepted as clean.
pub async fn merge(g: &Group, ours: &str, theirs: &str) -> Result<Option<String>, Error> {
    let parents = [ours.to_owned(), theirs.to_owned()];
    let Some(tree) = g.lock().engine.base(&parents).map(|v| v.tree.clone()) else {
        return Ok(None);
    };
    let oid = write_tree(&g.git, &tree).await?;
    let commit = g
        .git
        .commit_tree(&oid, &[ours, theirs], "pigeon: merge")
        .await?;
    judge_history(g, std::slice::from_ref(&commit)).await?;
    let outcome = g.lock().engine.outcome(&commit).cloned();
    Ok((outcome == Some(Outcome::Clean)).then_some(commit))
}

/// Makes a save of `path` that `head` changed meanwhile a contribution
/// concurrent with `head`'s version (07 §7.6, ADR 0034): a commit of
/// `mine` (`None`: a deletion) on the latest commit of `head`'s history
/// whose `path` is `before`, the version the person saved over, merged with
/// `head`. Answers the merge if it is accepted clean, else nothing.
pub async fn concurrent(
    g: &Group,
    head: &str,
    path: &str,
    before: Option<&Entry>,
    mine: Option<&Entry>,
) -> Result<Option<String>, Error> {
    let list = g.git.run(&["rev-list", "--topo-order", head]).await?;
    let ids: Vec<&str> = list.lines().collect();
    let mut input = String::new();
    for c in &ids {
        input.push_str(&format!("{c}:{path}\n"));
    }
    let out = g
        .git
        .run_in(
            &["cat-file", "--batch-check=%(objectname)"],
            input.as_bytes(),
        )
        .await?;
    let base = ids.iter().zip(out.lines()).find_map(|(c, line)| {
        let at = (!line.ends_with(" missing")).then_some(line);
        (at == before.map(|e| e.oid.as_str())).then_some(*c)
    });
    let Some(base) = base else {
        return Ok(None);
    };
    let mut tree = g.git.tree(base).await?;
    match mine {
        Some(e) => tree.insert(path.to_owned(), e.clone()),
        None => tree.remove(path),
    };
    let oid = write_tree(&g.git, &tree).await?;
    let side = g.git.commit_tree(&oid, &[base], "pigeon: capture").await?;
    judge_history(g, &[head.to_owned(), side.clone()]).await?;
    if g.lock().engine.outcome(&side) != Some(&Outcome::Contribution) {
        return Ok(None);
    }
    merge(g, head, &side).await
}
