//! Purge driver (08 §8.6): scan immutable history outside `busy`, then
//! evaluate an exact fresh ref reading under it. Incomplete scans, unknown
//! decisions and unsupported membership epochs pause content; none can
//! authorize a drop. Signed reports preserve durably checkpointed seals.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::filter::{ContentPolicy, Role};
use pigeon_core::purge::{Reference, View};

use crate::group::Group;
use crate::purge_index::Index;
use crate::purge_receipts::Ledger;

/// Incremental metadata, private to one live group. Never a released bit.
#[derive(Debug, Default)]
pub struct State {
    index: Index,
    archives: Option<crate::purge_archive::Archives>,
    ledger: Option<Ledger>,
    observed: BTreeSet<String>,
}

fn invalid(detail: &str) -> crate::git::Error {
    crate::git::Error::Parse {
        argv: vec!["purge".into()],
        detail: detail.into(),
    }
}

/// Read immutable reference metadata for a preview without holding the
/// worktree lock. The caller must bind and recheck its exact ref snapshot.
pub(crate) async fn reference_index(
    g: &Group,
    tips: &[String],
) -> Result<Index, crate::git::Error> {
    let mut index = g.purge.lock().await.index.clone();
    index.extend(&g.git, tips).await?;
    Ok(index)
}

/// Cold scans never hold the group's worktree/request lock. A changed ref
/// will invalidate the reading again before it can authorize content.
pub async fn prepare(g: &Group) {
    if !g.git.purge_gate.armed() {
        return;
    }
    let result = async {
        let reading = crate::retention::snapshot(g).await?;
        let mut state = g.purge.lock().await;
        if state.archives.is_none() {
            state.archives = Some(crate::purge_archive::load(g).await?);
        }
        state.index.extend(&g.git, &reading.tips).await
    }
    .await;
    if let Err(e) = result {
        g.git.purge_gate.suspend();
        tracing::warn!(group = %g.id, error = %e, "purge-index-waits");
    }
}

/// Evaluate after metadata acceptance and before placement. Caller holds
/// `busy`. A stale or unsupported reading keeps metadata available while
/// content and physical erasure wait.
pub async fn round(g: &Group, me: &str, allow_drop: bool) {
    if !g.git.purge_gate.armed() {
        return;
    }
    if let Err(e) = evaluate(g, me, allow_drop).await {
        g.git.purge_gate.suspend();
        tracing::warn!(group = %g.id, error = %e, "purge-waits");
    }
}

async fn evaluate(g: &Group, me: &str, allow_drop: bool) -> Result<(), crate::git::Error> {
    let revision = g
        .git
        .purge_gate
        .revision()
        .ok_or_else(|| invalid("a ref writer is active"))?;
    let reading = crate::retention::snapshot(g).await?;
    let main = reading
        .main
        .as_ref()
        .ok_or_else(|| invalid("no accepted main"))?;
    if g.lock().admission_error.is_some() {
        return Err(invalid(
            "custody admission is pending or its durable fence is unavailable",
        ));
    }
    let fence = g.lock().admission_fence.clone();
    if let Some(fence) = fence
        && !g.git.contains(main, &fence).await
    {
        return Err(invalid("purge main is behind its durable admission fence"));
    }
    let mut state = g.purge.lock().await;
    let tips: BTreeSet<_> = reading.tips.iter().cloned().collect();
    if state.index.tips != tips {
        return Err(invalid("the complete reference scan is pending"));
    }
    let active = crate::purge_state::active(g, main).await?;
    g.reload_roster().await;
    let membership = crate::membership::read(g, main).await?;
    g.lock().membership = Some((main.clone(), membership.clone()));
    if crate::membership::pending(g, me) {
        return Err(invalid("this backup has not completed custody admission"));
    }
    let mut certificates = crate::purge_epoch::collect(g, main).await?;
    let custodians = membership.eligible;
    for (id, decision) in &active {
        if certificates.contains_key(id) {
            continue;
        }
        if !membership.pending.is_empty() {
            return Err(invalid(
                "cancel or close old erasures before admitting a pending backup",
            ));
        }
        let bytes = g
            .git
            .blob(&format!("{}:.pigeon/roster.toml", decision.introduced))
            .await
            .ok_or_else(|| invalid("missing decision roster"))?;
        let doc = crate::roster_file::parse(&String::from_utf8_lossy(&bytes))
            .map_err(|_| invalid("unknown decision roster"))?;
        let original: BTreeSet<String> = doc
            .roster
            .devices
            .into_iter()
            .filter_map(|(id, d)| (d.role == pigeon_core::roster::Role::Custodian).then_some(id.0))
            .collect();
        if custodians != original {
            // Static-roster proofs do not justify treating a new custodian
            // as sealed, shrinking an unfinished quorum, or fetching
            // previously released old references after a roster change.
            return Err(invalid("purge membership epoch needs a completion proof"));
        }
    }
    let archives = state
        .archives
        .as_ref()
        .ok_or_else(|| invalid("archive verification is pending"))?;
    if !archives.anchored(&reading.named) {
        return Err(invalid(
            "a signed erasure archive was removed or substituted",
        ));
    }
    certificates.extend(archives.epochs.clone());
    let epochs: BTreeMap<_, _> = certificates
        .iter()
        .map(|(id, verified)| (id.clone(), verified.epoch.clone()))
        .collect();
    let mut all_active = active.clone();
    all_active.extend(archives.active.clone());
    let mut proofs = crate::purge_proofs::collect(g, &active).await?;
    for certificate in certificates.values() {
        for (key, seal) in &certificate.seals {
            if proofs
                .insert(key.clone(), seal.clone())
                .is_some_and(|old| old != *seal)
            {
                return Err(invalid("completion contradicts an immutable seal"));
            }
        }
    }
    let mut reachable = BTreeSet::new();
    let mut ancestry = BTreeMap::new();
    for tip in tips
        .iter()
        .chain(all_active.values().map(|a| &a.doc.base))
        .chain(all_active.values().flat_map(|a| a.doc.proposals.iter()))
        .chain(proofs.values().map(|seal| &seal.main))
        .chain(epochs.values().map(|epoch| &epoch.main))
    {
        if let Some(ancestors) = state.index.ancestors(tip) {
            if tips.contains(tip) {
                reachable.extend(ancestors.iter().cloned());
            }
            ancestry.insert(tip.clone(), ancestors);
        }
    }
    // Signed archive links establish observation across authority domains,
    // without pretending that a new genesis has Git parents. Its current
    // files therefore keep their genuine fresh births in the index.
    let mut observed = ancestry
        .get(main)
        .cloned()
        .ok_or_else(|| invalid("missing current ancestry"))?;
    for tip in archives.anchors.values() {
        observed.extend(
            state
                .index
                .ancestors(tip)
                .ok_or_else(|| invalid("incomplete signed archive"))?,
        );
    }
    ancestry.insert(main.clone(), observed);
    let references: BTreeSet<Reference<String, String>> = state
        .index
        .references
        .iter()
        .filter(|r| reachable.contains(&r.birth))
        .cloned()
        .collect();
    let current: BTreeSet<String> = g.git.keys(main).await?.into_values().collect();
    let decisions: BTreeMap<_, _> = all_active
        .iter()
        .map(|(id, a)| (id.clone(), a.doc.decision()))
        .collect();
    let contains = |tip: &String, ancestor: &String| {
        ancestry.get(tip).is_some_and(|set| set.contains(ancestor))
    };
    let view = View {
        main,
        references: &references,
        current: &current,
        decisions: &decisions,
        custodians: &custodians,
        seals: &proofs,
        epochs: &epochs,
    };
    let archive_roots = archives.anchors.clone();
    let mut own: BTreeMap<String, crate::report_file::Purge> =
        crate::purge_proofs::own(&proofs, me)
            .into_iter()
            .map(|seal| (seal.decision.clone(), seal))
            .collect();
    if g.role == Role::Custodian && custodians.contains(me) {
        let named = g.roster().and_then(|doc| {
            doc.roster
                .devices
                .get(&pigeon_core::roster::DeviceId(me.into()))
                .and_then(|device| device.annex_uuid.clone())
        });
        if named.is_none() || g.git.config("annex.uuid").await != named {
            return Err(invalid("custodian repository identity is not intact"));
        }
        let path = g.root().join("purges.toml");
        let fresh = state.ledger.is_none();
        if fresh {
            state.ledger = Some(Ledger::load(&path).map_err(|e| invalid(&e))?);
        }
        let ids: BTreeSet<String> = active.keys().cloned().collect();
        let changed = fresh || ids != state.observed;
        state.observed = ids.clone();
        let ledger = state
            .ledger
            .as_mut()
            .ok_or_else(|| invalid("missing ledger"))?;
        ledger.observe(&ids);
        ledger.save(&path, changed).map_err(|e| invalid(&e))?;
        for (id, a) in &active {
            if !epochs.contains_key(id)
                && !own.contains_key(id)
                && pigeon_core::purge::can_seal(
                    crate::util::now(),
                    a.doc.end().ok_or_else(|| invalid("invalid deadline"))?,
                    ledger.elapsed(id),
                    a.retention_seconds,
                )
            {
                let seal = crate::report_file::Purge {
                    decision: id.clone(),
                    main: main.clone(),
                    held: view.held(&decisions[id], contains).into_iter().collect(),
                };
                ledger.seal(seal).map_err(|e| invalid(&e))?;
                ledger.save(&path, true).map_err(|e| invalid(&e))?;
            }
        }
        for seal in ledger.seals() {
            if let Some(previous) = own.insert(seal.decision.clone(), seal.clone())
                && previous != seal
            {
                return Err(invalid("own retained seal contradicts receiver ledger"));
            }
        }
    }
    // Completion witnesses already retain the original full signed
    // reports. Bound replaceable reports to still-pending decisions.
    g.lock().purge_seals = own
        .into_values()
        .filter(|seal| active.contains_key(&seal.decision) && !epochs.contains_key(&seal.decision))
        .collect();
    let renewed =
        crate::purge_missing::renewed(g, &state.index, &view, &all_active, &archive_roots).await;
    let released = view.released(contains);
    let after = crate::retention::snapshot(g).await?;
    if after.named != reading.named
        || !g.git.purge_gate.publish(
            revision,
            ContentPolicy {
                waiting: false,
                released: released.clone(),
            },
        )
    {
        return Err(invalid("retained refs changed during purge evaluation"));
    }
    g.lock().renewed_after_erasure = Some((main.clone(), renewed));
    if allow_drop {
        let held_hashes: BTreeSet<_> = references
            .iter()
            .filter(|r| !released.contains(&r.key))
            .filter_map(|r| crate::net::sync::key_sha256(&r.key))
            .map(str::to_ascii_lowercase)
            .collect();
        for key in released {
            let _lease = g.git.purge_gate.erasure().await;
            let policy = g.git.purge_gate.policy();
            if policy.waiting || !policy.released.contains(&key) {
                return Err(invalid("ref mutation interrupted purge drops"));
            }
            if crate::custody::present(g, &key).await.is_some() {
                // --key never names a working path. Current and uncovered
                // references were excluded by the pure algebra above.
                g.git
                    .run(&["annex", "drop", "--force", "--key", &key])
                    .await?;
            }
            if crate::net::sync::key_sha256(&key)
                .is_some_and(|hash| !held_hashes.contains(&hash.to_ascii_lowercase()))
            {
                crate::shared::release(g, &key)
                    .await
                    .map_err(|e| invalid(&e))?;
            }
            tracing::info!(group = %g.id, key, "purge-dropped");
        }
    }
    Ok(())
}
