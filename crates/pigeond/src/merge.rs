//! Accept's judge over histories with merges (04 §4.4, 05 §5.2): every
//! commit of the histories it is shown, parents first, is judged once
//! against B(c), the catalogue of its accepted parents, and recorded in the
//! group's `catalogue::Engine`; and [`merge`] writes the merge of two
//! accepted tips, whose tree is the catalogue of both (P5).
//!
//! A commit with several accepted parents is judged under each parent's
//! roster R(p): with Δ = ∅ one roster that accepts its signer suffices;
//! otherwise every one must accept it (rules 2 and 3). A parent that is not
//! accepted must be a proposal the commit adopts (rule 7, [`adopted`]); a
//! merge of parents whose rosters differ (a roster fork, 04 §4.8) must
//! hold their fold (`fold`), which then leaves Δ. Rule 5 refuses a tree
//! whose names Δ adds are not portable. [`adopt`] writes an adoption.

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
    let mut genesis = None;
    let (verdict, accepted, delta) = if parents.is_empty() {
        let v = match roster_at(git, &after).await {
            Some(Ok(r)) => {
                let v = authority::judge_genesis(&raw, &r);
                name(&mut meta, &v, &[&r]);
                genesis = Some(r);
                v
            }
            Some(Err(Unread::Unknown(_))) => Verdict::Unknown("format"),
            _ => Verdict::Refused(Refusal::RosterUnreadable("genesis".into())),
        };
        (v, Vec::new(), Vec::new())
    } else {
        judge_child(g, &raw, &after, &mut meta).await?
    };
    let judgement = match &verdict {
        Verdict::Accepted(..) => Judgement::Accepted,
        Verdict::Refused(r) => Judgement::Refused(authority::code(r).to_owned()),
        Verdict::Unknown(why) => Judgement::Refused(format!("unknown-{why}")),
    };
    let outcome = g.lock().engine.admit(&meta, &after, judgement);
    match (&verdict, &outcome) {
        (Verdict::Accepted(signer, _), Outcome::Clean | Outcome::Contribution) => match &genesis {
            Some(r) => crate::fold::genesis(g, c, r),
            None => crate::fold::record(g, c, &accepted, secs, signer, delta),
        },
        _ => {
            let v = match verdict {
                Verdict::Accepted(..) => Verdict::Unknown("parent"),
                v => v,
            };
            g.lock().verdicts.insert(c.to_owned(), v);
        }
    }
    Ok(())
}

/// A verdict, the accepted parents it was judged under, and the roster
/// change δ of 04 §4.8 the commit makes.
type Judged = (Verdict, Vec<String>, Vec<pigeon_core::roster::Edit>);

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
) -> Result<Judged, Error> {
    let (accepted, others): (Vec<String>, Vec<String>) = {
        let k = g.lock();
        meta.parents
            .iter()
            .cloned()
            .partition(|p| k.engine.accepted(p))
    };
    let (verdict, delta) = judge_parents(g, raw, after, meta, &accepted, &others).await?;
    Ok((verdict, accepted, delta))
}

async fn judge_parents(
    g: &Group,
    raw: &[u8],
    after: &Tree,
    meta: &mut Meta,
    accepted: &[String],
    others: &[String],
) -> Result<(Verdict, Vec<pigeon_core::roster::Edit>), Error> {
    let git = &g.git;
    let none = |v| Ok((v, Vec::new()));
    if accepted.is_empty() {
        return none(Verdict::Unknown("parent"));
    }
    let Some(mut before) = g.lock().engine.base(accepted).map(|v| v.tree.clone()) else {
        return none(Verdict::Unknown("parent"));
    };
    let mut author = None;
    for x in others {
        match adopted(g, x, &before, after).await? {
            Ok(a) => author = author.or(Some(a)),
            Err(v) => return none(v),
        }
    }
    let mut rosters = Vec::new();
    for p in accepted {
        match roster_at(git, &git.tree(p).await?).await {
            Some(Ok(r)) => rosters.push(r),
            Some(Err(Unread::Unknown(_))) => return none(Verdict::Unknown("format")),
            _ => return none(Verdict::Refused(Refusal::RosterUnreadable("parent".into()))),
        }
    }
    if rosters.windows(2).any(|w| w[0] != w[1]) {
        // A merge of roster forks holds their fold, and changes no roster
        // itself (04 §4.8): its roster leaves Δ.
        let Some(folded) = crate::fold::of_merge(g, accepted) else {
            return none(Verdict::Unknown("roster-fork"));
        };
        match roster_at(git, after).await {
            Some(Ok(r)) if r == folded.roster => {}
            Some(Err(Unread::Unknown(_))) => return none(Verdict::Unknown("format")),
            _ => return none(Verdict::Refused(Refusal::RosterNotAChange)),
        }
        if !folded.skipped.is_empty() {
            tracing::info!(group = %g.id, commit = %meta.id, skipped = ?folded.skipped, "roster-skipped");
        }
        if let Some(e) = after.get(ROSTER) {
            before.insert(ROSTER.to_owned(), e.clone());
        }
    }
    let verdict = judge_step(g, raw, &before, after, &rosters).await?;
    let refs: Vec<&Roster> = rosters.iter().collect();
    name(meta, &verdict, &refs);
    if let (Verdict::Accepted(..), Some((device, when))) = (&verdict, author) {
        // An adoption shows the proposal's author and time (05 §5.3).
        let signer = Verdict::Accepted(pigeon_core::roster::DeviceId(device), None);
        let decider = std::mem::take(&mut meta.who);
        name(meta, &signer, &refs);
        if meta.who.is_empty() {
            meta.who = decider;
        }
        meta.when = when;
    }
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
    let delta = match (&verdict, rosters.first()) {
        (Verdict::Accepted(_, Some(new)), Some(old)) => {
            pigeon_core::roster::diff(old, new).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    Ok((verdict, delta))
}

/// Rule 7 for the parent `x` of a commit outside A(c), whose tree is
/// `after` and B(c) `before`: `x` must be a proposal, signed by a device
/// and refused for folder rules alone against its base, and `after` must
/// hold its change (`proposal::adopts`). Answers its signer and author
/// time, or the verdict on the commit.
async fn adopted(
    g: &Group,
    x: &str,
    before: &Tree,
    after: &Tree,
) -> Result<Result<(String, Stamp), Verdict>, Error> {
    let raw = g.git.object(x).await?;
    let Ok(signer) = pigeon_core::sig::verify(&raw) else {
        return Ok(Err(Verdict::Unknown("parent")));
    };
    let Some(base) = crate::proposal::base(g, x).await? else {
        return Ok(Err(Verdict::Unknown("parent")));
    };
    let mine = g.git.tree(x).await?;
    match judge_proposal(g, &raw, &base, &mine).await? {
        Verdict::Refused(Refusal::NotAllowed(_)) => {}
        _ => return Ok(Err(Verdict::Unknown("parent"))),
    }
    let Some(at) = g
        .lock()
        .engine
        .base(std::slice::from_ref(&base))
        .map(|v| v.tree.clone())
    else {
        return Ok(Err(Verdict::Unknown("parent")));
    };
    if let Err(path) = pigeon_core::proposal::adopts(&at, &mine, before, after) {
        return Ok(Err(Verdict::Refused(Refusal::NotAllowed(path))));
    }
    let (utc, offset) = authority::author_time(&raw).unwrap_or((0, 0));
    Ok(Ok((signer.0, Stamp { utc, offset })))
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
    let decisions = decisions(g, before, after, &changed, rosters).await?;
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
                decisions: &decisions,
            })
        })
        .collect();
    Ok(combine(verdicts, changed.is_empty()))
}

/// The decisions that a commit's Δ adds (04 §4.5), each with the deciders
/// and author of the proposal it names, if this device keeps it, under the
/// first of `rosters`; deciders are reckoned on `before`, B(c).
async fn decisions(
    g: &Group,
    before: &Tree,
    after: &Tree,
    changed: &BTreeSet<String>,
    rosters: &[Roster],
) -> Result<BTreeMap<String, Result<authority::Decision, Unread>>, Error> {
    let mut out = BTreeMap::new();
    let Some(roster) = rosters.first() else {
        return Ok(out);
    };
    for path in changed {
        let Some(e) = after.get(path) else {
            continue;
        };
        if !path.starts_with(".pigeon/decisions/") || path.starts_with(".pigeon/decisions/purge-") {
            continue;
        }
        let text = String::from_utf8_lossy(&g.git.object(&e.oid).await?).into_owned();
        let doc = match crate::decision_file::parse(&text) {
            Ok(d) => d,
            Err(u) => {
                out.insert(path.clone(), Err(u));
                continue;
            }
        };
        let about = about(g, &doc.proposal, before, roster).await?;
        let d = authority::Decision {
            proposal: doc.proposal,
            verdict: doc.verdict,
            about,
        };
        out.insert(path.clone(), Ok(d));
    }
    Ok(out)
}

/// The deciders on `before` and the author's person of the kept proposal
/// `x`, under `roster`.
async fn about(
    g: &Group,
    x: &str,
    before: &Tree,
    roster: &Roster,
) -> Result<Option<(authority::Deciders, pigeon_core::roster::PersonId)>, Error> {
    let kept = crate::proposal::kept(&g.git).await;
    let Some((device, _)) = kept.into_iter().find(|(_, c)| c == x) else {
        return Ok(None);
    };
    let Some(person) = roster
        .devices
        .get(&pigeon_core::roster::DeviceId(device))
        .map(|d| d.person.clone())
    else {
        return Ok(None);
    };
    let Some(base) = crate::proposal::base(g, x).await? else {
        return Ok(None);
    };
    let at = g
        .lock()
        .engine
        .base(std::slice::from_ref(&base))
        .map(|v| v.tree.clone());
    let Some(at) = at else {
        return Ok(None);
    };
    let adoption = pigeon_core::proposal::adoption_changes(&at, &g.git.tree(x).await?, before);
    let changes: Vec<(&str, bool)> = adoption.iter().map(|(p, a)| (p.as_str(), *a)).collect();
    Ok(Some((authority::deciders(roster, &changes), person)))
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
    let Some(mut tree) = g.lock().engine.base(&parents).map(|v| v.tree.clone()) else {
        return Ok(None);
    };
    let (a, b) = (g.git.tree(ours).await?, g.git.tree(theirs).await?);
    let fork = a.get(ROSTER) != b.get(ROSTER);
    if fork {
        // Roster forks merge by the fold (04 §4.8), never a file merge.
        let Some(folded) = crate::fold::of_merge(g, &parents) else {
            return Ok(None);
        };
        let doc = crate::roster_file::RosterDoc {
            roster: folded.roster,
        };
        let Ok(text) = crate::roster_file::render(&doc) else {
            return Ok(None);
        };
        let blob = g
            .git
            .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
            .await?;
        tree.insert(
            ROSTER.to_owned(),
            Entry {
                mode: "100644".to_owned(),
                kind: "blob".to_owned(),
                oid: blob.trim().to_owned(),
            },
        );
    }
    let oid = write_tree(&g.git, &tree).await?;
    let commit = g
        .git
        .commit_tree(&oid, &[ours, theirs], "pigeon: merge")
        .await?;
    judge_history(g, std::slice::from_ref(&commit)).await?;
    let outcome = g.lock().engine.outcome(&commit).cloned();
    let ok = outcome == Some(Outcome::Clean) || fork && outcome == Some(Outcome::Contribution);
    Ok(ok.then_some(commit))
}

/// Writes and judges the adoption of the proposal `x` on `main` (04 §4.5):
/// a merge of both whose tree is `proposal::adoption`, signed with the
/// device key, with the Force trailer when `force` gives a reason (04 §4.6).
/// Answers it if it is accepted.
pub async fn adopt(
    g: &Group,
    main: &str,
    x: &str,
    force: Option<&str>,
) -> Result<Option<String>, Error> {
    judge_history(g, &[main.to_owned(), x.to_owned()]).await?;
    let Some(base) = crate::proposal::base(g, x).await? else {
        return Ok(None);
    };
    let (at, ours) = {
        let mut k = g.lock();
        let at = k
            .engine
            .base(std::slice::from_ref(&base))
            .map(|v| v.tree.clone());
        let ours = k.engine.base(&[main.to_owned()]).map(|v| v.tree.clone());
        (at, ours)
    };
    let (Some(at), Some(ours)) = (at, ours) else {
        return Ok(None);
    };
    let raw = g.git.object(x).await?;
    let (utc, offset) = authority::author_time(&raw).unwrap_or((0, 0));
    let who = pigeon_core::sig::verify(&raw)
        .ok()
        .and_then(|signer| {
            let doc = g.roster()?;
            let d = doc.roster.devices.get(&signer)?;
            doc.roster.persons.get(&d.person).map(|p| p.name.clone())
        })
        .unwrap_or_else(|| "?".to_owned());
    let mine = g.git.tree(x).await?;
    let tree = pigeon_core::proposal::adoption(&at, &mine, &ours, &who, Stamp { utc, offset });
    let oid = write_tree(&g.git, &tree).await?;
    let message = match force {
        Some(why) => format!("pigeon: accept a suggestion\n\n{} {why}", authority::FORCE),
        None => "pigeon: accept a suggestion".to_owned(),
    };
    let commit = g.git.commit_tree(&oid, &[main, x], &message).await?;
    judge_history(g, std::slice::from_ref(&commit)).await?;
    let accepted = g.lock().engine.accepted(&commit);
    if !accepted {
        let v = g.lock().verdicts.get(&commit).cloned();
        tracing::warn!(group = %g.id, commit = %commit, verdict = ?v, "adoption-refused");
    }
    Ok(accepted.then_some(commit))
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
