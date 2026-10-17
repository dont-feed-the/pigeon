//! Diagnostics stay off the daemon's work (ADR 0022, 12 §12.1): bounded,
//! lossy tracing-appender queues feed human stderr and the existing daily
//! JSON files. Guards drain the queues at shutdown. Trace proofs use their
//! separate durable channel, not these diagnostic logs.

use std::io::Write;
use std::path::PathBuf;

use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_appender::non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::{EnvFilter, Layer, filter::LevelFilter, layer::Context, prelude::*};

mod files;
#[cfg(test)]
mod tests;

const QUEUED_LINES: usize = 128;
const MAX_LINE: usize = 16_384;

/// Owned for the daemon's lifetime so queued diagnostics drain on exit.
#[must_use]
pub struct Guards {
    _file: WorkerGuard,
    _stderr: WorkerGuard,
}

struct JsonLog(NonBlocking);
struct Fields(serde_json::Map<String, serde_json::Value>);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.insert(
            field.name().to_owned(),
            serde_json::Value::String(format!("{value:?}")),
        );
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_owned(), value.into());
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().to_owned(), value.into());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().to_owned(), value.into());
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().to_owned(), value.into());
    }
}

impl<S: Subscriber> Layer<S> for JsonLog {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let mut fields = Fields(serde_json::Map::new());
        event.record(&mut fields);
        let mut map = serde_json::Map::new();
        map.insert(
            "time".into(),
            crate::util::rfc3339(crate::util::now()).into(),
        );
        map.insert("level".into(), event.metadata().level().as_str().into());
        map.insert("target".into(), event.metadata().target().into());
        map.extend(fields.0);
        let mut line = serde_json::to_vec(&serde_json::Value::Object(map)).unwrap_or_default();
        if line.len() >= MAX_LINE {
            line = serde_json::to_vec(&serde_json::json!({
                "time": crate::util::rfc3339(crate::util::now()),
                "level": event.metadata().level().as_str(),
                "target": event.metadata().target(),
                "message": "log-event-too-large", "original_bytes": line.len()
            }))
            .unwrap_or_default();
        }
        line.push(b'\n');
        let _ = self.0.clone().write_all(&line);
    }
}

fn filter(text: &str) -> EnvFilter {
    EnvFilter::builder()
        .with_regex(false)
        .with_default_directive(LevelFilter::INFO.into())
        .parse_lossy(text)
}

fn writer(sink: impl Write + Send + 'static) -> (NonBlocking, WorkerGuard) {
    NonBlockingBuilder::default()
        .buffered_lines_limit(QUEUED_LINES)
        .lossy(true)
        .finish(sink)
}

/// Start human stderr and daily JSON logging. PIGEON_LOG accepts EnvFilter
/// directives and defaults to info. Keep the returned guards until shutdown.
pub fn init(dir: PathBuf) -> Guards {
    let filter = filter(&std::env::var("PIGEON_LOG").unwrap_or_default());
    let (file, file_guard) = writer(files::Daily::new(dir));
    let (stderr, stderr_guard) = writer(std::io::stderr());
    let registry = tracing_subscriber::registry();
    #[cfg(feature = "tokio-console")]
    let registry = registry.with(pigeon_telemetry::console_layer());
    let subscriber = registry
        .with(JsonLog(file).with_filter(filter.clone()))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(stderr)
                .with_filter(filter.clone()),
        )
        .with(pigeon_telemetry::layer().with_filter(filter).with_filter(
            tracing_subscriber::filter::filter_fn(|m| {
                !m.target().starts_with("tokio") && !m.target().starts_with("runtime")
            }),
        ));
    let _ = tracing::subscriber::set_global_default(subscriber);
    Guards {
        _file: file_guard,
        _stderr: stderr_guard,
    }
}
