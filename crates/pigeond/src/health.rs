//! User group health from accepted state (12 §12.2, ADR 0025). Counts
//! share status's snapshot. Capacity is a timestamped signed report;
//! telemetry and stale reports cannot create a protection claim.

use crate::{daemon::Daemon, group::Group, report_file};
use pigeon_core::{
    roster::{DeviceId, Role},
    time::Time,
};
use pigeon_proto::{
    Response,
    api::Machine,
    health::{self as wire, HealthReport},
};
use std::collections::{BTreeMap, BTreeSet};

fn counts(c: pigeon_core::health::Counts) -> wire::Counts {
    wire::Counts {
        missing: c.missing,
        erased: c.erased,
        local: c.local,
        published: c.published,
        uncertain: c.uncertain,
        protected: c.protected,
    }
}

pub(crate) async fn own_report(g: &Group, me: &str) -> Option<report_file::Status> {
    let name = format!("refs/pigeon/status/{me}");
    let commit = g.git.rev(&name).await?;
    let raw = g.git.object(&commit).await.ok()?;
    if pigeon_core::sig::verify(&raw).ok()?.0 != me {
        return None;
    }
    let text = g.git.blob(&format!("{commit}:status.toml")).await?;
    match report_file::parse(&String::from_utf8_lossy(&text)) {
        report_file::Read::Known(status)
            if status.device == me && (!status.lab || pigeon_sys::lab::LAB) =>
        {
            Some(*status)
        }
        _ => None,
    }
}

fn epochs(g: &Group, removed: BTreeSet<String>) -> Option<Vec<wire::RepairEpoch>> {
    if removed.is_empty() {
        return Some(Vec::new());
    }
    let mut known = g.lock();
    if let Some((previous, epochs)) = &known.health_repair_epochs
        && *previous == removed
    {
        return Some(epochs.clone());
    }
    let (Some(history), Some(tip)) = (&known.history, &known.roster_commit) else {
        return None;
    };
    if tip != &history.genesis && !history.commits.contains_key(tip) {
        return None;
    }
    let epochs: Vec<_> = pigeon_core::health::repair_epochs(history, tip)
        .into_iter()
        .filter(|epoch| removed.contains(&epoch.device.0))
        .map(|epoch| wire::RepairEpoch {
            commit: epoch.commit,
            device: epoch.device.0,
            name: epoch.name,
            started_at: crate::util::rfc3339(epoch.at),
        })
        .collect();
    known.health_repair_epochs = Some((removed, epochs.clone()));
    Some(epochs)
}

pub(crate) async fn report(d: &Daemon) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![crate::api::problem("not-in-group", &[], None)];
    };
    let Some(doc) = g.roster() else {
        return vec![crate::api::problem("not-in-group", &[], None)];
    };
    let (mut status, inventory) = crate::status::inventory_report(&g, &d.device, "", false).await;
    let omitted_paths = status
        .versions
        .len()
        .saturating_sub(200)
        .try_into()
        .unwrap_or(u64::MAX);
    status.versions.truncate(200);
    if let Some(pause) = crate::pause::now(d)
        && status.level != pigeon_proto::api::Level::NeedsYou
    {
        status.headline = crate::pause::headline(pause);
    }
    // Reading a count must not race the mutable proposal judge. An
    // unavailable count is explicit while another operation holds busy.
    let suggestions = if let Ok(_busy) = g.busy.try_lock() {
        crate::decide::pending(&g)
            .await
            .ok()
            .map(|p| p.len().try_into().unwrap_or(u64::MAX))
    } else {
        None
    };
    let now = crate::util::now();
    let (heard, reports) = {
        let known = g.lock();
        (known.heard.clone(), known.report_files.clone())
    };
    let mut reports: BTreeMap<_, _> = reports
        .into_iter()
        .filter(|(id, _)| doc.roster.devices.contains_key(&DeviceId(id.clone())))
        .collect();
    if let Some(ours) = own_report(&g, &d.device).await {
        reports.insert(d.device.clone(), ours);
    }
    let stale_after = crate::status::settings(&g).stale_after;
    let machines = doc
        .roster
        .devices
        .iter()
        .map(|(id, device)| {
            let report = (device.role != Role::Recovery)
                .then(|| reports.get(&id.0))
                .flatten();
            let at = report.and_then(|r| crate::util::parse_rfc3339(&r.at.to_string()));
            wire::MachineHealth {
                machine: Machine {
                    name: device.name.clone(),
                    device: id.0.clone(),
                    custodian: device.role == Role::Custodian,
                    recovery: device.role == Role::Recovery,
                    heard_at: (device.role != Role::Recovery)
                        .then(|| heard.get(&id.0).map(|t| crate::util::rfc3339(*t)))
                        .flatten(),
                    this_device: id.0 == d.device,
                },
                reported_at: at.map(crate::util::rfc3339),
                report_fresh: at.is_some_and(|at| {
                    at <= now.saturating_add(pigeon_core::report::SKEW.0)
                        && Time(at).within(stale_after, Time(now))
                }),
                disk: report.map(|r| wire::Disk {
                    used: r.disk.used,
                    free: r.disk.free,
                    maxsize: r.disk.maxsize,
                    reserve: r.disk.reserve,
                    full: r.disk.full,
                }),
                checking: report.is_some_and(|r| r.checking),
                repository_lost: report
                    .is_some_and(|r| r.custody.as_ref().is_some_and(|c| c.repository == "lost")),
                queue: report.map(|r| wire::Queue {
                    send_bytes: r.queue.bytes_to_send,
                    receive_bytes: r.queue.bytes_to_receive,
                    deferred_keys: r.queue.deferred,
                    missing_bytes: r.queue.missing,
                }),
            }
        })
        .collect();
    let removed = doc.roster.removed.keys().map(|id| id.0.clone()).collect();
    vec![Response::Health(Box::new(HealthReport {
        group_id: g.id.clone(),
        group: doc.roster.group,
        at: crate::util::rfc3339(now),
        status,
        omitted_paths,
        current: inventory.current.map(counts),
        retained: inventory.retained.map(counts),
        suggestions,
        endangered: crate::admission::endangered(&g)
            .len()
            .try_into()
            .unwrap_or(u64::MAX),
        machines,
        repair_epochs: epochs(&g, removed),
    }))]
}
