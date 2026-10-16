//! Roster observations from accepted Git history and the daemon's loaded
//! roster. The fold's inputs are recorded; expected rosters are never
//! computed here. Raw rosters remain available for richer projections.

use pigeon_core::roster::{Edit, Field, Roster, Value as RValue};
use serde_json::{Value, json};

use crate::{group::Group, trace::Tracer};

fn text(s: &str) -> String {
    crate::util::hex(s.as_bytes())
}

fn roster(r: &Roster) -> Result<String, String> {
    crate::roster_file::render(&crate::roster_file::RosterDoc { roster: r.clone() })
        .map(|s| text(&s))
}

fn value(v: &RValue) -> Value {
    match v {
        RValue::Text(s) => json!({"kind": "text", "value": text(s)}),
        RValue::Number(n) => json!({"kind": "number", "value": n}),
        RValue::Flag(b) => json!({"kind": "flag", "value": b}),
        RValue::Unset => json!({"kind": "unset"}),
    }
}

fn edit(e: &Edit) -> Value {
    match e {
        Edit::Set { field, from, to } => {
            let (field, id) = match field {
                Field::Group => ("group", String::new()),
                Field::K => ("k", String::new()),
                Field::RetentionDays => ("retention", String::new()),
                Field::Rules => ("rules", String::new()),
                Field::Relay => ("relay", String::new()),
                Field::PersonName(p) => ("person_name", text(&p.0)),
                Field::PersonAdmin(p) => ("admin", text(&p.0)),
                Field::DeviceName(d) => ("device_name", d.0.clone()),
                Field::DeviceMaxsize(d) => ("maxsize", d.0.clone()),
            };
            json!({"kind": "set", "field": field, "id": id, "from": value(from), "to": value(to)})
        }
        Edit::AddPerson(p, person) => {
            json!({"kind": "add_person", "id": text(&p.0), "name": text(&person.name), "admin": person.admin})
        }
        Edit::AddDevice(d, device) => {
            json!({"kind": "add", "id": d.0, "person": text(&device.person.0), "role": format!("{:?}", device.role), "name": text(&device.name), "uuid": device.annex_uuid, "maxsize": device.maxsize})
        }
        Edit::RemoveDevice(d) => json!({"kind": "rem", "id": d.0}),
        Edit::RemovePerson(p) => json!({"kind": "remp", "id": text(&p.0)}),
        Edit::AddFolder(f) => {
            json!({"kind": "add_folder", "path": text(&f.path), "rule": format!("{:?}", f.rule)})
        }
        Edit::RemoveFolder(path) => json!({"kind": "rem_folder", "path": text(path)}),
    }
}

async fn observe(g: &Group, before: Option<&str>) -> Result<Value, String> {
    let head = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or("no observed main")?;
    let mut tips = vec![head.clone()];
    if let Some(before) = before {
        tips.push(before.to_owned());
    }
    crate::merge::judge_history(g, &tips)
        .await
        .map_err(|e| e.to_string())?;
    let history = g
        .lock()
        .history
        .clone()
        .ok_or("no accepted roster history")?;
    let mut ids = vec![history.genesis.clone()];
    ids.extend(history.commits.keys().cloned());
    let mut commits = Vec::new();
    for id in ids {
        let tree = g.git.tree(&id).await.map_err(|e| e.to_string())?;
        let entry = tree
            .get(".pigeon/roster.toml")
            .ok_or("commit has no roster")?;
        let raw = g.git.object(&entry.oid).await.map_err(|e| e.to_string())?;
        let body = g.git.object(&id).await.map_err(|e| e.to_string())?;
        let signer = pigeon_core::sig::verify(&body).map_err(|e| format!("{e:?}"))?;
        let c = history.commits.get(&id);
        commits.push(json!({
            "id": id, "signer": signer.0,
            "parents": c.map_or_else(Vec::new, |c| c.parents.clone()),
            "time": pigeon_core::authority::author_time(&body).ok_or("commit has no author time")?.0,
            "edits": c.map_or_else(Vec::new, |c| c.change.iter().map(edit).collect()),
            "roster": text(std::str::from_utf8(&raw).map_err(|e| e.to_string())?),
        }));
    }
    let loaded = g.roster().ok_or("daemon has no loaded roster")?;
    Ok(
        json!({"group": g.id, "before": before.unwrap_or(""), "head": head,
        "genesis": history.genesis, "commits": commits, "loaded": roster(&loaded.roster)?}),
    )
}

/// Observe a completed ref move after reloading the daemon's actual roster.
pub async fn moved(t: &Tracer, g: &Group, action: &str, before: Option<&str>) {
    if !t.enabled() {
        return;
    }
    let vars = match observe(g, before).await {
        Ok(vars) => vars,
        Err(error) => json!({"group": g.id, "error": text(&error)}),
    };
    t.observed("Roster", action, vars);
}
