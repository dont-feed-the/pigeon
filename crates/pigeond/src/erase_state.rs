//! Read-only erasure readiness and waiting names (08 §8.6). Every positive
//! claim comes from accepted metadata and full signed reports. Observing
//! status never creates a protocol request or a retained proof reference.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_proto::erase::{Decision, Readiness, Waiting};

use crate::git::Error;
use crate::group::Group;

fn names(roster: &pigeon_core::roster::Roster, ids: BTreeSet<String>) -> Vec<Waiting> {
    ids.into_iter()
        .map(|id| Waiting {
            name: roster
                .devices
                .get(&pigeon_core::roster::DeviceId(id.clone()))
                .map_or_else(|| id.clone(), |d| d.name.clone()),
            device: id,
        })
        .collect()
}

/// Eligibility and capability are separate from submitting an erasure.
pub async fn readiness(g: &Group, main: &str) -> Result<Readiness, Error> {
    let state = crate::membership::read(g, main).await?;
    let roster = crate::membership::roster(g, main).await?;
    let participants = crate::membership::participants(&roster);
    let active = crate::purge_state::active(g, main).await?;
    let epochs = crate::purge_epoch::read(g, main).await?;
    let open_decisions: Vec<_> = if state.pending.is_empty() {
        Vec::new()
    } else {
        active
            .keys()
            .filter(|id| !epochs.contains_key(*id))
            .cloned()
            .collect()
    };
    let preparation_needed = state.upgrade.is_none() && state.requests.is_empty();
    let mut missing = if preparation_needed {
        participants.clone()
    } else {
        BTreeSet::new()
    };
    let requests: Vec<_> = if state.upgrade.is_none() {
        state.requests.values().take(1).collect()
    } else {
        state.pending.values().collect()
    };
    for request in requests {
        let kind = match &request.device {
            Some(device) => pigeon_core::admission::Kind::Custodian(device.clone()),
            None => pigeon_core::admission::Kind::Upgrade,
        };
        let view = pigeon_core::admission::View {
            main: &main.to_owned(),
            introduced: &request.base,
            kind: &kind,
            custodians: &state.eligible,
            participants: &participants,
            acknowledgements: &BTreeMap::new(),
            unclosed_purges: false,
        };
        let required = view.required();
        let offered = crate::admission_publish::offers(g, main, request, &required).await?;
        missing.extend(required.into_iter().filter(|id| !offered.contains_key(id)));
    }
    let problem = if g.lock().admission_error.is_some() {
        Some(crate::status::msg("erase-checking", &[]))
    } else if crate::membership::configured(&roster).is_empty() {
        Some(crate::status::msg("erase-needs-backup", &[]))
    } else {
        None
    };
    let can_prepare = crate::git::signer_device()
        .and_then(|id| roster.devices.get(&pigeon_core::roster::DeviceId(id)))
        .filter(|d| d.role != pigeon_core::roster::Role::Recovery)
        .and_then(|d| roster.persons.get(&d.person))
        .is_some_and(|p| p.admin);
    Ok(Readiness {
        can_prepare,
        ready: roster.rules >= 2
            && !state.eligible.is_empty()
            && state.pending.is_empty()
            && problem.is_none(),
        preparation_needed,
        waiting: names(&roster, missing),
        open_decisions,
        problem,
    })
}

/// Accepted decisions and their exact missing causal seals. A later empty
/// report cannot revoke completion evidence or an earlier retained seal.
pub async fn decisions(g: &Group, main: &str) -> Result<Vec<Decision>, Error> {
    let active = crate::purge_state::active(g, main).await?;
    let epochs = crate::purge_epoch::read(g, main).await?;
    let proofs = crate::purge_proofs::read(g, &active).await?;
    let roster = crate::membership::roster(g, main).await?;
    let current = crate::membership::configured(&roster);
    let mut out = Vec::new();
    for (id, active) in active {
        let original = crate::membership::roster(g, &active.introduced).await?;
        let custodians = crate::membership::configured(&original);
        let completed = epochs.contains_key(&id);
        let mut missing = BTreeSet::new();
        if !completed {
            for device in &custodians {
                match proofs.get(&(id.clone(), device.clone())) {
                    Some(seal) if g.git.contains(main, &seal.main).await => {}
                    _ => {
                        missing.insert(device.clone());
                    }
                }
            }
        }
        out.push(Decision {
            id,
            paths: active.doc.paths,
            not_before: active.doc.retention_end.to_string(),
            waiting: names(&original, missing),
            completed,
            membership_changed: !completed && custodians != current,
        });
    }
    Ok(out)
}
