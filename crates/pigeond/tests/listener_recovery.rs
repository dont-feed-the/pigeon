//! An actual listener helper may exit while its annex parent stays alive.
//! Restore incoming metadata reachability without restarting pigeon or
//! weakening admission, capture, custody or deadline assertions.
#![cfg(feature = "lab")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "disposable child-process recovery fixture"
)]

mod common;
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Mutex;

use common::{ask, dirs, eventually_within, join, pigeon, said, start, until};
use pigeon_proto::{Request, Response};

fn listener_children(root: u32) -> Vec<(u32, u32)> {
    let output = Command::new("ps")
        .args(["-ww", "-axo", "pid=,ppid=,args="])
        .output()
        .unwrap();
    assert!(output.status.success());
    let rows: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse::<u32>().ok()?;
            let parent = fields.next()?.parse::<u32>().ok()?;
            let args: Vec<_> = fields.map(str::to_owned).collect();
            Some((pid, parent, args))
        })
        .collect();
    let parents: BTreeMap<_, _> = rows
        .iter()
        .map(|(pid, parent, _)| (*pid, *parent))
        .collect();
    rows.iter()
        .filter_map(|(pid, parent, args)| {
            if args.len() != 3
                || !args[0].ends_with("git-annex-p2p-iroh")
                || !args[2].contains("annex/p2p/")
            {
                return None;
            }
            let direct_parent = *parent;
            let mut parent = direct_parent;
            for _ in 0..30 {
                if parent == root {
                    return Some((*pid, direct_parent));
                }
                parent = *parents.get(&parent)?;
            }
            None
        })
        .collect()
}

#[test]
fn a_lost_listener_is_replaced_while_pigeon_keeps_running() {
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let a = start(&root, sockets.path(), "a1");
    let c = start(&root, sockets.path(), "c1");
    let all = [&a, &c];
    let folder = root.join("files");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("seed.txt"), "seed\n").unwrap();
    let created = pigeon(
        &a,
        &["create", "Family", folder.to_str().unwrap(), "--k", "1"],
    );
    assert!(said(&created, "created", "group").is_some());
    join(&a, &c, &all);
    until(&a, pigeon_proto::api::State::Protected, &all);
    let peer = pigeon(&c, &["machines"])
        .into_iter()
        .find_map(|r| match r {
            Response::Machines { machines, .. } => machines
                .into_iter()
                .find(|m| m.this_device)
                .map(|m| m.device),
            _ => None,
        })
        .unwrap();
    let connect = || {
        ask(
            &a,
            &Request::Connect {
                repository: folder.to_string_lossy().into_owned(),
                device: peer.clone(),
            },
        )
        .iter()
        .any(|r| matches!(r, Response::Connected))
    };
    assert!(
        connect(),
        "initial admitted metadata path must be reachable"
    );
    let helper = Mutex::new(None);
    eventually_within(
        20,
        || {
            let found = listener_children(c.process_id());
            if found.len() != 1 {
                return false;
            }
            *helper.lock().unwrap() = Some(found[0]);
            true
        },
        "one actual backup listener helper",
        &all,
    );
    let (helper, native_parent) = helper.into_inner().unwrap().unwrap();
    assert!(
        Command::new("kill")
            .args(["-TERM", &helper.to_string()])
            .status()
            .unwrap()
            .success()
    );
    let parent_alive = Command::new("kill")
        .args(["-0", &native_parent.to_string()])
        .status()
        .unwrap()
        .success();
    std::fs::write(
        root.join("listener-loss.json"),
        serde_json::to_vec(&serde_json::json!({
            "pigeon": c.process_id(), "terminated_helper": helper, "native_parent": native_parent,
            "native_parent_alive_after_helper_termination": parent_alive,
        }))
        .unwrap(),
    )
    .unwrap();
    eventually_within(
        30,
        || {
            listener_children(c.process_id())
                .iter()
                .any(|(pid, _)| *pid != helper)
                && connect()
        },
        "lost listener never recovered after helper termination",
        &all,
    );
    // The fixture retains the same pigeon process and requires its actual
    // native helper plus admission path, rather than an invented healthy bit.
    assert!(
        Command::new("kill")
            .args(["-0", &c.process_id().to_string()])
            .status()
            .unwrap()
            .success()
    );
}
