//! Status reports (06 §6.3): each device publishes `status.toml` in a
//! parentless commit it signs at `refs/pigeon/status/<device>`, when a
//! field other than a byte count changes and at least every 10 minutes.
//! Accept fetches each peer's `refs/pigeon/status/*` to
//! `refs/pigeon/reports/<peer>/*`; a report counts as pigeon-core's
//! `report::judge` says (signed by the roster device it names, known
//! format and fields, later than the last; from a release build, unless
//! this is a lab build), is then kept, raises when its device was heard
//! of, and is relayed at `refs/pigeon/status/<device>`.
//!
//! What the reports say reaches `status` and `doctor`: a peer that is full
//! (`disk-full`), short of room (`no-capacity`) or has lost its repository
//! (`repository-lost`), and the `main` each custodian has.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

use pigeon_core::filter::Role;
use pigeon_core::report::{Offer, Seen, judge};
use pigeon_core::roster::DeviceId;
use pigeon_core::time::Time;

use crate::group::Group;
use crate::report_file::{self, Custody, Disk, FORMAT, Queue, Read, Status, Versions};

/// The longest a device waits between two publications.
pub const PERIOD: i64 = 600;
/// The report's file in its commit.
const FILE: &str = "status.toml";

/// Where a peer's reports are fetched: `refs/pigeon/reports/<peer>/`.
pub fn refspec(peer: &str) -> String {
    format!("+refs/pigeon/status/*:refs/pigeon/reports/{peer}/*")
}

static VERSIONS: OnceLock<Versions> = OnceLock::new();

async fn versions(g: &Group) -> Versions {
    if let Some(v) = VERSIONS.get() {
        return v.clone();
    }
    let git = g.git.run(&["--version"]).await.unwrap_or_default();
    let annex = g
        .git
        .run(&["annex", "version", "--raw"])
        .await
        .unwrap_or_default();
    let v = Versions {
        pigeon: env!("CARGO_PKG_VERSION").to_owned(),
        git_annex: annex.lines().next().unwrap_or_default().trim().to_owned(),
        git: pigeon_git::version::parse_git(&git)
            .map(|(a, b, c)| format!("{a}.{b}.{c}"))
            .unwrap_or_default(),
    };
    if !v.git.is_empty() && !v.git_annex.is_empty() {
        let _ = VERSIONS.set(v.clone());
    }
    v
}

/// The bytes the annex objects under `git_dir` take, by their keys' sizes.
fn used(git_dir: &Path) -> u64 {
    let level = |p: &Path| -> Vec<std::path::PathBuf> {
        std::fs::read_dir(p)
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default()
    };
    let mut total = 0;
    for a in level(&git_dir.join("annex").join("objects")) {
        for b in level(&a) {
            for k in level(&b) {
                let size = k
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(pigeon_core::capacity::key_size);
                total += size.unwrap_or(0);
            }
        }
    }
    total
}

/// This device's report now.
pub async fn build(g: &Group, me: &str) -> Status {
    let now = crate::util::now();
    let doc = g.roster();
    let mine = doc
        .as_ref()
        .and_then(|d| d.roster.devices.get(&DeviceId(me.to_owned())).cloned());
    let space = pigeon_sys::fs::space(&g.git_dir).ok();
    let reserve = crate::settings::reserve_bytes(g.role, &g.git_dir);
    let (shortfall, queued) = {
        let k = g.lock();
        (k.shortfall.clone(), k.queue_bytes)
    };
    let custody = match g.role {
        Role::Laptop => None,
        Role::Custodian => {
            // Lost: not the UUID the roster names for this device. One that
            // is joining is not in its roster copy yet: that is no loss.
            let named = mine.as_ref().and_then(|d| d.annex_uuid.clone());
            let ok = named.is_none() || g.git.config("annex.uuid").await == named;
            Some(Custody {
                repository: if ok { "ok" } else { "lost" }.to_owned(),
            })
        }
    };
    let admits = match shortfall.as_ref().map(|s| s.admits) {
        Some(pigeon_core::capacity::Admits::Endangered) => "endangered",
        _ => "all",
    };
    let purge = g.lock().purge_seals.clone();
    let scrub = g.lock().scrub_report.clone();
    let acknowledgements = g.lock().admission_acknowledgements.clone();
    Status {
        format: FORMAT,
        device: me.to_owned(),
        at: report_file::datetime(now),
        main: g.git.rev("refs/heads/main").await,
        proposals: crate::proposal::kept(&g.git)
            .await
            .into_iter()
            .map(|(_, c)| c)
            .collect(),
        checking: crate::membership::pending(g, me) || g.lock().admission_error.is_some(),
        lab: pigeon_sys::lab::LAB,
        versions: versions(g).await,
        disk: Disk {
            used: used(&g.git_dir),
            free: space.as_ref().map_or(0, |s| s.available),
            maxsize: mine.and_then(|d| d.maxsize).unwrap_or(0),
            reserve,
            full: space.is_some_and(|s| s.available < reserve),
        },
        custody,
        scrub,
        purge,
        acknowledgements,
        queue: Queue {
            bytes_to_send: if g.role == Role::Laptop { queued } else { 0 },
            bytes_to_receive: if g.role == Role::Custodian { queued } else { 0 },
            paused: String::new(),
            admits: admits.to_owned(),
            deferred: shortfall.as_ref().map_or(0, |s| s.keys as u64),
            missing: shortfall.as_ref().map_or(0, |s| s.missing_bytes),
        },
    }
}

/// What of a report decides whether to publish it: all but its time and
/// its byte counts.
fn gist(s: &Status) -> String {
    let mut s = s.clone();
    s.at = report_file::datetime(0);
    (s.disk.used, s.disk.free) = (0, 0);
    (s.queue.bytes_to_send, s.queue.bytes_to_receive) = (0, 0);
    report_file::render(&s).unwrap_or_default()
}

/// Publishes this device's report if it changed or is due; answers whether
/// it did.
pub async fn publish(g: &Group, me: &str) -> bool {
    let status = build(g, me).await;
    let now = crate::util::now();
    let gist = gist(&status);
    let due = g
        .lock()
        .published
        .as_ref()
        .is_none_or(|(last, at)| *last != gist || now - at >= PERIOD);
    if !due {
        return false;
    }
    match write(g, me, &status).await {
        Ok(commit) => {
            tracing::debug!(group = %g.id, commit = %commit, "report-published");
            g.lock().published = Some((gist, now));
            true
        }
        Err(e) => {
            tracing::warn!(group = %g.id, error = %e, "report-failed");
            false
        }
    }
}

async fn write(g: &Group, me: &str, status: &Status) -> Result<String, crate::git::Error> {
    let fence = g.lock().admission_fence.clone();
    if let Some(fence) = fence
        && (status.main.as_ref().is_none()
            || !g
                .git
                .contains(status.main.as_deref().unwrap_or_default(), &fence)
                .await)
    {
        g.git.purge_gate.suspend();
        return Err(crate::membership::invalid(
            "report main is behind the durable admission fence",
        ));
    }
    let text = report_file::render(status).map_err(|e| crate::git::Error::Parse {
        argv: vec!["status.toml".into()],
        detail: e.to_string(),
    })?;
    let blob = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await?;
    let line = format!("100644 blob {}\t{FILE}\n", blob.trim());
    let tree = g.git.run_in(&["mktree"], line.as_bytes()).await?;
    let commit = g.git.commit_tree(tree.trim(), &[], "status").await?;
    crate::admission_proofs::retain_ack(g, status, &commit).await?;
    let name = format!("refs/pigeon/status/{me}");
    g.git.run(&["update-ref", &name, &commit]).await?;
    Ok(commit)
}

/// Reads the reports fetched and relayed here, keeps those that count,
/// and relays them; answers how many counted.
pub async fn receive(g: &Group, me: &str) -> usize {
    let Some(doc) = g.roster() else {
        return 0;
    };
    let args = [
        "for-each-ref",
        "--format=%(objectname) %(refname)",
        "refs/pigeon/reports/",
        "refs/pigeon/status/",
    ];
    let listed = g.git.run(&args).await.unwrap_or_default();
    let now = crate::util::now();
    let mut counted = 0;
    for line in listed.lines() {
        let Some((oid, name)) = line.split_once(' ') else {
            continue;
        };
        let Some(device) = name.rsplit('/').next().filter(|d| *d != me) else {
            continue;
        };
        let id = DeviceId(device.to_owned());
        let last = g.lock().reports.get(&id).cloned();
        if last.as_ref().is_some_and(|s| s.commit == oid) {
            continue;
        }
        let Ok(raw) = g.git.object(oid).await else {
            continue;
        };
        let signer = pigeon_core::sig::verify(&raw).ok();
        let text = g
            .git
            .blob(&format!("{oid}:{FILE}"))
            .await
            .unwrap_or_default();
        let read = report_file::parse(&String::from_utf8_lossy(&text));
        let (status, known) = match read {
            Read::Known(s) if s.device == device && (!s.lab || pigeon_sys::lab::LAB) => {
                (Some(*s), true)
            }
            _ => (None, false),
        };
        let at = status.as_ref().and_then(Status::at_unix).unwrap_or(0);
        let offer = Offer {
            signer: signer.as_ref(),
            device: &id,
            known,
            seen: Seen {
                at: Time(at),
                commit: oid.to_owned(),
            },
        };
        if let Err(why) = judge(&doc.roster, &offer, last.as_ref(), Time(now)) {
            tracing::debug!(group = %g.id, report = %name, why = ?why, "report-ignored");
            continue;
        }
        let Some(status) = status else {
            continue;
        };
        if let Err(error) = crate::admission_proofs::retain_ack(g, &status, oid).await {
            tracing::warn!(group = %g.id, %error, "acknowledgement-retention-waits");
            continue;
        }
        {
            let mut k = g.lock();
            k.reports.offer(id.clone(), offer.seen.clone());
            k.report_files.insert(device.to_owned(), status);
            let heard = k.heard.entry(device.to_owned()).or_insert(at);
            *heard = (*heard).max(at.min(now));
        }
        let relay = format!("refs/pigeon/status/{device}");
        if name != relay
            && let Err(e) = g.git.run(&["update-ref", &relay, oid]).await
        {
            tracing::warn!(group = %g.id, report = %name, error = %e, "report-relay-failed");
        }
        counted += 1;
    }
    counted
}

/// The latest reports of the devices of the roster, by device ID.
fn current(g: &Group) -> Vec<(String, Status)> {
    let devices = g.roster().map(|d| d.roster.devices).unwrap_or_default();
    g.lock()
        .report_files
        .iter()
        .filter(|(d, _)| devices.contains_key(&DeviceId((*d).clone())))
        .map(|(d, s)| (d.clone(), s.clone()))
        .collect()
}

/// The devices of the roster whose latest report says their repository is
/// lost.
pub fn lost(g: &Group) -> BTreeSet<String> {
    current(g)
        .into_iter()
        .filter(|(_, s)| s.custody.as_ref().is_some_and(|c| c.repository == "lost"))
        .map(|(d, _)| d)
        .collect()
}

/// What the latest reports of the other devices of the roster say needs a
/// person: a problem code, the device, and its arguments (files, missing
/// bytes).
pub fn problems(g: &Group) -> Vec<(&'static str, String, u64, u64)> {
    let mut out = Vec::new();
    for (d, s) in &current(g) {
        if s.disk.full {
            out.push(("disk-full", d.clone(), 0, 0));
        } else if s.queue.deferred > 0 {
            out.push(("no-capacity", d.clone(), s.queue.deferred, s.queue.missing));
        }
        if s.custody.as_ref().is_some_and(|c| c.repository == "lost") {
            out.push(("repository-lost", d.clone(), 0, 0));
        }
    }
    out
}
