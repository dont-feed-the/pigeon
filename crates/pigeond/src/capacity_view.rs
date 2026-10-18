//! Capacity accounting (08 §8.9): immutable ref snapshots, core distinct-key
//! accounting and signed reports. Missing disk evidence remains explicit.

use crate::{daemon::Daemon, group::Group};
use pigeon_core::capacity::{self, Part};
use pigeon_proto::{Response, capacity as wire};
use std::collections::{BTreeMap, BTreeSet};

fn error(message: impl ToString) -> crate::git::Error {
    crate::git::Error::Parse {
        argv: vec!["capacity".into()],
        detail: message.to_string(),
    }
}

fn size(key: &str) -> Result<u64, crate::git::Error> {
    capacity::key_size(key).ok_or_else(|| error("retained key has no readable size"))
}

struct Inventory {
    main: String,
    parts: BTreeMap<String, Part>,
    decisions: BTreeMap<String, String>,
    proposals: Vec<String>,
}

async fn inventory(g: &Group) -> Result<Inventory, crate::git::Error> {
    let snapshot = crate::retention::snapshot(g).await?;
    let main = snapshot
        .main
        .ok_or_else(|| error("accepted history unavailable"))?;
    let mut parts = BTreeMap::new();
    let current: BTreeSet<_> = g.git.keys(&main).await?.into_values().collect();
    for key in current {
        parts.insert(key, Part::Current);
    }
    let mut proposals = Vec::new();
    for (name, tip) in snapshot.named {
        let proposal = name.starts_with(crate::proposal::PREFIX) || name.contains("/proposals/");
        let part = if proposal {
            Part::Suggestions
        } else {
            Part::History
        };
        if name.starts_with(crate::proposal::PREFIX) {
            proposals.push(tip.clone());
        }
        for key in g.git.retained_keys(&[tip]).await? {
            parts
                .entry(key)
                .and_modify(|old| *old = (*old).min(part))
                .or_insert(part);
        }
    }
    let mut decisions = BTreeMap::new();
    for path in g.git.tree(&main).await?.keys().filter(|p| {
        p.starts_with(".pigeon/decisions/") && !p.starts_with(".pigeon/decisions/purge-")
    }) {
        let bytes = g
            .git
            .blob(&format!("{main}:{path}"))
            .await
            .ok_or_else(|| error("decision unavailable"))?;
        let doc = crate::decision_file::parse(&String::from_utf8_lossy(&bytes))
            .map_err(|_| error("decision unreadable"))?;
        decisions.insert(
            doc.proposal,
            match doc.verdict {
                pigeon_core::authority::Decided::Rejected => "rejected",
                pigeon_core::authority::Decided::Withdrawn => "withdrawn",
            }
            .to_owned(),
        );
    }
    Ok(Inventory {
        main,
        parts,
        decisions,
        proposals,
    })
}

pub(crate) async fn report(d: &Daemon) -> Vec<Response> {
    if d.default_group().is_none() {
        return vec![crate::api::problem("not-in-group", &[], None)];
    }
    match build(d).await {
        Ok(report) => vec![Response::Capacity(Box::new(report))],
        Err(error) => vec![crate::api::problem(
            "internal-error",
            &[],
            Some(error.to_string()),
        )],
    }
}

async fn build(d: &Daemon) -> Result<wire::Report, crate::git::Error> {
    let g = d.default_group().ok_or_else(|| error("no group"))?;
    let doc = g.roster().ok_or_else(|| error("roster unavailable"))?;
    let Inventory {
        main,
        parts,
        decisions,
        proposals: tips,
    } = inventory(&g).await?;
    let versions: Vec<_> = parts
        .iter()
        .map(|(key, part)| Ok((key.as_str(), size(key)?, *part)))
        .collect::<Result<_, crate::git::Error>>()?;
    // Refuse impossible metadata rather than overflowing the core's exact sums.
    let one_copy = versions
        .iter()
        .try_fold(0u64, |sum, (_, size, _)| sum.checked_add(*size))
        .ok_or_else(|| error("capacity size overflow"))?;
    one_copy
        .checked_mul(u64::from(doc.roster.k))
        .ok_or_else(|| error("capacity size overflow"))?;
    let need = capacity::need(doc.roster.k, versions);
    let custodians: Vec<_> = doc
        .roster
        .devices
        .values()
        .filter(|device| device.role == pigeon_core::roster::Role::Custodian)
        .collect();
    let offered: Vec<_> = custodians
        .iter()
        .map(|device| (device.maxsize, one_copy))
        .collect();
    let limited = capacity::below_one_copy(&offered)
        .into_iter()
        .filter_map(|i| custodians.get(i).map(|device| device.name.clone()))
        .collect();
    let no_spare = capacity::no_spare(custodians.len(), doc.roster.k);
    offered
        .iter()
        .try_fold(0u64, |sum, (max, bytes)| {
            sum.checked_add(max.map_or(*bytes, |m| m.min(*bytes)))
        })
        .ok_or_else(|| error("capacity size overflow"))?;
    let offered = capacity::capacity(&offered);
    let mut proposals = Vec::new();
    for tip in tips {
        let state = if g.git.contains(&main, &tip).await {
            "adopted".into()
        } else {
            decisions
                .get(&tip)
                .cloned()
                .unwrap_or_else(|| "open".into())
        };
        let mut bytes = 0u64;
        for key in g.git.retained_keys(std::slice::from_ref(&tip)).await? {
            if parts.get(&key) == Some(&Part::Suggestions) {
                bytes = bytes
                    .checked_add(size(&key)?)
                    .ok_or_else(|| error("suggestion size overflow"))?;
            }
        }
        proposals.push(wire::Proposal {
            saved_at: crate::util::rfc3339(
                g.git
                    .commit_time(&tip)
                    .await
                    .ok_or_else(|| error("suggestion time unavailable"))?,
            ),
            id: tip,
            state,
            additional_bytes: bytes,
        });
    }
    proposals.sort_by(|a, b| a.saved_at.cmp(&b.saved_at).then(a.id.cmp(&b.id)));
    let health = crate::health::report(d)
        .await
        .into_iter()
        .find_map(|r| {
            if let Response::Health(h) = r {
                Some(h)
            } else {
                None
            }
        })
        .ok_or_else(|| error("device reports unavailable"))?;
    let active: BTreeSet<_> = doc
        .roster
        .devices
        .values()
        .filter(|device| device.role == pigeon_core::roster::Role::Custodian)
        .filter_map(|device| device.annex_uuid.as_ref())
        .collect();
    let overflow = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        overflow(&g, &parts, &active, doc.roster.k),
    )
    .await
    .ok()
    .flatten();
    Ok(wire::Report {
        group: doc.roster.group,
        at: crate::util::rfc3339(crate::util::now()),
        k: doc.roster.k,
        current: *need.parts.get(&Part::Current).unwrap_or(&0),
        history: *need.parts.get(&Part::History).unwrap_or(&0),
        suggestions: *need.parts.get(&Part::Suggestions).unwrap_or(&0),
        overflow,
        configured: offered,
        limited,
        no_spare,
        machines: health.machines,
        proposals,
    })
}

// Command/Whereis.hs (git-annex 10.20261006) supports batch keys and
// reports missing content with a valid JSON row even when its exit is 1.
// Keep this optional observation bounded; it cannot affect custody.
async fn overflow(
    g: &Group,
    keys: &BTreeMap<String, Part>,
    active: &BTreeSet<&String>,
    k: u32,
) -> Option<u64> {
    let keys: Vec<_> = keys.keys().collect();
    let mut total = 0u64;
    for chunk in keys.chunks(256) {
        if chunk.iter().any(|key| key.contains(['\r', '\n'])) {
            return None;
        }
        let input = chunk
            .iter()
            .map(|key| key.as_str())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let (_, out) = g
            .git
            .location_claims(input.as_bytes(), std::time::Duration::from_secs(5))
            .await
            .ok()?;
        let rows: Vec<_> = out
            .stdout
            .split(|b| *b == b'\n')
            .filter(|b| !b.is_empty())
            .map(serde_json::from_slice::<serde_json::Value>)
            .collect::<Result<_, _>>()
            .ok()?;
        if rows.len() != chunk.len() {
            return None;
        }
        for (row, expected) in rows.iter().zip(chunk) {
            if row["key"].as_str() != Some(expected.as_str()) {
                return None;
            }
            let holders: BTreeSet<_> = ["whereis", "untrusted"]
                .iter()
                .flat_map(|field| row[field].as_array().into_iter().flatten())
                .filter_map(|holder| holder["uuid"].as_str())
                .filter(|uuid| active.iter().any(|active| active.as_str() == *uuid))
                .collect();
            let extra = holders.len().saturating_sub(k as usize);
            total = total.checked_add(size(expected).ok()?.checked_mul(extra.try_into().ok()?)?)?;
        }
    }
    Some(total)
}
