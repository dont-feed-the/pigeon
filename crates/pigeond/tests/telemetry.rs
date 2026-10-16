//! Real-device collection (ADR 0025): no export without a roster choice,
//! authenticated peer data reaches only its chosen collector's explicit
//! local OTLP backend, and turning it off stops further peer export.
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
            pigeon_proto::Response::Machines { machines } => machines
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
