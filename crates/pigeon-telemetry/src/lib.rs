//! Telemetry (12 §12.2, ADR 0025): spans and metrics written as OTLP-JSON
//! lines, one `ExportTraceServiceRequest` or `ExportMetricsServiceRequest`
//! per line, as OpenTelemetry's file exporter writes them, into
//! `<state>/telemetry/`. Files rotate at [`FILE_BYTES`]; the oldest go once
//! the directory holds more than [`LIMIT_BYTES`]. Nothing leaves the device:
//! `pigeon debug bundle` collects them, and any OTLP back end reads them.
//!
//! The lines are written by hand rather than by `opentelemetry-otlp`, which
//! a collector of the group will need **[no ADR: hand-written local
//! export]**. Without [`init`], spans and metrics cost nothing.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// The most the directory holds (12 §12.2).
pub const LIMIT_BYTES: u64 = 50 * 1024 * 1024;
/// When a file is closed and the next begun.
pub const FILE_BYTES: u64 = 5 * 1024 * 1024;

struct Sink {
    dir: PathBuf,
    resource: Value,
    file: Mutex<Option<(PathBuf, std::fs::File, u64)>>,
}

static SINK: OnceLock<Sink> = OnceLock::new();

/// Starts writing into `dir`, as `service` on `device`; later calls change
/// nothing.
pub fn init(dir: PathBuf, service: &str, device: &str) {
    let resource = json!({"attributes": [
        attr("service.name", &Value::from(service)),
        attr("pigeon.device", &Value::from(device)),
        attr("service.version", &Value::from(env!("CARGO_PKG_VERSION"))),
    ]});
    let _ = SINK.set(Sink {
        dir,
        resource,
        file: Mutex::new(None),
    });
}

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    if getrandom::fill(&mut b).is_err() {
        // Unique enough for telemetry when the system has no randomness.
        let t = now_ns().to_le_bytes();
        for (i, x) in b.iter_mut().enumerate() {
            *x = t.get(i % t.len()).copied().unwrap_or(0) ^ (i as u8);
        }
    }
    b
}

/// An OTLP attribute: `key` with a string, integer, float or boolean.
fn attr(key: &str, value: &Value) -> Value {
    let v = match value {
        Value::Bool(b) => json!({"boolValue": b}),
        Value::Number(n) if n.is_i64() || n.is_u64() => json!({"intValue": n.to_string()}),
        Value::Number(n) => json!({"doubleValue": n.as_f64()}),
        Value::String(s) => json!({"stringValue": s}),
        other => json!({"stringValue": other.to_string()}),
    };
    json!({"key": key, "value": v})
}

fn attrs(list: &[(String, Value)]) -> Vec<Value> {
    list.iter().map(|(k, v)| attr(k, v)).collect()
}

/// The files of `files` (name, bytes; names sort oldest first) to delete so
/// that the rest hold at most `limit` bytes; the newest always stays.
pub fn prune(files: &[(String, u64)], limit: u64) -> Vec<String> {
    let mut sorted: Vec<&(String, u64)> = files.iter().collect();
    sorted.sort();
    let mut total: u64 = sorted.iter().map(|(_, n)| n).sum();
    let mut out = Vec::new();
    for (name, n) in sorted.iter().take(sorted.len().saturating_sub(1)) {
        if total <= limit {
            break;
        }
        total -= n;
        out.push(name.clone());
    }
    out
}

fn listed(dir: &Path) -> Vec<(String, u64)> {
    std::fs::read_dir(dir)
        .map(|it| {
            it.filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
                .map(|e| {
                    let n = e.metadata().map_or(0, |m| m.len());
                    (e.file_name().to_string_lossy().into_owned(), n)
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Sink {
    fn write(&self, line: &Value) {
        let mut bytes = serde_json::to_vec(line).unwrap_or_default();
        bytes.push(b'\n');
        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        if file.as_ref().is_none_or(|(_, _, n)| *n >= FILE_BYTES) {
            let _ = std::fs::create_dir_all(&self.dir);
            let name = format!("pigeond-{:020}.jsonl", now_ns() / 1_000);
            let path = self.dir.join(name);
            *file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok()
                .map(|f| (path, f, 0));
            for old in prune(&listed(&self.dir), LIMIT_BYTES - FILE_BYTES) {
                let _ = std::fs::remove_file(self.dir.join(old));
            }
        }
        if let Some((_, f, n)) = file.as_mut()
            && f.write_all(&bytes).is_ok()
        {
            *n += bytes.len() as u64;
        }
    }

    fn scope() -> Value {
        json!({"name": "pigeon", "version": env!("CARGO_PKG_VERSION")})
    }
}

/// A span, written when it is dropped.
pub struct Span {
    trace: [u8; 16],
    id: [u8; 8],
    parent: Option<[u8; 8]>,
    name: String,
    start: u128,
    attributes: Vec<(String, Value)>,
    error: Option<String>,
}

/// A new root span: a trace of its own.
pub fn span(name: &str) -> Span {
    Span {
        trace: if SINK.get().is_some() {
            random()
        } else {
            [0; 16]
        },
        id: if SINK.get().is_some() {
            random()
        } else {
            [0; 8]
        },
        parent: None,
        name: name.to_owned(),
        start: now_ns(),
        attributes: Vec::new(),
        error: None,
    }
}

impl Span {
    /// A span inside this one, in its trace.
    pub fn child(&self, name: &str) -> Span {
        let mut s = span(name);
        s.parent = Some(self.id);
        s.trace = self.trace;
        s
    }

    /// Sets an attribute, such as `group` or `device` (natural IDs).
    pub fn set(&mut self, key: &str, value: impl Into<Value>) -> &mut Self {
        self.attributes.push((key.to_owned(), value.into()));
        self
    }

    /// Marks the span failed, with why.
    pub fn fail(&mut self, why: &str) {
        self.error = Some(why.to_owned());
    }

    /// The OTLP-JSON line of this span ended at `end`.
    fn line(&self, resource: &Value, end: u128) -> Value {
        let mut s = json!({
            "traceId": hex(&self.trace),
            "spanId": hex(&self.id),
            "name": self.name,
            "kind": 1,
            "startTimeUnixNano": self.start.to_string(),
            "endTimeUnixNano": end.to_string(),
            "attributes": attrs(&self.attributes),
            "status": match &self.error {
                Some(m) => json!({"code": 2, "message": m}),
                None => json!({"code": 1}),
            },
        });
        if let (Some(p), Some(o)) = (self.parent, s.as_object_mut()) {
            o.insert("parentSpanId".into(), hex(&p).into());
        }
        json!({"resourceSpans": [{
            "resource": resource,
            "scopeSpans": [{"scope": Sink::scope(), "spans": [s]}],
        }]})
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(sink) = SINK.get() {
            sink.write(&self.line(&sink.resource, now_ns()));
        }
    }
}

/// Writes a gauge's value now, such as `pigeon.versions` by `state`.
pub fn gauge(name: &str, value: f64, labels: &[(&str, &str)]) {
    let Some(sink) = SINK.get() else {
        return;
    };
    let labels: Vec<Value> = labels
        .iter()
        .map(|(k, v)| attr(k, &Value::from(*v)))
        .collect();
    let point = json!({
        "timeUnixNano": now_ns().to_string(),
        "asDouble": value,
        "attributes": labels,
    });
    sink.write(&json!({"resourceMetrics": [{
        "resource": sink.resource,
        "scopeMetrics": [{"scope": Sink::scope(), "metrics": [
            {"name": name, "gauge": {"dataPoints": [point]}}
        ]}],
    }]}));
}

/// The last `n` lines written into `dir`, oldest first, for a debug bundle.
pub fn tail(dir: &Path, n: usize) -> Vec<String> {
    let mut files = listed(dir);
    files.sort();
    let mut out: Vec<String> = Vec::new();
    for (name, _) in files.iter().rev() {
        let text = std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        lines.append(&mut out);
        out = lines;
        if out.len() >= n {
            break;
        }
    }
    let skip = out.len().saturating_sub(n);
    out.split_off(skip)
}

#[cfg(test)]
mod tests;
