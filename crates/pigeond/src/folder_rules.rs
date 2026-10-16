//! Supported folder-rule setup (04 §4.3). Names are resolved without
//! ambiguity. Core authority and full commit verification precede a CAS;
//! only roster metadata is staged, preserving personal file/index state.
use crate::{
    api::{problem, said},
    daemon::Daemon,
    group::Group,
};
use pigeon_core::{
    folder_rules::{Refusal, configure},
    roster::{PersonId, Role, Roster, Rule},
};
use pigeon_proto::{Arg, Response, policy};
use std::{path::Path, sync::Arc};

fn locate(d: &Daemon, folder: Option<&str>) -> Option<(Arc<Group>, Option<String>)> {
    match folder {
        None => Some((d.default_group()?, None)),
        Some(folder) => {
            let path = Path::new(folder);
            let group = d.group_at(path).or_else(|| d.group_at(path.parent()?))?;
            let rel = crate::history::relative(&group, path)?;
            Some((group, Some(rel)))
        }
    }
}

fn wire(path: String, rule: &Rule, direct: bool) -> policy::Rule {
    let (owner, stewards) = match rule {
        Rule::Owned(owner) => (Some(owner.0.clone()), Vec::new()),
        Rule::AddOnly(stewards) => (None, stewards.iter().map(|id| id.0.clone()).collect()),
    };
    policy::Rule {
        path,
        owner,
        stewards,
        direct,
    }
}

pub(crate) fn list(d: &Daemon, folder: Option<&str>) -> Vec<Response> {
    let Some((g, path)) = locate(d, folder) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let roster = &doc.roster;
    let me = roster
        .devices
        .get(&pigeon_core::roster::DeviceId(d.device.clone()));
    let can_edit = me.is_some_and(|device| {
        device.role != Role::Recovery && roster.persons.get(&device.person).is_some_and(|p| p.admin)
    });
    let people = roster
        .persons
        .iter()
        .map(|(id, p)| policy::Person {
            id: id.0.clone(),
            name: p.name.clone(),
            this_person: me.is_some_and(|m| m.person == *id),
            admin: p.admin,
        })
        .collect();
    let default = Rule::AddOnly(Default::default());
    let rules = if let Some(path) = path {
        let found = roster
            .folders
            .iter()
            .filter(|f| pigeon_core::restore::under(&path, &f.path))
            .max_by_key(|f| f.path.len());
        let direct = found.is_some_and(|f| f.path == path);
        vec![wire(path, found.map_or(&default, |f| &f.rule), direct)]
    } else {
        let mut rules = vec![wire(String::new(), &default, false)];
        rules.extend(
            roster
                .folders
                .iter()
                .map(|f| wire(f.path.clone(), &f.rule, true)),
        );
        rules
    };
    vec![Response::FolderRules(Box::new(policy::Report {
        root: g
            .worktree
            .as_ref()
            .unwrap_or(&g.git_dir)
            .to_string_lossy()
            .into_owned(),
        rules,
        people,
        can_edit,
    }))]
}

fn person(roster: &Roster, name: &str, me: &str) -> Option<PersonId> {
    if name == "me" {
        return roster
            .devices
            .get(&pigeon_core::roster::DeviceId(me.into()))
            .map(|d| d.person.clone());
    }
    if roster.persons.contains_key(&PersonId(name.into())) {
        return Some(PersonId(name.into()));
    }
    let mut people = roster
        .persons
        .iter()
        .filter(|(_, person)| person.name == name);
    let id = people.next()?.0.clone();
    people.next().is_none().then_some(id)
}

pub(crate) async fn set(d: &Daemon, folder: &str, request: policy::Change) -> Vec<Response> {
    let Some((g, Some(path))) = locate(d, Some(folder)) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let _busy = g.busy.lock().await;
    let Some(mut doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let resolve = |name: &str| person(&doc.roster, name, &d.device);
    let unknown = |name: &str| {
        problem(
            "folder-rule-person",
            &[("name", Arg::Text(name.into()))],
            None,
        )
    };
    let rule = match request {
        policy::Change::Inherit => None,
        policy::Change::Owned { person } => match resolve(&person) {
            Some(person) => Some(Rule::Owned(person)),
            None => return vec![unknown(&person)],
        },
        policy::Change::AddOnly { stewards } => match stewards
            .iter()
            .map(|p| resolve(p).ok_or(p.as_str()))
            .collect::<Result<_, _>>()
        {
            Ok(stewards) => Some(Rule::AddOnly(stewards)),
            Err(name) => return vec![unknown(name)],
        },
    };
    doc.roster = match configure(
        &doc.roster,
        &pigeon_core::roster::DeviceId(d.device.clone()),
        &path,
        rule,
    ) {
        Ok(roster) => roster,
        Err(Refusal::Authority) => return vec![problem("not-allowed", &[], None)],
        Err(Refusal::Path) => return vec![problem("folder-rule-path", &[], None)],
        Err(_) => return vec![problem("internal-error", &[], None)],
    };
    if g.roster().as_ref() == Some(&doc) {
        return vec![said("folder-rule-result", &[("folder", Arg::Text(path))])];
    }
    if let Err(error) = commit(&g, &doc, &d.tracer).await {
        return vec![problem("internal-error", &[], Some(error))];
    }
    d.wake.notify_one();
    vec![said("folder-rule-result", &[("folder", Arg::Text(path))])]
}

async fn commit(
    g: &Group,
    doc: &crate::roster_file::RosterDoc,
    tracer: &crate::trace::Tracer,
) -> Result<(), String> {
    let parent = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or("no accepted head")?;
    let mut tree = g.git.tree(&parent).await.map_err(|e| e.to_string())?;
    let text = crate::roster_file::render(doc)?;
    let blob = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    tree.insert(
        ".pigeon/roster.toml".into(),
        pigeon_core::authority::Entry {
            mode: "100644".into(),
            kind: "blob".into(),
            oid: blob.trim().into(),
        },
    );
    let tree = crate::merge::write_tree(&g.git, &tree)
        .await
        .map_err(|e| e.to_string())?;
    let commit = g
        .git
        .commit_tree(&tree, &[&parent], "pigeon: change folder sharing rule")
        .await
        .map_err(|e| e.to_string())?;
    tracer.event(crate::trace::Act::Commit, &[("commit", &commit)]);
    let judged = crate::verify::judge(g, None, &commit)
        .await
        .map_err(|e| e.to_string())?;
    if judged.accepted.as_deref() != Some(&commit) {
        return Err("folder rule did not pass authority verification".into());
    }
    g.git
        .update_ref("refs/heads/main", &commit, Some(&parent))
        .await
        .map_err(|e| e.to_string())?;
    crate::change::stage_file(
        &g.git,
        g.worktree.as_deref(),
        ".pigeon/roster.toml",
        text.as_bytes(),
    )
    .await
    .map_err(|e| e.to_string())?;
    g.reload_roster().await;
    Ok(())
}
