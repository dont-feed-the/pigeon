//! Real-device collection (ADR 0025): no export without a roster choice,
//! authenticated peer data reaches only its chosen collector's explicit
//! local OTLP backend, and turning it off stops further peer export.
//! Backup peers on macOS are lab simulations; normal builds retain the
//! Linux-only backup role policy (ADR 0032).
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]
mod common;

use common::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

struct Backend {
    address: String,
    records: Arc<Mutex<Vec<serde_json::Value>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Backend {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let records = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (r, s) = (records.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buf = [0; 8192];
                let mut header = None;
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                    if header.is_none() {
                        header = bytes.windows(4).position(|b| b == b"\r\n\r\n").map(|at| {
                            let size = String::from_utf8_lossy(&bytes[..at])
                                .lines()
                                .find_map(|l| {
                                    let (k, v) = l.split_once(':')?;
                                    k.eq_ignore_ascii_case("content-length")
                                        .then(|| v.trim().parse::<usize>().ok())
                                        .flatten()
                                })
                                .unwrap();
                            (at + 4, size)
                        });
                    }
                    if let Some((start, len)) = header
                        && bytes.len() >= start + len
                    {
                        r.lock()
                            .unwrap()
                            .push(serde_json::from_slice(&bytes[start..start + len]).unwrap());
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
                        break;
                    }
                }
            }
        });
        Self {
            address,
            records,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn metric<'a>(records: &'a [serde_json::Value], name: &str) -> Vec<&'a serde_json::Value> {
    records
        .iter()
        .flat_map(|r| r["resourceMetrics"].as_array().into_iter().flatten())
        .flat_map(|r| r["scopeMetrics"].as_array().into_iter().flatten())
        .flat_map(|r| r["metrics"].as_array().into_iter().flatten())
        .filter(|m| m["name"] == name)
        .collect()
}

fn attribute(point: &serde_json::Value, key: &str, value: &str) -> bool {
    point["attributes"].as_array().is_some_and(|attrs| {
        attrs
            .iter()
            .any(|a| a["key"] == key && a["value"]["stringValue"] == value)
    })
}

#[test]
fn real_acceptance_protection_paths_and_work_batches_reach_the_backend() {
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let backend = Backend::start();
    let alice = start(&root, sockets.path(), "alice");
    let backup = start(&root, sockets.path(), "backup");
    let folder = root.join("shared");
    assert!(
        said(
            &pigeon(
                &alice,
                &[
                    "create",
                    "operator-metrics",
                    folder.to_str().unwrap(),
                    "--k",
                    "1"
                ]
            ),
            "created",
            "group"
        )
        .is_some()
    );
    join(&alice, &backup, &[&alice, &backup]);
    let config = alice.home.join("config/config.toml");
    let mut doc = std::fs::read_to_string(&config)
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    doc["telemetry"]["endpoint"] = toml_edit::value(&backend.address);
    std::fs::write(&config, doc.to_string()).unwrap();
    let own_id = pigeon(&alice, &["machines"])
        .into_iter()
        .find_map(|r| match r {
            pigeon_proto::Response::Machines { machines, .. } => machines
                .into_iter()
                .find(|m| m.this_device)
                .map(|m| m.device),
            _ => None,
        })
        .unwrap();
    let chosen = pigeon(&alice, &["settings", "telemetry-device", &own_id]);
    assert!(
        said(&chosen, "setting-changed", "value").is_some(),
        "{chosen:?}"
    );
    drop(backup);
    std::fs::create_dir_all(folder.join("Media")).unwrap();
    let file = folder.join("Media/exact.bin");
    let bytes = b"operator proof\0with exact bytes\r\n";
    std::fs::write(&file, bytes).unwrap();
    until(&alice, pigeon_proto::api::State::Local, &[&alice]);
    // This proves the real sampler observed an unprotected state before
    // reconnect, rather than manufacturing a protection measurement at boot.
    eventually_within(
        100,
        || {
            metric(&backend.records.lock().unwrap(), "pigeon.versions")
                .iter()
                .any(|m| {
                    m["gauge"]["dataPoints"].as_array().is_some_and(|points| {
                        points.iter().any(|p| {
                            (attribute(p, "state", "local") || attribute(p, "state", "published"))
                                && p["asDouble"].as_f64().is_some_and(|n| n > 0.0)
                        })
                    })
                })
        },
        "a real unprotected inventory measurement",
        &[&alice],
    );
    let backup = start(&root, sockets.path(), "backup");
    eventually_within(
        100,
        || status(&alice).state == pigeon_proto::api::State::Protected,
        "the exact saved version protected after reconnect",
        &[&alice, &backup],
    );
    let _ = pigeon(&alice, &["free", folder.join("Media").to_str().unwrap()]);
    eventually(
        || {
            // Thin/install also move the path aside briefly. Only the
            // skip-worktree bit distinguishes an indexed Free from that window.
            !file.exists()
                && std::process::Command::new("git")
                    .current_dir(&folder)
                    .args(["ls-files", "-v", "--", "Media/exact.bin"])
                    .output()
                    .is_ok_and(|out| out.status.success() && out.stdout.starts_with(b"S "))
        },
        "actual indexed free before requested keep",
        &[&alice, &backup],
    );
    let _ = pigeon(&alice, &["keep", folder.join("Media").to_str().unwrap()]);
    eventually(
        || std::fs::read(&file).ok().as_deref() == Some(bytes.as_slice()),
        "exact kept bytes",
        &[&alice, &backup],
    );
    let last_flags = Mutex::new([false; 4]);
    eventually_within(
        120,
        || {
            let records = backend.records.lock().unwrap();
            let samples = |name, observation| {
                metric(&records, name)
                    .iter()
                    .flat_map(|m| {
                        m["histogram"]["dataPoints"]
                            .as_array()
                            .into_iter()
                            .flatten()
                    })
                    .filter(|p| attribute(p, "observation", observation))
                    .filter_map(|p| p["timeUnixNano"].as_str())
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
            };
            // Two genuine SDK exports allow rate-based dashboard queries;
            // a single snapshot proves delivery, not a five-minute rate.
            let protected = samples("pigeon.protect.lag", "sampled-transition") >= 2;
            let peer_acceptance = samples("pigeon.sync.lag", "peer") >= 2;
            let direct = metric(&records, "pigeon.connection.observations")
                .iter()
                .any(|m| {
                    m["sum"]["dataPoints"]
                        .as_array()
                        .is_some_and(|p| p.iter().any(|p| attribute(p, "path", "direct")))
                });
            let queue_classes: std::collections::BTreeSet<_> =
                metric(&records, "pigeon.queue.depth")
                    .iter()
                    .flat_map(|m| m["gauge"]["dataPoints"].as_array().into_iter().flatten())
                    .flat_map(|p| p["attributes"].as_array().into_iter().flatten())
                    .filter(|a| a["key"] == "class")
                    .filter_map(|a| a["value"]["stringValue"].as_str())
                    .collect();
            let queues = ["capture", "history", "protect", "download"]
                .iter()
                .all(|class| queue_classes.contains(class));
            let flags = [protected, peer_acceptance, direct, queues];
            let mut previous = last_flags.lock().unwrap();
            if *previous != flags {
                let _ = writeln!(
                    std::io::stderr(),
                    "operator metric conditions [protection, acceptance, direct, queues]: {flags:?}; queue classes: {queue_classes:?}"
                );
                *previous = flags;
            }
            flags.into_iter().all(|ready| ready)
        },
        "lag, selected direct paths and all implemented work classes",
        &[&alice, &backup],
    );
    assert_eq!(std::fs::read(file).unwrap(), bytes);
    if let Some(path) = std::env::var_os("PIGEON_METRICS_EVIDENCE") {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        serde_json::to_writer(&mut file, &*backend.records.lock().unwrap()).unwrap();
    }
}

#[test]
fn peer_diagnostics_require_the_signed_collector_choice() {
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let backend = Backend::start();
    let alice = start(&root, sockets.path(), "alice");
    let backup = start(&root, sockets.path(), "backup");
    let all = [&alice, &backup];
    let folder = root.join("shared");
    let result = pigeon(
        &alice,
        &[
            "create",
            "telemetry-test",
            folder.to_str().unwrap(),
            "--k",
            "1",
        ],
    );
    assert!(said(&result, "created", "group").is_some(), "{result:?}");
    join(&alice, &backup, &all);
    let alice_id = pigeon(&alice, &["machines"])
        .into_iter()
        .find_map(|r| match r {
            pigeon_proto::Response::Machines { machines, .. } => machines
                .into_iter()
                .find(|m| m.this_device)
                .map(|m| m.device),
            _ => None,
        })
        .unwrap();
    let config = backup.home.join("config/config.toml");
    let mut doc = std::fs::read_to_string(&config)
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    doc["telemetry"]["endpoint"] = toml_edit::value(&backend.address);
    std::fs::write(&config, doc.to_string()).unwrap();
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(32) {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        backend.records.lock().unwrap().is_empty(),
        "a local endpoint must not opt the group into sharing"
    );
    let result = pigeon(&alice, &["settings", "telemetry-device", "backup"]);
    assert!(
        said(&result, "setting-changed", "value").is_some(),
        "{result:?}"
    );
    // Collection can take one interval to learn the roster and one to send.
    eventually_within(
        90,
        || {
            backend.records.lock().unwrap().iter().any(|r| {
                r["resourceSpans"].as_array().is_some_and(|resources| {
                    resources.iter().any(|r| {
                        r["resource"]["attributes"].as_array().is_some_and(|attrs| {
                            attrs.iter().any(|a| {
                                a["key"] == "pigeon.device"
                                    && a["value"]["stringValue"]
                                        .as_str()
                                        .is_some_and(|id| id == alice_id)
                            })
                        })
                    })
                })
            })
        },
        "peer diagnostics at the chosen backend",
        &all,
    );
    let result = pigeon(&alice, &["settings", "telemetry-device", "off"]);
    assert!(
        said(&result, "setting-changed", "value").is_some(),
        "{result:?}"
    );
    eventually(
        || {
            pigeon(&backup, &["settings", "telemetry-device"]).iter().any(|r|
        matches!(r, pigeon_proto::Response::Settings { settings } if settings.iter().any(|s| s.value == "off"))
    )
        },
        "collector removal accepted",
        &all,
    );
    std::thread::sleep(Duration::from_secs(2));
    let count = backend.records.lock().unwrap().len();
    std::thread::sleep(Duration::from_secs(32));
    assert_eq!(
        backend.records.lock().unwrap().len(),
        count,
        "export continued after the collector was removed"
    );
}
