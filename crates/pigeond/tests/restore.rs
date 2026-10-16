//! `pigeon restore PATH --at TIME` (10 §10.1): Ana saves a file twice; a
//! time between the saves brings the first version back, and a time before
//! the first is refused with `restore-at`. `pigeon deleted FOLDER` lists
//! only the files deleted under the folder.
//!
//! Needs what `tests/slice.rs` needs: git-annex, and `pigeon` and
//! `git-annex-p2p-iroh` built next to `pigeond`.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::{SERIAL, dirs, eventually_within, pigeon, said, start};
use pigeon_proto::Response;

fn versions(d: &common::Daemon, path: &str) -> usize {
    pigeon(d, &["history", path])
        .iter()
        .map(|r| match r {
            Response::History { versions } => versions.len(),
            _ => 0,
        })
        .sum()
}

#[test]
fn a_time_between_two_saves_restores_the_first() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let a1 = start(&root, sockets.path(), "a1");
    let fa = root.join("ana");
    std::fs::create_dir_all(&fa).unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    let file = fa.join("a.txt");
    let p = file.to_str().unwrap();
    std::fs::write(&file, "one\n").unwrap();
    eventually_within(120, || versions(&a1, p) == 1, "no first version", &[&a1]);
    std::thread::sleep(std::time::Duration::from_secs(2));
    let between = pigeon_proto::time::rfc3339(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .try_into()
            .unwrap(),
    );
    std::thread::sleep(std::time::Duration::from_secs(2));
    std::fs::write(&file, "two\n").unwrap();
    eventually_within(120, || versions(&a1, p) == 2, "no second version", &[&a1]);

    let early = pigeon(&a1, &["restore", p, "--at", "1970-01-01"]);
    assert!(
        early
            .iter()
            .any(|r| matches!(r, Response::Problem(x) if x.code == "restore-at")),
        "{early:?}"
    );
    let back = pigeon(&a1, &["restore", p, "--at", &between]);
    assert!(said(&back, "restore-result", "path").is_some(), "{back:?}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "one\n");

    // `deleted FOLDER` lists only what was deleted under it.
    std::fs::create_dir_all(fa.join("sub")).unwrap();
    std::fs::write(fa.join("sub/b.txt"), "bee\n").unwrap();
    std::fs::write(fa.join("c.txt"), "sea\n").unwrap();
    let b = fa.join("sub/b.txt");
    let c = fa.join("c.txt");
    eventually_within(
        120,
        || versions(&a1, b.to_str().unwrap()) == 1 && versions(&a1, c.to_str().unwrap()) == 1,
        "b.txt and c.txt never saved",
        &[&a1],
    );
    std::fs::remove_file(&b).unwrap();
    std::fs::remove_file(&c).unwrap();
    let deleted = |at: &std::path::Path| -> Vec<String> {
        pigeon(&a1, &["deleted", at.to_str().unwrap()])
            .into_iter()
            .flat_map(|r| match r {
                Response::Deleted { files, .. } => files.into_iter().map(|f| f.path).collect(),
                _ => Vec::new(),
            })
            .collect()
    };
    eventually_within(
        120,
        || deleted(&fa).len() == 2,
        "the deletions never reached main",
        &[&a1],
    );
    assert_eq!(deleted(&fa.join("sub")), ["sub/b.txt"]);

    // The SDK exports asynchronously. Observe a completed batch rather
    // than assuming that reading the just-updated catalogue flushed spans.
    eventually_within(
        20,
        || {
            let lines = pigeon_telemetry::tail(&a1.home.join("state/telemetry"), 1000);
            let mut spans = Vec::new();
            for line in &lines {
                let Ok(batch) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                for resource in batch["resourceSpans"].as_array().into_iter().flatten() {
                    for scope in resource["scopeSpans"].as_array().into_iter().flatten() {
                        spans.extend(scope["spans"].as_array().into_iter().flatten().cloned());
                    }
                }
            }
            spans.iter().any(|s| {
                s["name"] == "capture"
                    && spans.iter().any(|r| {
                        r["name"] == "round"
                            && r["traceId"] == s["traceId"]
                            && r["spanId"] == s["parentSpanId"]
                    })
            })
        },
        "capture must be a child of a completed round in the exported SDK spans",
        &[&a1],
    );
}
