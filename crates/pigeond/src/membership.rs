//! Derive purge-capable custody from accepted admission witnesses (ADR
//! 0078, proposed). Configured roles remain immutable. A rules-3 backup
//! without activation is pending, regardless of what its report claims.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::roster::{DeviceId, Edit, Role, Roster};

use crate::admission_file::{Request, Witness};
use crate::git::Error;
use crate::group::Group;
use crate::report_file::AdmissionKind;

/// One verified historical reading. No released-key or completion bit.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The one initial upgrade, if its certificate has been accepted.
    pub upgrade: Option<Request>,
    /// Eligible custodians still present in this roster.
    pub eligible: BTreeSet<String>,
    /// Configured backups awaiting their causal admission barrier.
    pub pending: BTreeMap<String, Request>,
    /// Initial requests awaiting agreement; normal sharing still works.
    pub requests: BTreeMap<String, Request>,
}

pub(crate) fn invalid(detail: &str) -> Error {
    Error::Parse {
        argv: vec!["custody membership".into()],
        detail: detail.into(),
    }
}

/// Roster of an independently accepted historical head.
pub async fn roster(g: &Group, main: &str) -> Result<Roster, Error> {
    if !g.lock().engine.accepted(main) {
        return Err(invalid("unaccepted membership head"));
    }
    let bytes = g
        .git
        .blob(&format!("{main}:.pigeon/roster.toml"))
        .await
        .ok_or_else(|| invalid("missing membership roster"))?;
    crate::roster_file::parse(&String::from_utf8_lossy(&bytes))
        .map(|d| d.roster)
        .map_err(|_| invalid("unknown membership roster"))
}

/// Non-recovery identities, including pending custodians.
pub fn participants(roster: &Roster) -> BTreeSet<String> {
    roster
        .devices
        .iter()
        .filter(|(_, d)| d.role != Role::Recovery)
        .map(|(id, _)| id.0.clone())
        .collect()
}

/// The configured custody set is not necessarily the eligible quorum.
pub fn configured(roster: &Roster) -> BTreeSet<String> {
    roster
        .devices
        .iter()
        .filter(|(_, d)| d.role == Role::Custodian)
        .map(|(id, _)| id.0.clone())
        .collect()
}

/// Cheap status/transport reading. A changed rules-3 roster waits until
/// the daemon has derived its new eligibility; old reports cannot bypass
/// this local check. Legacy groups continue sharing before the upgrade.
pub fn pending(g: &Group, device: &str) -> bool {
    let k = g.lock();
    let Some(roster) = k.roster.as_ref().map(|d| &d.roster) else {
        return true;
    };
    if !roster
        .devices
        .get(&DeviceId(device.into()))
        .is_some_and(|d| d.role == Role::Custodian)
    {
        return false;
    }
    if k.admission_error.is_some() {
        return true;
    }
    if roster.rules < pigeon_core::purge::RULES {
        return false;
    }
    match &k.membership {
        Some((main, state)) if k.roster_commit.as_ref() == Some(main) => {
            !state.eligible.contains(device)
        }
        _ => true,
    }
}

fn introduction(g: &Group, main: &str, device: &str) -> Result<String, Error> {
    let k = g.lock();
    let h = k
        .history
        .as_ref()
        .ok_or_else(|| invalid("missing accepted roster history"))?;
    let device = DeviceId(device.into());
    if h.roster.devices.contains_key(&device) {
        return Ok(h.genesis.clone());
    }
    h.canonical_order(&main.into())
        .into_iter()
        .find(|id| {
            h.commits.get(id).is_some_and(|c| {
                c.change
                    .iter()
                    .any(|e| matches!(e, Edit::AddDevice(id, _) if id == &device))
            })
        })
        .ok_or_else(|| invalid("missing configured backup introduction"))
}

/// Read only after full history admission. The judge validated every new
/// witness against its earlier closing head; it cannot authorize itself.
pub async fn read(g: &Group, main: &str) -> Result<State, Error> {
    let roster = roster(g, main).await?;
    let mut state = State::default();
    let mut activated = BTreeSet::new();
    for (path, entry) in g.git.tree(main).await? {
        if !path.starts_with(crate::admission_file::PREFIX) {
            continue;
        }
        let bytes = g.git.object(&entry.oid).await?;
        let text = std::str::from_utf8(&bytes).map_err(|_| invalid("admission encoding"))?;
        if path.starts_with(crate::admission_file::REQUESTS) {
            let request =
                Request::parse(&path, text).map_err(|_| invalid("unknown admission request"))?;
            state
                .requests
                .insert(request.id().map_err(|e| invalid(&e))?, request);
        } else {
            let witness =
                Witness::parse(&path, text).map_err(|_| invalid("unknown admission witness"))?;
            match witness.request.kind {
                AdmissionKind::Upgrade => {
                    if state
                        .upgrade
                        .as_ref()
                        .is_some_and(|old| old != &witness.request)
                    {
                        return Err(invalid("conflicting initial upgrade certificates"));
                    }
                    state.upgrade = Some(witness.request);
                }
                AdmissionKind::Custodian => {
                    activated.insert(
                        witness
                            .request
                            .device
                            .ok_or_else(|| invalid("missing pending device"))?,
                    );
                }
            }
        }
    }
    let configured = configured(&roster);
    let Some(upgrade) = &state.upgrade else {
        if roster.rules >= pigeon_core::purge::RULES {
            return Err(invalid("rules 3 has no verified initial upgrade"));
        }
        state.eligible = configured;
        return Ok(state);
    };
    let seed = self::roster(g, &upgrade.base).await?;
    state.eligible = self::configured(&seed)
        .union(&activated)
        .filter(|d| configured.contains(*d))
        .cloned()
        .collect();
    for device in configured.difference(&state.eligible) {
        state.pending.insert(
            device.clone(),
            Request {
                format: 1,
                kind: AdmissionKind::Custodian,
                base: introduction(g, main, device)?,
                device: Some(device.clone()),
            },
        );
    }
    Ok(state)
}
