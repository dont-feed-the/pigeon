//! Diagnostic contracts: a stalled sink never blocks emitters, shutdown
//! drains queued JSON, filters apply, and only seven matching files remain.
//! Dated fixtures check selection; no real elapsed retention is claimed.
#![allow(clippy::unwrap_used, reason = "isolated logging contract fixtures")]

use super::*;
use std::io;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Capture {
    fn records(&self) -> Vec<serde_json::Value> {
        std::str::from_utf8(&self.0.lock().unwrap())
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }
}

#[test]
fn queued_json_keeps_top_level_fields_and_target_filters_and_drains_on_shutdown() {
    let capture = Capture::default();
    let (writer, guard) = writer(capture.clone());
    let subscriber = tracing_subscriber::registry()
        .with(JsonLog(writer).with_filter(filter("info,logger_cases=debug")));
    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!(target: "logger_cases", count = 7u64, ready = true, "kept-debug");
        tracing::debug!(target: "ignored_cases", "not-kept");
        tracing::info!(target: "ordinary", machine = "fixture", "kept-info");
    });
    drop(guard);
    let records = capture.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["message"], "kept-debug");
    assert_eq!(records[0]["count"], 7);
    assert_eq!(records[0]["ready"], true);
    assert_eq!(records[0]["target"], "logger_cases");
    assert_eq!(records[0]["level"], "DEBUG");
    assert!(
        records[0]["time"]
            .as_str()
            .is_some_and(|s| s.ends_with('Z'))
    );
    assert_eq!(records[1]["machine"], "fixture");
}

struct Stalled {
    entered: Option<mpsc::Sender<()>>,
    release: mpsc::Receiver<()>,
}
impl Write for Stalled {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(entered) = self.entered.take() {
            let _ = entered.send(());
            let _ = self.release.recv();
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_stalled_worker_drops_excess_diagnostics_without_blocking_the_emitter() {
    let (entered, ready) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let (mut writer, guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(2)
        .lossy(true)
        .finish(Stalled {
            entered: Some(entered),
            release: wait,
        });
    writer.write_all(b"first\n").unwrap();
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    // The worker is now actually blocked in its sink. All writes below
    // must complete before we let it run, including overflow of its queue.
    for _ in 0..10 {
        writer.write_all(b"later\n").unwrap();
    }
    assert!(writer.error_counter().dropped_lines() > 0);
    release.send(()).unwrap();
    drop(guard);
}

#[test]
fn an_oversized_event_is_still_valid_bounded_json() {
    let capture = Capture::default();
    let (writer, guard) = writer(capture.clone());
    let subscriber = tracing_subscriber::registry().with(JsonLog(writer));
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(payload = "x".repeat(MAX_LINE * 2), "large");
    });
    drop(guard);
    assert!(capture.0.lock().unwrap().len() <= MAX_LINE);
    let records = capture.records();
    assert_eq!(records[0]["message"], "log-event-too-large");
    assert!(records[0]["original_bytes"].as_u64().unwrap() > MAX_LINE as u64);
}

#[test]
fn retention_preserves_the_active_file_after_a_clock_correction_and_ignores_other_names() {
    let temp = tempfile::tempdir().unwrap();
    let active = "pigeond-1999-01-01.ndjson";
    std::fs::write(temp.path().join(active), b"active").unwrap();
    for day in 1..=9 {
        std::fs::write(
            temp.path().join(format!("pigeond-2001-01-{day:02}.ndjson")),
            b"old fixture",
        )
        .unwrap();
    }
    let unrelated = temp.path().join("notes.txt");
    std::fs::write(&unrelated, b"keep").unwrap();
    for name in [
        "pigeond-2001-13-01.ndjson",
        "pigeond-2001-02-30.ndjson",
        "pigeond-2001-01-01.ndjson.bak",
    ] {
        std::fs::write(temp.path().join(name), b"keep unrelated").unwrap();
    }
    std::fs::create_dir(temp.path().join("pigeond-2002-01-01.ndjson")).unwrap();
    std::os::unix::fs::symlink(&unrelated, temp.path().join("pigeond-2003-01-01.ndjson")).unwrap();
    files::prune(temp.path(), active).unwrap();
    assert_eq!(std::fs::read(temp.path().join(active)).unwrap(), b"active");
    for day in 1..=9 {
        assert_eq!(
            temp.path()
                .join(format!("pigeond-2001-01-{day:02}.ndjson"))
                .exists(),
            day >= 4
        );
    }
    assert_eq!(std::fs::read(unrelated).unwrap(), b"keep");
    for name in [
        "pigeond-2001-13-01.ndjson",
        "pigeond-2001-02-30.ndjson",
        "pigeond-2001-01-01.ndjson.bak",
    ] {
        assert_eq!(
            std::fs::read(temp.path().join(name)).unwrap(),
            b"keep unrelated"
        );
    }
    assert!(temp.path().join("pigeond-2002-01-01.ndjson").is_dir());
    assert!(temp.path().join("pigeond-2003-01-01.ndjson").is_symlink());
}

#[test]
fn the_worker_keeps_existing_daily_names_and_private_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let (mut writer, guard) = writer(files::Daily::new(temp.path().to_owned()));
    writer.write_all(b"{\"message\":\"fixture\"}\n").unwrap();
    drop(guard);
    let entries = std::fs::read_dir(temp.path())
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1);
    let path = entries[0].path();
    assert!(
        path.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("pigeond-")
    );
    assert!(path.extension().is_some_and(|s| s == "ndjson"));
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "{\"message\":\"fixture\"}\n"
    );
}
