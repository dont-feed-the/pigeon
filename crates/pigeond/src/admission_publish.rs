//! Make admission requests and publish independently verifiable causal
//! certificates (proposed ADR 0078). Every commit uses a private tree and
//! a main CAS. No private working or staged file enters these commits.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::authority::{Entry, ROSTER, Tree};
use pigeon_core::roster::DeviceId;

use crate::admission_file::{Request, Witness};
use crate::git::Error;
use crate::group::Group;
use crate::membership::{self, invalid};
use crate::report_file::{AdmissionKind, Read};

fn admin(g: &Group) -> bool {
    let Some(me) = crate::git::signer_device() else {
        return false;
    };
    g.roster().is_some_and(|doc| {
        doc.roster
            .devices
            .get(&DeviceId(me))
            .and_then(|d| doc.roster.persons.get(&d.person))
            .is_some_and(|p| p.admin)
    })
}

/// Explicit preparation by an admin, before an erasure can be submitted.
/// Reuse an existing request so ordinary repeated clicks cannot split votes.
/// Caller holds `busy`; no request is made by a read-only preview.
pub async fn start(g: &Group) -> Result<String, Error> {
    if !admin(g) {
        return Err(invalid("ask a folder admin to prepare erasing"));
    }
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| invalid("no main"))?;
    crate::merge::judge_history(g, std::slice::from_ref(&main)).await?;
    let state = membership::read(g, &main).await?;
    if let Some(upgrade) = state.upgrade {
        return upgrade.id().map_err(|e| invalid(&e));
    }
    if let Some((id, _)) = state.requests.first_key_value() {
        return Ok(id.clone());
    }
    if state.eligible.is_empty() {
        return Err(invalid("add a backup before preparing erasing"));
    }
    let request = Request {
        format: 1,
        kind: AdmissionKind::Upgrade,
        base: main.clone(),
        device: None,
    };
    let mut tree = g.git.tree(&main).await?;
    let path = request.path().map_err(|e| invalid(&e))?;
    let text = request.render().map_err(|e| invalid(&e))?;
    insert(g, &mut tree, path, &text).await?;
    commit(
        g,
        &main,
        &tree,
        "pigeon: request erasure protocol readiness",
    )
    .await?;
    request.id().map_err(|e| invalid(&e))
}

/// At most one certificate per round. Missing or not-yet-merged reports
/// simply wait; they never weaken the required quorum.
pub async fn round(g: &Group) -> Result<bool, Error> {
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| invalid("no main"))?;
    let state = membership::read(g, &main).await?;
    let roster = membership::roster(g, &main).await?;
    if state.upgrade.is_some() && roster.rules < pigeon_core::purge::RULES && admin(g) {
        let mut tree = g.git.tree(&main).await?;
        set_rules(g, &mut tree, roster).await?;
        commit(g, &main, &tree, "pigeon: enable verified erasure protocol").await?;
        return Ok(true);
    }
    let requests: Vec<_> = if state.upgrade.is_none() {
        state.requests.values().cloned().collect()
    } else {
        state.pending.values().cloned().collect()
    };
    for request in requests {
        let kind = match &request.device {
            Some(device) => pigeon_core::admission::Kind::Custodian(device.clone()),
            None => pigeon_core::admission::Kind::Upgrade,
        };
        let participants = membership::participants(&roster);
        let view = pigeon_core::admission::View {
            main: &main,
            introduced: &request.base,
            kind: &kind,
            custodians: &state.eligible,
            participants: &participants,
            acknowledgements: &BTreeMap::new(),
            unclosed_purges: false,
        };
        let required = view.required();
        if required.is_empty() {
            continue;
        }
        let reports = offers(g, &main, &request, &required).await?;
        if reports.len() != required.len() {
            continue;
        }
        let witness = Witness {
            format: 1,
            request,
            main: main.clone(),
            reports,
        };
        // Known old erasures, changed snapshots and unsupported report
        // histories must settle before any activation can be published.
        if crate::admission_proofs::verify(g, &witness, std::slice::from_ref(&main))
            .await
            .is_err()
        {
            continue;
        }
        crate::admission_proofs::retain(g, &witness).await?;
        let (path, text) = witness.render().map_err(|e| invalid(&e))?;
        let mut tree = g.git.tree(&main).await?;
        insert(g, &mut tree, path, &text).await?;
        if witness.request.kind == AdmissionKind::Upgrade && admin(g) {
            set_rules(g, &mut tree, roster).await?;
        }
        commit(g, &main, &tree, "pigeon: retain verified custody admission").await?;
        return Ok(true);
    }
    Ok(false)
}

pub(crate) async fn offers(
    g: &Group,
    main: &str,
    request: &Request,
    required: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>, Error> {
    let id = request.id().map_err(|e| invalid(&e))?;
    let refs = g
        .git
        .run(&[
            "for-each-ref",
            "--format=%(objectname)",
            crate::admission_proofs::ACKS,
            crate::purge_epoch::PROOFS,
            "refs/pigeon/status/",
            "refs/pigeon/reports/",
        ])
        .await?;
    let mut out = BTreeMap::new();
    for oid in refs.lines() {
        let raw = g.git.object(oid).await?;
        let Ok(signer) = pigeon_core::sig::verify(&raw) else {
            continue;
        };
        if !required.contains(&signer.0) || out.contains_key(&signer.0) {
            continue;
        }
        let Some(bytes) = g.git.blob(&format!("{oid}:status.toml")).await else {
            continue;
        };
        let Read::Known(report) = crate::report_file::parse(&String::from_utf8_lossy(&bytes))
        else {
            continue;
        };
        if report.device != signer.0
            || (report.lab && !pigeon_sys::lab::LAB)
            || report
                .custody
                .as_ref()
                .is_some_and(|c| c.repository != "ok")
        {
            continue;
        }
        let Some(head) = report.main else { continue };
        if !g.lock().engine.accepted(&head)
            || !g.git.contains(main, &head).await
            || !g.git.contains(&head, &request.base).await
        {
            continue;
        }
        if request.kind == AdmissionKind::Upgrade {
            let path = request.path().map_err(|e| invalid(&e))?;
            let Some(bytes) = g.git.blob(&format!("{head}:{path}")).await else {
                continue;
            };
            if Request::parse(&path, &String::from_utf8_lossy(&bytes))
                .ok()
                .as_ref()
                != Some(request)
            {
                continue;
            }
        }
        if report
            .acknowledgements
            .iter()
            .filter(|a| a.kind == request.kind && a.request == id && a.protocol == 2)
            .count()
            == 1
        {
            out.insert(signer.0, oid.into());
        }
    }
    Ok(out)
}

async fn set_rules(
    g: &Group,
    tree: &mut Tree,
    mut roster: pigeon_core::roster::Roster,
) -> Result<(), Error> {
    roster.rules = pigeon_core::purge::RULES;
    let text = crate::roster_file::render(&crate::roster_file::RosterDoc { roster })
        .map_err(|e| invalid(&e))?;
    insert(g, tree, ROSTER.into(), &text).await
}

async fn insert(g: &Group, tree: &mut Tree, path: String, text: &str) -> Result<(), Error> {
    let oid = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await?;
    tree.insert(
        path,
        Entry {
            mode: "100644".into(),
            kind: "blob".into(),
            oid: oid.trim().into(),
        },
    );
    Ok(())
}

async fn commit(g: &Group, parent: &str, tree: &Tree, message: &str) -> Result<(), Error> {
    let tree = crate::merge::write_tree(&g.git, tree).await?;
    let commit = g.git.commit_tree(&tree, &[parent], message).await?;
    crate::merge::judge_history(g, std::slice::from_ref(&commit)).await?;
    if !g.lock().engine.accepted(&commit) {
        return Err(invalid("admission commit was not accepted"));
    }
    g.git
        .update_ref("refs/heads/main", &commit, Some(parent))
        .await?;
    g.reload_roster().await;
    g.lock().roster_index_pending = true;
    Ok(())
}
