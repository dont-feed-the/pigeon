//! Deciding suggestions (04 §4.5, 10 §10.1): `pigeon suggestions` lists the
//! pending proposals this device keeps, and `pigeon accept` adopts one on
//! `main` (`merge::adopt`), with `--override` when nobody may make its
//! change and an admin forces it (04 §4.6). A proposal is pending while
//! `main` does not contain it and no other kept proposal builds on it (a
//! later save stacked one) and `main` holds no decision on it; its deciders
//! are recomputed on `main` each time. `pigeon decline` commits the
//! decision `rejected` (`decision_file`), which a decider may sign.

use pigeon_core::authority::{self, Deciders};
use pigeon_core::roster::{DeviceId, PersonId};
use pigeon_proto::api::Suggestion;
use pigeon_proto::{Arg, Response};

use crate::daemon::Daemon;
use crate::git::Error;
use crate::group::Group;

/// One pending proposal.
pub struct Pending {
    /// Its commit.
    pub id: String,
    /// The device that signed it.
    pub device: String,
    /// Its author's person.
    pub person: Option<PersonId>,
    /// When it was saved, seconds.
    pub saved: i64,
    /// The paths it changes on its base.
    pub paths: Vec<String>,
    /// Its deciders on `main`.
    pub deciders: Deciders,
}

/// The pending proposals of `g`, oldest first.
pub async fn pending(g: &Group) -> Result<Vec<Pending>, Error> {
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return Ok(Vec::new());
    };
    let kept = crate::proposal::kept(&g.git).await;
    let mut tips = vec![main.clone()];
    tips.extend(kept.iter().map(|(_, c)| c.clone()));
    crate::merge::judge_history(g, &tips).await?;
    let Some(doc) = g.roster() else {
        return Ok(Vec::new());
    };
    let main_tree = g.git.tree(&main).await?;
    let mut out = Vec::new();
    for (device, c) in &kept {
        if main_tree.contains_key(&authority::decision_path(c)) || g.git.contains(&main, c).await {
            continue;
        }
        let mut stacked = false;
        for (_, other) in &kept {
            if other != c && g.git.contains(other, c).await {
                stacked = true;
                break;
            }
        }
        if stacked {
            continue;
        }
        let Some(base) = crate::proposal::base(g, c).await? else {
            continue;
        };
        let at = g
            .lock()
            .engine
            .base(std::slice::from_ref(&base))
            .map(|v| v.tree.clone());
        let Some(at) = at else {
            continue;
        };
        let mine = g.git.tree(c).await?;
        let paths: Vec<String> = authority::delta(&at, &mine).into_iter().collect();
        let adoption = pigeon_core::proposal::adoption_changes(&at, &mine, &main_tree);
        let changes: Vec<(&str, bool)> = adoption.iter().map(|(p, a)| (p.as_str(), *a)).collect();
        let raw = g.git.object(c).await?;
        out.push(Pending {
            id: c.clone(),
            device: device.clone(),
            person: doc
                .roster
                .devices
                .get(&DeviceId(device.clone()))
                .map(|d| d.person.clone()),
            saved: authority::author_time(&raw).map_or(0, |t| t.0),
            deciders: authority::deciders(&doc.roster, &changes),
            paths,
        });
    }
    out.sort_by_key(|p| p.saved);
    Ok(out)
}

fn problem(code: &str, args: &[(&str, Arg)]) -> Vec<Response> {
    vec![crate::api::problem(code, args, None)]
}

/// This device's person in `g`'s roster.
fn me(d: &Daemon, g: &Group) -> Option<PersonId> {
    let doc = g.roster()?;
    let device = doc.roster.devices.get(&DeviceId(d.device.clone()))?;
    Some(device.person.clone())
}

/// `pigeon suggestions`.
pub async fn list(d: &Daemon) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return problem("not-in-group", &[]);
    };
    let pending = match pending(&g).await {
        Ok(p) => p,
        Err(e) => {
            return vec![crate::api::problem(
                "internal-error",
                &[],
                Some(e.to_string()),
            )];
        }
    };
    let me = me(d, &g);
    let doc = g.roster();
    let name_of = |p: &Option<PersonId>| {
        p.as_ref()
            .and_then(|p| doc.as_ref()?.roster.persons.get(p).map(|x| x.name.clone()))
            .unwrap_or_default()
    };
    let device_of = |id: &str| {
        doc.as_ref()
            .and_then(|x| x.roster.devices.get(&DeviceId(id.to_owned())))
            .map(|x| x.name.clone())
            .unwrap_or_default()
    };
    let proposals = pending
        .iter()
        .map(|p| Suggestion {
            id: p.id.clone(),
            author: name_of(&p.person),
            device: device_of(&p.device),
            saved_at: crate::util::rfc3339(p.saved),
            paths: p.paths.clone(),
            decide: me.as_ref().is_some_and(|u| p.deciders.0.contains(u)),
            force: p.deciders.1,
            mine: p.person.is_some() && p.person == me,
        })
        .collect();
    vec![Response::Proposals { proposals }]
}

/// The one pending proposal of `g` whose commit starts with `id`, which
/// this device's person decides; else the problem to answer.
async fn decided(d: &Daemon, g: &Group, id: &str) -> Result<Pending, Vec<Response>> {
    let pending = match pending(g).await {
        Ok(p) => p,
        Err(e) => {
            let p = crate::api::problem("internal-error", &[], Some(e.to_string()));
            return Err(vec![p]);
        }
    };
    let mut found: Vec<Pending> = pending
        .into_iter()
        .filter(|p| id.len() >= 4 && p.id.starts_with(id))
        .collect();
    if found.len() != 1 {
        return Err(problem(
            "suggestion-unknown",
            &[("id", Arg::Text(id.to_owned()))],
        ));
    }
    let p = found.remove(0);
    if !me(d, g).is_some_and(|u| p.deciders.0.contains(&u)) {
        return Err(problem("not-allowed", &[]));
    }
    Ok(p)
}

/// The name of the author of `p`.
fn author(g: &Group, p: &Pending) -> String {
    p.person
        .as_ref()
        .and_then(|u| g.roster()?.roster.persons.get(u).map(|x| x.name.clone()))
        .unwrap_or_default()
}

/// `pigeon accept ID [--override REASON]`.
pub async fn accept(d: &Daemon, id: &str, reason: Option<&str>) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return problem("not-in-group", &[]);
    };
    let p = match decided(d, &g, id).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    let force = match (p.deciders.1, reason) {
        (true, None) => return problem("needs-override", &[]),
        (true, Some(r)) => Some(r),
        (false, _) => None,
    };
    let _busy = g.busy.lock().await;
    let Some(main) = g.git.rev("refs/heads/main").await else {
        return problem("not-in-group", &[]);
    };
    let adopted = match crate::merge::adopt(&g, &main, &p.id, force).await {
        Ok(Some(c)) => c,
        Ok(None) => return problem("not-allowed", &[]),
        Err(e) => {
            return vec![crate::api::problem(
                "internal-error",
                &[],
                Some(e.to_string()),
            )];
        }
    };
    if let Err(e) = g
        .git
        .update_ref("refs/heads/main", &adopted, Some(&main))
        .await
    {
        return vec![crate::api::problem(
            "internal-error",
            &[],
            Some(e.to_string()),
        )];
    }
    tracing::info!(group = %g.id, proposal = %p.id, commit = %adopted, "suggestion-accepted");
    drop(_busy);
    crate::api::wake_soon(d);
    let author = author(&g, &p);
    vec![crate::api::said(
        "accepted-suggestion",
        &[("author", Arg::Text(author))],
    )]
}

/// `pigeon decline ID [--note TEXT]`: commits the decision `rejected`.
pub async fn decline(d: &Daemon, id: &str, note: Option<&str>) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return problem("not-in-group", &[]);
    };
    let p = match decided(d, &g, id).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    let doc = crate::decision_file::DecisionDoc {
        proposal: p.id.clone(),
        verdict: authority::Decided::Rejected,
        note: note.map(str::to_owned),
    };
    let _busy = g.busy.lock().await;
    let done = async {
        let main = g
            .git
            .rev("refs/heads/main")
            .await
            .ok_or_else(|| "no main".to_owned())?;
        let text = crate::decision_file::render(&doc);
        let blob = g
            .git
            .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut tree = g.git.tree(&main).await.map_err(|e| e.to_string())?;
        tree.insert(
            authority::decision_path(&p.id),
            authority::Entry {
                mode: "100644".to_owned(),
                kind: "blob".to_owned(),
                oid: blob.trim().to_owned(),
            },
        );
        let oid = crate::merge::write_tree(&g.git, &tree)
            .await
            .map_err(|e| e.to_string())?;
        let commit = g
            .git
            .commit_tree(&oid, &[&main], "pigeon: decline a suggestion")
            .await
            .map_err(|e| e.to_string())?;
        crate::merge::judge_history(&g, std::slice::from_ref(&commit))
            .await
            .map_err(|e| e.to_string())?;
        if !g.lock().engine.accepted(&commit) {
            let v = g.lock().verdicts.get(&commit).cloned();
            return Err(format!("not accepted: {v:?}"));
        }
        g.git
            .update_ref("refs/heads/main", &commit, Some(&main))
            .await
            .map_err(|e| e.to_string())?;
        Ok::<String, String>(commit)
    };
    let commit = match done.await {
        Ok(c) => c,
        Err(e) => return vec![crate::api::problem("internal-error", &[], Some(e))],
    };
    tracing::info!(group = %g.id, proposal = %p.id, commit = %commit, "suggestion-declined");
    drop(_busy);
    crate::api::wake_soon(d);
    vec![crate::api::said(
        "declined-suggestion",
        &[("author", Arg::Text(author(&g, &p)))],
    )]
}
