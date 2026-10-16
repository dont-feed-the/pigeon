//! Signed rules-3 preparation for destructive-driver fixtures. This uses
//! the real request, report and witness readers, not a test bypass. The
//! known fixture keys are never used by a running production daemon.
#![allow(clippy::unwrap_used, reason = "signed protocol upgrade fixtures")]

use std::collections::BTreeMap;

use pigeon_core::authority::{Entry, ROSTER, Tree};
use pigeon_core::roster::{DeviceId, Role};
use pigeon_core::sig;
use pigeond::admission_file::{Request, Witness};
use pigeond::group::Group;
use pigeond::report_file::{AdmissionAck, AdmissionKind, Custody};

async fn put(g: &Group, text: &str) -> Entry {
    let oid = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await
        .unwrap();
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: oid.trim().into(),
    }
}

async fn signed(g: &Group, tree: &Tree, parents: &[&str], key: u8) -> String {
    let root = pigeond::merge::write_tree(&g.git, tree).await.unwrap();
    let mut raw = format!("tree {root}\n");
    for parent in parents {
        raw.push_str(&format!("parent {parent}\n"))
    }
    raw.push_str("author pigeon <pigeon@localhost> 1783252800 +0000\ncommitter pigeon <pigeon@localhost> 1783252800 +0000\n\npigeon: protocol fixture\n");
    let raw = sig::sign(raw.as_bytes(), &[key; 32]).unwrap();
    g.git
        .run_in(&["hash-object", "-t", "commit", "-w", "--stdin"], &raw)
        .await
        .unwrap()
        .trim()
        .into()
}

fn key(device: &DeviceId) -> u8 {
    (1..=10)
        .find(|n| sig::device_id(&[*n; 32]) == *device)
        .unwrap()
}

/// Prepare the exact current fixture group, retaining all signed reports.
/// Returns its accepted upgraded head, without changing working/index data.
pub async fn upgrade(g: &Group) -> String {
    let parent = g.git.rev("refs/heads/main").await.unwrap();
    pigeond::merge::judge_history(g, std::slice::from_ref(&parent))
        .await
        .unwrap();
    let mut roster = g.roster().unwrap();
    let signer = roster
        .roster
        .devices
        .iter()
        .find(|(_, d)| d.role != Role::Recovery && roster.roster.persons[&d.person].admin)
        .map(|(id, _)| key(id))
        .unwrap();
    let request = Request {
        format: 1,
        kind: AdmissionKind::Upgrade,
        base: parent.clone(),
        device: None,
    };
    let mut tree = g.git.tree(&parent).await.unwrap();
    tree.insert(
        request.path().unwrap(),
        put(g, &request.render().unwrap()).await,
    );
    let requested = signed(g, &tree, &[&parent], signer).await;
    pigeond::merge::judge_history(g, std::slice::from_ref(&requested))
        .await
        .unwrap();
    assert!(g.lock().engine.accepted(&requested));
    let mut reports = BTreeMap::new();
    for (id, device) in &roster.roster.devices {
        if device.role == Role::Recovery {
            continue;
        }
        let mut report = pigeond::report::build(g, &id.0).await;
        report.main = Some(requested.clone());
        report.checking = false;
        report.custody = (device.role == Role::Custodian).then(|| Custody {
            repository: "ok".into(),
        });
        report.acknowledgements = vec![AdmissionAck {
            kind: AdmissionKind::Upgrade,
            request: request.id().unwrap(),
            protocol: 2,
        }];
        let report_tree = [(
            "status.toml".into(),
            put(g, &pigeond::report_file::render(&report).unwrap()).await,
        )]
        .into();
        reports.insert(id.0.clone(), signed(g, &report_tree, &[], key(id)).await);
    }
    let witness = Witness {
        format: 1,
        request,
        main: requested.clone(),
        reports,
    };
    let (path, text) = witness.render().unwrap();
    tree.insert(path, put(g, &text).await);
    roster.roster.rules = pigeon_core::purge::RULES;
    tree.insert(
        ROSTER.into(),
        put(g, &pigeond::roster_file::render(&roster).unwrap()).await,
    );
    let upgraded = signed(g, &tree, &[&requested], signer).await;
    pigeond::merge::judge_history(g, std::slice::from_ref(&upgraded))
        .await
        .unwrap();
    assert!(g.lock().engine.accepted(&upgraded));
    g.git
        .update_ref("refs/heads/main", &upgraded, Some(&parent))
        .await
        .unwrap();
    g.reload_roster().await;
    let state = pigeond::membership::read(g, &upgraded).await.unwrap();
    g.lock().membership = Some((upgraded.clone(), state));
    upgraded
}
