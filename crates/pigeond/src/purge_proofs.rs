//! Retain signed seals independently of replaceable status refs (08 §8.6).
//! A later report or a lost receiver ledger cannot revoke an earlier seal.
//! Only current custodians, verified accepted seal histories, exact active
//! identities and immutable seal values count; any conflict stops release.

use std::collections::BTreeMap;

use pigeon_core::purge::Seal;
use pigeon_core::roster::{DeviceId, Role};

use crate::git::Error;
use crate::group::Group;
use crate::purge_state::Active;
use crate::report_file::{Purge, Read};

/// Local roots keep complete signed report objects, not unsigned seal data.
const PREFIX: &str = "refs/pigeon/purge-seals/";

/// Admitted immutable seal values, keyed by (decision identity, signer).
pub type Proofs = BTreeMap<(String, String), Seal<String, String>>;

fn invalid(detail: &str) -> Error {
    Error::Parse {
        argv: vec!["signed purge seals".into()],
        detail: detail.into(),
    }
}

/// Current and earlier locally retained signed reports. Existing proof
/// roots are checked again after restart, against today's active identity
/// and custodian membership, before they can authorize a drop.
pub async fn collect(g: &Group, active: &BTreeMap<String, Active>) -> Result<Proofs, Error> {
    if active.is_empty() {
        return Ok(BTreeMap::new());
    }
    let roster = g
        .roster()
        .ok_or_else(|| invalid("no accepted roster"))?
        .roster;
    let listed = g
        .git
        .run(&[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            PREFIX,
            "refs/pigeon/status/",
            "refs/pigeon/reports/",
        ])
        .await?;
    let refs: BTreeMap<String, String> = listed
        .lines()
        .map(|line| {
            line.split_once(' ')
                .map(|(name, oid)| (name.into(), oid.into()))
                .ok_or_else(|| invalid("unreadable seal reference"))
        })
        .collect::<Result<_, _>>()?;
    let mut proofs = BTreeMap::new();
    for (name, oid) in &refs {
        let raw = g.git.object(oid).await?;
        let Ok(signer) = pigeon_core::sig::verify(&raw) else {
            continue;
        };
        if !roster
            .devices
            .get(&signer)
            .is_some_and(|d| d.role == Role::Custodian)
        {
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
            || report.checking
            || report.custody.as_ref().is_none_or(|c| c.repository != "ok")
        {
            continue;
        }
        let offer = pigeon_core::report::Offer {
            signer: Some(&signer),
            device: &signer,
            known: true,
            seen: pigeon_core::report::Seen {
                at: pigeon_core::time::Time(report.at_unix().unwrap_or(i64::MAX)),
                commit: oid.clone(),
            },
        };
        if pigeon_core::report::judge(
            &roster,
            &offer,
            None,
            pigeon_core::time::Time(crate::util::now()),
        )
        .is_err()
        {
            continue;
        }
        for seal in report
            .purge
            .iter()
            .filter(|s| active.contains_key(&s.decision))
        {
            if active[&seal.decision]
                .doc
                .end()
                .is_none_or(|end| report.at_unix().is_none_or(|at| at < end))
            {
                continue;
            }
            if !crate::purge_file::commit_id(&seal.main) || !g.lock().engine.accepted(&seal.main) {
                continue;
            }
            let then = crate::purge_state::active(g, &seal.main).await?;
            if !then.contains_key(&seal.decision) {
                continue;
            }
            let key = (seal.decision.clone(), signer.0.clone());
            let value = Seal {
                main: seal.main.clone(),
                held: seal.held.iter().cloned().collect(),
            };
            if let Some(old) = proofs.get(&key) {
                if old != &value {
                    return Err(invalid("a custodian contradicted its immutable purge seal"));
                }
                continue;
            }
            let saved = format!("{PREFIX}{}/{}", seal.decision, signer.0);
            if name != &saved {
                // Replacing an invalid/torn old witness still needs a CAS.
                // Valid earlier witnesses were processed first and remain.
                g.git
                    .update_ref(&saved, oid, refs.get(&saved).map(String::as_str))
                    .await?;
            }
            proofs.insert(key, value);
        }
    }
    Ok(proofs)
}

/// The own seals retained from signed reports, for recovery after a lost
/// receipt file and for publication alongside freshly persisted seals.
pub fn own(proofs: &Proofs, me: &str) -> Vec<Purge> {
    proofs
        .iter()
        .filter(|((_, signer), _)| signer == me)
        .map(|((id, _), seal)| Purge {
            decision: id.clone(),
            main: seal.main.clone(),
            held: seal.held.iter().cloned().collect(),
        })
        .collect()
}

/// Membership uses immutable device identities; a name or old UUID cannot
/// make a removed custodian count in a new roster.
pub fn custodians(g: &Group) -> std::collections::BTreeSet<String> {
    g.roster()
        .map(|d| {
            d.roster
                .devices
                .into_iter()
                .filter_map(|(DeviceId(id), device)| (device.role == Role::Custodian).then_some(id))
                .collect()
        })
        .unwrap_or_default()
}
