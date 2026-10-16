//! `pigeond`'s logs (12 §12.1, ADR 0022): one JSON object per line, in
//! `<state>/logs/pigeond-YYYY-MM-DD.ndjson` (UTC), with the time, the level,
//! the target, the event's name and its fields. A minimal `tracing`
//! layer writes them; a second layer records spans through the shared
//! OpenTelemetry pipeline (ADR 0025).

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{Layer, layer::Context, prelude::*};

struct JsonLog {
    dir: PathBuf,
    file: Mutex<Option<(String, std::fs::File)>>,
}

struct Fields(serde_json::Map<String, serde_json::Value>);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.insert(
            field.name().to_owned(),
            serde_json::Value::String(format!("{value:?}")),
        );
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(
            field.name().to_owned(),
            serde_json::Value::String(value.to_owned()),
        );
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

impl JsonLog {
    fn write(&self, line: &[u8]) {
        let now = crate::util::now();
        let day = crate::util::date(now);
        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        if file.as_ref().is_none_or(|(d, _)| *d != day) {
            let _ = std::fs::create_dir_all(&self.dir);
            let path = self.dir.join(format!("pigeond-{day}.ndjson"));
            *file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()
                .map(|f| (day, f));
        }
        if let Some((_, f)) = file.as_mut() {
            let _ = f.write_all(line);
        }
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
        line.push(b'\n');
        self.write(&line);
    }
}

/// Sends `tracing` events to the daily files in `dir`; `PIGEON_LOG=debug`
/// adds debug events.
pub fn init(dir: PathBuf) {
    let max = match std::env::var("PIGEON_LOG").as_deref() {
        Ok("debug") => Level::DEBUG,
        Ok("trace") => Level::TRACE,
        _ => Level::INFO,
    };
    let log = JsonLog {
        dir,
        file: Mutex::new(None),
    };
    let registry = tracing_subscriber::registry();
    #[cfg(feature = "tokio-console")]
    let registry = registry.with(pigeon_telemetry::console_layer());
    let _ =
        tracing::subscriber::set_global_default(
            registry
                .with(
                    log.with_filter(tracing_subscriber::filter::filter_fn(move |m| {
                        *m.level() <= max
                    })),
                )
                .with(pigeon_telemetry::layer().with_filter(
                    tracing_subscriber::filter::filter_fn(move |m| {
                        *m.level() <= max
                            && !m.target().starts_with("tokio")
                            && !m.target().starts_with("runtime")
                    }),
                )),
        );
}
