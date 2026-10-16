//! Diagnostic disclosure with explicit local, one-use consent (12 §12.2).
//! A permission names a group and authenticated peer, expires after ten
//! monotonic minutes and is lost on restart. Peer snapshots include only
//! that group's records; ungrouped and other-group data stay local.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Instant;

use pigeon_core::consent::Grant;
use pigeon_core::roster::{DeviceId, Role};
use pigeon_proto::Response;
use serde_json::Value;

use crate::daemon::Daemon;
use crate::group::Group;

type Scope = (String, String);

/// One-use permissions granted on this computer, never by another peer.
#[derive(Debug)]
pub struct Permissions {
    began: Instant,
    grants: Mutex<BTreeMap<Scope, Grant<Scope>>>,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            began: Instant::now(),
            grants: Mutex::new(BTreeMap::new()),
        }
    }
}

impl Permissions {
    fn allow(&self, group: &str, device: &str) -> bool {
        let now = self.began.elapsed().as_secs();
        let mut grants = self.grants.lock().unwrap_or_else(|p| p.into_inner());
        grants.retain(|_, grant| grant.live(now));
        let scope = (group.to_owned(), device.to_owned());
        if grants.len() >= 64 && !grants.contains_key(&scope) {
            return false;
        }
        grants.insert(scope.clone(), Grant::new(scope, now, 600));
        true
    }

    fn take(&self, group: &str, device: &str) -> bool {
        let now = self.began.elapsed().as_secs();
        let scope = (group.to_owned(), device.to_owned());
        let mut grants = self.grants.lock().unwrap_or_else(|p| p.into_inner());
        let allowed = grants.get_mut(&scope).is_some_and(|g| g.take(&scope, now));
        grants.retain(|_, grant| grant.live(now));
        allowed
    }
}

fn peer(g: &Group, name: &str, me: &str) -> Option<(String, String)> {
    let doc = g.roster()?;
    if doc.roster.devices.get(&DeviceId(me.to_owned()))?.role == Role::Recovery {
        return None;
    }
    doc.roster
        .devices
        .iter()
        .find(|(id, d)| id.0 != me && d.role != Role::Recovery && (id.0 == name || d.name == name))
        .map(|(id, d)| (id.0.clone(), d.name.clone()))
}

/// Explicitly authorizes one bundle on the recipient's own local socket.
pub fn allow(d: &Daemon, name: &str) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![crate::api::problem("not-in-group", &[], None)];
    };
    let Some((id, name)) = peer(&g, name, &d.device) else {
        return vec![crate::api::problem(
            "unknown-computer",
            &[("name", pigeon_proto::Arg::Text(name.to_owned()))],
            None,
        )];
    };
    if !d.debug.allow(&g.id, &id) {
        return vec![crate::api::problem("not-allowed", &[], None)];
    }
    vec![crate::api::said(
        "debug-allowed",
        &[("name", pigeon_proto::Arg::Text(name.to_owned()))],
    )]
}

fn member(g: &Group, device: &str) -> bool {
    g.roster().is_some_and(|doc| {
        doc.roster
            .devices
            .get(&DeviceId(device.to_owned()))
            .is_some_and(|d| d.role != Role::Recovery)
    })
}

/// Called only for a peer authenticated by QUIC. The current roster is
/// checked both before consuming consent and after building the snapshot.
pub async fn share(d: &Daemon, group: &str, device: &str) -> Option<Value> {
    let g = d.group(group)?;
    if !member(&g, &d.device) || !member(&g, device) || !d.debug.take(group, device) {
        return None;
    }
    pigeon_telemetry::flush();
    let now = crate::util::now();
    let log = d
        .dirs
        .logs()
        .join(format!("pigeond-{}.ndjson", crate::util::date(now)));
    let mut logs: Vec<Value> = std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event.get("group").and_then(Value::as_str) == Some(group))
        .take(300)
        .collect();
    let records: Vec<Value> = pigeon_telemetry::tail(&d.dirs.state.join("telemetry"), 500)
        .iter()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    // An active daemon can fill 500 OTLP batches in minutes. Bound each
    // section by encoded bytes so legitimate fresh consent never fails the
    // wire's 2 MiB cap merely because this computer has been running longer.
    logs.reverse();
    let (logs, logs_omitted) = bounded(logs, 256 * 1024);
    let (telemetry, telemetry_omitted) =
        bounded(pigeon_telemetry::only_group(&records, group), 512 * 1024);
    let roster = g
        .roster()
        .and_then(|doc| crate::roster_file::render(&doc).ok());
    let roster_omitted = roster.as_ref().is_some_and(|r| r.len() > 512 * 1024);
    let roster = roster.filter(|r| r.len() <= 512 * 1024);
    let value = serde_json::json!({
        "format": 1, "group": group, "device": d.device,
        "time": crate::util::rfc3339(now),
        "main": g.git.rev("refs/heads/main").await,
        "roster": roster, "log_tail": logs,
        "telemetry_tail": telemetry,
        "omitted": {"logs": logs_omitted, "telemetry": telemetry_omitted, "roster": roster_omitted},
    });
    (member(&g, &d.device) && member(&g, device)).then_some(value)
}

/// Collects chosen peer parts only after their users granted permission.
pub async fn collect(d: &Daemon, names: &[String]) -> Vec<Response> {
    if names.is_empty() {
        return crate::history::debug_bundle(d).await;
    }
    let Some(g) = d.default_group() else {
        return vec![crate::api::problem("not-in-group", &[], None)];
    };
    let mut parts = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for name in names {
        let Some((id, name)) = peer(&g, name, &d.device) else {
            return vec![crate::api::problem(
                "unknown-computer",
                &[("name", pigeon_proto::Arg::Text(name.clone()))],
                None,
            )];
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let addresses = g.lock().addresses.get(&id).cloned().unwrap_or_default();
        match d.node.diagnostics(&id, addresses, &g.id).await {
            Ok(value) => parts.push(value),
            Err(code) => {
                let args = if code == "debug-not-allowed" {
                    vec![("name", pigeon_proto::Arg::Text(name.to_owned()))]
                } else {
                    Vec::new()
                };
                return vec![crate::api::problem(&code, &args, None)];
            }
        }
    }
    let mut local = crate::history::snapshot(d).await;
    local["peer_parts"] = Value::Array(parts);
    crate::history::write_bundle(d, &local)
}

/// Retain newest complete records that fit, counting all omitted records.
fn bounded(records: Vec<Value>, budget: usize) -> (Vec<Value>, usize) {
    let total = records.len();
    let mut left = budget;
    let mut kept = Vec::new();
    for record in records.into_iter().rev() {
        if let Ok(bytes) = serde_json::to_vec(&record) {
            let size = bytes.len().saturating_add(1);
            if size <= left {
                left -= size;
                kept.push(record);
            }
        }
    }
    kept.reverse();
    let omitted = total - kept.len();
    (kept, omitted)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_tail_keeps_newest_complete_records_and_reports_omissions() {
        let records = vec![
            serde_json::json!({"old":"x".repeat(4096)}),
            serde_json::json!({"new":1}),
            serde_json::json!({"new":2}),
        ];
        let (kept, omitted) = super::bounded(records, 64);
        assert_eq!(
            kept,
            vec![serde_json::json!({"new":1}), serde_json::json!({"new":2})]
        );
        assert_eq!(omitted, 1);
        assert!(serde_json::to_vec(&kept).is_ok_and(|b| b.len() <= 64));
    }
}
