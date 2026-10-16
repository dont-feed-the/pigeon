//! Independently check signed custody admission evidence (proposed ADR
//! 0078). No present-day clock or report freshness changes an immutable
//! certificate. Missing proof objects remain provisional during fetching.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::admission::{Ack, Kind, View};
use pigeon_core::authority::{Tree, Unread};
use pigeon_core::roster::DeviceId;

use crate::admission_file::{Request, Witness};
use crate::git::Error;
use crate::group::Group;
use crate::membership::{self, invalid};
use crate::report_file::{AdmissionKind, Read};

/// Exact acknowledged reports, including first retained own reports.
pub const ACKS: &str = "refs/pigeon/admission-acks/";

/// Keep the first full signed report per request/device, before replacing
/// status. Readers call this only after verifying the report's signature
/// and admission; the generic object root is fetched with certificates.
pub async fn retain_ack(
    g: &Group,
    report: &crate::report_file::Status,
    oid: &str,
) -> Result<(), Error> {
    for ack in &report.acknowledgements {
        let kind = match ack.kind {
            AdmissionKind::Upgrade => "upgrade",
            AdmissionKind::Custodian => "custodian",
        };
        let name = format!("{}{}/{kind}/{}/{}", ACKS, g.id, ack.request, report.device);
        if g.git.rev(&name).await.is_none() {
            let root = format!("{}{oid}", crate::purge_epoch::PROOFS);
            match g.git.rev(&root).await {
                Some(old) if old != oid => {
                    return Err(invalid("acknowledgement proof root conflict"));
                }
                Some(_) => {}
                None => g.git.update_ref(&root, oid, None).await?,
            }
            g.git.update_ref(&name, oid, None).await?;
        }
    }
    Ok(())
}

/// Required report identities and all causal boundaries, independently
/// checked before passing the facts into the pure admission algebra.
pub async fn verify(g: &Group, witness: &Witness, parents: &[String]) -> Result<(), Error> {
    Box::pin(verify_inner(g, witness, parents)).await
}

async fn verify_inner(g: &Group, witness: &Witness, parents: &[String]) -> Result<(), Error> {
    let (path, text) = witness.render().map_err(|e| invalid(&e))?;
    Witness::parse(&path, &text).map_err(|_| invalid("unknown admission certificate"))?;
    let mut seen = false;
    for parent in parents {
        if g.lock().engine.accepted(parent) && g.git.contains(parent, &witness.main).await {
            seen = true
        }
    }
    if !seen {
        return Err(invalid(
            "admission closing head was not accepted before its witness",
        ));
    }
    let main = &witness.main;
    let request = &witness.request;
    let state = membership::read(g, main).await?;
    let roster = membership::roster(g, main).await?;
    let base = membership::roster(g, &request.base).await?;
    if !g.git.contains(main, &request.base).await {
        return Err(invalid("admission snapshot is outside closing history"));
    }
    let request_id = request.id().map_err(|e| invalid(&e))?;
    let kind = match request.kind {
        AdmissionKind::Upgrade => {
            if !(1..pigeon_core::purge::RULES).contains(&base.rules)
                || state.upgrade.as_ref().is_some_and(|old| old != request)
                || state.requests.get(&request_id) != Some(request)
                || membership::participants(&base) != membership::participants(&roster)
                || membership::configured(&base) != membership::configured(&roster)
            {
                return Err(invalid(
                    "initial upgrade membership changed or another upgrade was chosen",
                ));
            }
            Kind::Upgrade
        }
        AdmissionKind::Custodian => {
            let device = request
                .device
                .as_ref()
                .ok_or_else(|| invalid("missing pending backup"))?;
            if state.upgrade.is_none() || state.pending.get(device) != Some(request) {
                return Err(invalid(
                    "request is not this backup's actual pending introduction",
                ));
            }
            Kind::Custodian(device.clone())
        }
    };
    let active = crate::purge_state::active(g, main).await?;
    let epochs = crate::purge_epoch::collect(g, main).await?;
    let unclosed = active.keys().any(|id| !epochs.contains_key(id));
    let participants = membership::participants(&roster);
    let mut acks = BTreeMap::new();
    for (device, oid) in &witness.reports {
        let raw = g.git.object(oid).await?;
        if pigeon_core::sig::verify(&raw).ok() != Some(DeviceId(device.clone())) {
            return Err(invalid("admission report signature"));
        }
        let bytes = g
            .git
            .blob(&format!("{oid}:status.toml"))
            .await
            .ok_or_else(|| invalid("missing acknowledgement body"))?;
        let Read::Known(report) = crate::report_file::parse(&String::from_utf8_lossy(&bytes))
        else {
            return Err(invalid("unknown acknowledgement report"));
        };
        if report.device != *device
            || (report.lab && !pigeon_sys::lab::LAB)
            || report
                .custody
                .as_ref()
                .is_some_and(|c| c.repository != "ok")
        {
            return Err(invalid("ineligible acknowledgement report"));
        }
        let offered: Vec<_> = report
            .acknowledgements
            .iter()
            .filter(|a| a.kind == request.kind && a.request == request_id)
            .collect();
        if offered.len() != 1 || offered[0].protocol != 2 {
            return Err(invalid("missing unique protocol-2 acknowledgement"));
        }
        let head = report
            .main
            .ok_or_else(|| invalid("acknowledgement has no accepted main"))?;
        if !g.lock().engine.accepted(&head)
            || !g.git.contains(&head, &request.base).await
            || !g.git.contains(main, &head).await
        {
            return Err(invalid(
                "acknowledgement history is absent from closing main",
            ));
        }
        if request.kind == AdmissionKind::Upgrade {
            let path = request.path().map_err(|e| invalid(&e))?;
            let bytes = g
                .git
                .blob(&format!("{head}:{path}"))
                .await
                .ok_or_else(|| invalid("acknowledgement precedes upgrade request"))?;
            if Request::parse(&path, &String::from_utf8_lossy(&bytes))
                .ok()
                .as_ref()
                != Some(request)
            {
                return Err(invalid(
                    "acknowledgement names a different upgrade snapshot",
                ));
            }
        }
        acks.insert(
            device.clone(),
            Ack {
                main: head,
                protocol: offered[0].protocol,
            },
        );
    }
    let view = View {
        main,
        introduced: &request.base,
        kind: &kind,
        custodians: &state.eligible,
        participants: &participants,
        acknowledgements: &acks,
        unclosed_purges: unclosed,
    };
    // Every ancestry boundary was independently checked above. Keep the
    // pure quorum and capability rule as the sole admission decision.
    if view.required() != witness.reports.keys().cloned().collect() || !view.ready(|_, _| true) {
        return Err(invalid(
            "admission quorum is incomplete or old erasures remain open",
        ));
    }
    Ok(())
}

/// Retain exact full reports before accepting or publishing a witness.
pub async fn retain(g: &Group, witness: &Witness) -> Result<(), Error> {
    for oid in witness.reports.values() {
        let name = format!("{}{oid}", crate::purge_epoch::PROOFS);
        match g.git.rev(&name).await {
            Some(old) if old == *oid => {}
            Some(_) => return Err(invalid("admission proof root names a different object")),
            None => g.git.update_ref(&name, oid, None).await?,
        }
    }
    Ok(())
}

/// Facts for the pure authority rule. Request scope is accepted history,
/// not just a syntactically valid SHA supplied by a peer.
pub(crate) async fn checks(
    g: &Group,
    after: &Tree,
    changed: &BTreeSet<String>,
    parents: &[String],
) -> Result<BTreeMap<String, Result<(), Unread>>, Error> {
    let mut out = BTreeMap::new();
    for path in changed
        .iter()
        .filter(|p| p.starts_with(crate::admission_file::PREFIX))
    {
        let Some(entry) = after.get(path) else {
            continue;
        };
        let bytes = g.git.object(&entry.oid).await?;
        let value = match std::str::from_utf8(&bytes) {
            Err(_) => Err(Unread::Malformed("admission encoding".into())),
            Ok(text) if path.starts_with(crate::admission_file::REQUESTS) => {
                match Request::parse(path, text) {
                    Err(e) => Err(e),
                    Ok(request) => {
                        let mut seen = false;
                        for parent in parents {
                            if g.lock().engine.accepted(parent)
                                && g.git.contains(parent, &request.base).await
                            {
                                seen = true
                            }
                        }
                        if !seen {
                            Err(Unread::Malformed("unaccepted upgrade snapshot".into()))
                        } else {
                            Ok(())
                        }
                    }
                }
            }
            Ok(text) => match Witness::parse(path, text) {
                Err(e) => Err(e),
                Ok(witness) => {
                    let ids: Vec<_> = witness.reports.values().cloned().collect();
                    let objects = g.git.objects(&ids).await?;
                    if objects.len() != ids.len() || objects.iter().any(Option::is_none) {
                        Err(Unread::Unknown("admission proof objects".into()))
                    } else {
                        match verify(g, &witness, parents).await {
                            Ok(()) => {
                                retain(g, &witness).await?;
                                Ok(())
                            }
                            Err(Error::Parse { detail, .. }) => Err(Unread::Malformed(detail)),
                            Err(_) => Err(Unread::Unknown("admission proof history".into())),
                        }
                    }
                }
            },
        };
        out.insert(path.clone(), value);
    }
    Ok(out)
}

/// Independent evidence for the roster's first rules-3 transition. A
/// previously accepted certificate remains valid; it need not be rewritten
/// with the roster. Two different initial choices can never form a quorum.
pub(crate) async fn upgrade(
    g: &Group,
    after: &Tree,
    parents: &[String],
) -> Result<Result<(), Unread>, Error> {
    let mut chosen = None;
    for (path, entry) in after
        .iter()
        .filter(|(p, _)| p.starts_with(crate::admission_file::WITNESSES))
    {
        let bytes = g.git.object(&entry.oid).await?;
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let Ok(witness) = Witness::parse(path, text) else {
            continue;
        };
        if witness.request.kind != AdmissionKind::Upgrade {
            continue;
        }
        if chosen.as_ref().is_some_and(|old| old != &witness.request) {
            return Ok(Err(Unread::Malformed(
                "conflicting initial upgrade certificates".into(),
            )));
        }
        let ids: Vec<_> = witness.reports.values().cloned().collect();
        if g.git.objects(&ids).await?.iter().any(Option::is_none) {
            return Ok(Err(Unread::Unknown("upgrade proof objects".into())));
        }
        if let Err(error) = verify(g, &witness, parents).await {
            return Ok(Err(Unread::Unknown(error.to_string())));
        }
        chosen = Some(witness.request);
    }
    Ok(if chosen.is_some() {
        Ok(())
    } else {
        Err(Unread::Unknown("upgrade agreement".into()))
    })
}
