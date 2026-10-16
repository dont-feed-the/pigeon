//! One OpenTelemetry SDK pipeline for spans and metrics (ADR 0025,
//! 12 §12.2). Export is bounded, local OTLP-JSON by default. No environment
//! variable can silently enable an external exporter. The same SDK tracer
//! records explicit loop spans and the `tracing` bridge.

mod export;
mod file;
mod metrics;
mod route;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use opentelemetry::context::{FutureExt, WithContext};
use opentelemetry::metrics::{Gauge, MeterProvider};
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry::trace::{Span as _, TraceContextExt, Tracer as _, TracerProvider};
use opentelemetry::{Context, KeyValue};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{
    BatchConfigBuilder, BatchSpanProcessor, SdkTracer, SdkTracerProvider,
};
use serde_json::Value;

pub use file::{prune, tail};
pub use metrics::{AnnexCall, counter, histogram};
#[cfg(feature = "tokio-console")]
mod console;
#[cfg(feature = "tokio-console")]
pub use console::console_layer;

/// The most the directory holds (12 §12.2).
pub const LIMIT_BYTES: u64 = 50 * 1024 * 1024;
/// When a file is closed and the next begun.
pub const FILE_BYTES: u64 = 5 * 1024 * 1024;

struct Pipeline {
    local: export::Local,
    instruments: metrics::Instruments,
    traces: SdkTracerProvider,
    metrics: SdkMeterProvider,
    gauges: Mutex<HashMap<String, Gauge<f64>>>,
}
static PIPELINE: OnceLock<Pipeline> = OnceLock::new();

/// Starts bounded local export. Calling it again leaves the original
/// device's pipeline alone; no SDK exporter reads an endpoint from the environment.
pub fn init(dir: PathBuf, service: &str, device: &str) {
    let _ = PIPELINE.get_or_init(|| {
        let resource = Resource::builder_empty()
            .with_attributes([
                KeyValue::new("service.name", service.to_owned()),
                KeyValue::new("pigeon.device", device.to_owned()),
                KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            ])
            .build();
        let exporter = export::Local::new(dir);
        let processor = BatchSpanProcessor::builder(exporter.clone())
            .with_batch_config(
                BatchConfigBuilder::default()
                    .with_max_queue_size(2048)
                    .with_max_export_batch_size(64)
                    .with_scheduled_delay(Duration::from_secs(1))
                    .build(),
            )
            .build();
        let traces = SdkTracerProvider::builder()
            .with_resource(resource.clone())
            .with_span_processor(processor)
            .build();
        let reader = PeriodicReader::builder(exporter.clone())
            .with_interval(Duration::from_secs(30))
            .build();
        let metrics = SdkMeterProvider::builder()
            .with_resource(resource)
            .with_reader(reader)
            .build();
        Pipeline {
            local: exporter,
            instruments: metrics::Instruments::default(),
            traces,
            metrics,
            gauges: Mutex::new(HashMap::new()),
        }
    });
}

fn tracer() -> SdkTracer {
    static NOOP: OnceLock<SdkTracerProvider> = OnceLock::new();
    PIPELINE.get().map_or_else(
        || {
            NOOP.get_or_init(|| SdkTracerProvider::builder().build())
                .tracer("pigeon")
        },
        |p| p.traces.tracer("pigeon"),
    )
}

/// The bridge used with the daemon's log layer. OpenTelemetry dependencies
/// stay inside this crate (ADR 0025).
pub fn layer<S>() -> tracing_opentelemetry::OpenTelemetryLayer<S, SdkTracer>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    tracing_opentelemetry::layer().with_tracer(tracer())
}

/// A span, exported by the SDK when dropped.
pub struct Span {
    inner: opentelemetry_sdk::trace::Span,
    inherited: Vec<KeyValue>,
}

/// A root span, or a child of the context currently attached to the task.
pub fn span(name: &str) -> Span {
    let mut span = Span {
        inner: tracer().start(name.to_owned()),
        inherited: Vec::new(),
    };
    if let Some(group) = group() {
        span.set("group", group);
    }
    span
}

#[derive(Clone)]
struct Group(String);
/// Natural group ID of the currently polled operation, if known.
pub fn group() -> Option<String> {
    Context::current().get::<Group>().map(|g| g.0.clone())
}

/// A span continuing a validated W3C traceparent from another process.
pub fn remote_span(name: &str, parent: Option<&str>) -> Span {
    let carrier: HashMap<String, String> = parent
        .map(|p| ("traceparent".into(), p.to_owned()))
        .into_iter()
        .collect();
    let context = opentelemetry_sdk::propagation::TraceContextPropagator::new().extract(&carrier);
    Span {
        inner: tracer().start_with_context(name.to_owned(), &context),
        inherited: Vec::new(),
    }
}

/// The current W3C context. Invalid or absent context stays absent.
pub fn traceparent() -> Option<String> {
    parent_of(&Context::current())
}

fn parent_of(context: &Context) -> Option<String> {
    let mut carrier = HashMap::new();
    opentelemetry_sdk::propagation::TraceContextPropagator::new()
        .inject_context(context, &mut carrier);
    carrier.remove("traceparent")
}

fn attribute(key: &str, value: Value) -> KeyValue {
    let value: opentelemetry::Value = match value {
        Value::Bool(b) => b.into(),
        Value::Number(n) if n.is_i64() => n.as_i64().unwrap_or_default().into(),
        Value::Number(n) => n.as_f64().unwrap_or_default().into(),
        Value::String(s) => s.into(),
        other => other.to_string().into(),
    };
    KeyValue::new(key.to_owned(), value)
}

impl Span {
    /// W3C context for a request, stream header or child environment.
    pub fn traceparent(&self) -> Option<String> {
        parent_of(&self.context())
    }

    fn context(&self) -> Context {
        let context = Context::new().with_remote_span_context(self.inner.span_context().clone());
        match self.inherited.iter().find(|kv| kv.key.as_str() == "group") {
            Some(kv) => context.with_value(Group(kv.value.as_str().into_owned())),
            None => context,
        }
    }

    /// Attaches only during polling, so concurrent Tokio tasks never share
    /// a thread-local context guard across an await point.
    pub fn in_scope<F>(&self, future: F) -> WithContext<F> {
        future.with_context(self.context())
    }

    /// Attaches to synchronous CLI work; keep the guard on this thread.
    pub fn enter(&self) -> opentelemetry::ContextGuard {
        self.context().attach()
    }

    /// A span in this trace, inheriting its natural IDs.
    pub fn child(&self, name: &str) -> Span {
        let context = Context::new().with_remote_span_context(self.inner.span_context().clone());
        let mut inner = tracer().start_with_context(name.to_owned(), &context);
        inner.set_attributes(self.inherited.clone());
        Span {
            inner,
            inherited: self.inherited.clone(),
        }
    }

    /// Records a natural ID or other attribute.
    pub fn set(&mut self, key: &str, value: impl Into<Value>) -> &mut Self {
        let kv = attribute(key, value.into());
        self.inner.set_attribute(kv.clone());
        self.inherited.retain(|old| old.key != kv.key);
        self.inherited.push(kv);
        self
    }

    /// Marks the span failed, with why.
    pub fn fail(&mut self, why: &str) {
        self.inner
            .set_status(opentelemetry::trace::Status::error(why.to_owned()));
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        self.inner.end();
    }
}

/// Records a gauge through the SDK. Instrument names are fixed at call
/// sites; labels carry group and other natural IDs.
pub fn gauge(name: &str, value: f64, labels: &[(&str, &str)]) {
    let Some(p) = PIPELINE.get() else {
        return;
    };
    let mut gauges = p.gauges.lock().unwrap_or_else(|e| e.into_inner());
    let gauge = gauges
        .entry(name.to_owned())
        .or_insert_with(|| p.metrics.meter("pigeon").f64_gauge(name.to_owned()).build());
    let labels: Vec<_> = labels
        .iter()
        .map(|(k, v)| KeyValue::new((*k).to_owned(), (*v).to_owned()))
        .collect();
    gauge.record(value, &labels);
}

/// Flushes pending telemetry for a debug bundle or graceful shutdown.
/// The SDK bounds both queues; export failure never changes file safety.
pub fn flush() {
    if let Some(p) = PIPELINE.get() {
        let _ = p.traces.force_flush();
        let _ = p.metrics.force_flush();
    }
}

#[cfg(test)]
mod tests;

/// A bounded batch for a group's collector; offline telemetry is expendable.
/// Records of any other group remain queued separately.
pub fn take_group(group: &str) -> Vec<Value> {
    PIPELINE
        .get()
        .map_or_else(Vec::new, |p| p.local.take(group))
}

/// Validates and stores authenticated peer telemetry, without putting it
/// in the outbound queue again. The caller checks roster membership and
/// whether this device is the configured collector before calling.
pub fn receive(group: &str, device: &str, mut records: Vec<Value>) -> Option<Vec<Value>> {
    if records.is_empty() || records.iter().any(|r| !route::validate(r, group)) {
        return None;
    }
    let pipeline = PIPELINE.get()?;
    for record in &mut records {
        route::identify(record, group, device);
        pipeline.local.store(record);
    }
    Some(records)
}

/// Forwards OTLP-JSON to a backend explicitly configured on this device.
/// The URL is never obtained from the roster or OTEL environment variables.
/// Redirects are disabled so a local collector cannot redirect private data.
pub async fn forward(endpoint: &str, records: &[Value]) -> bool {
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return false;
    }
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
    else {
        return false;
    };
    for record in records {
        let kind = if record.get("resourceSpans").is_some() {
            "traces"
        } else if record.get("resourceMetrics").is_some() {
            "metrics"
        } else {
            return false;
        };
        let mut target = url.clone();
        target.set_path(&format!("{}/v1/{kind}", url.path().trim_end_matches('/')));
        match client.post(target).json(record).send().await {
            Ok(response) if response.status().is_success() => {}
            _ => return false,
        }
    }
    true
}

/// A diagnostic snapshot restricted to one group, preserving SDK batches.
/// Ungrouped records and records from other groups stay on this computer.
pub fn only_group(records: &[Value], group: &str) -> Vec<Value> {
    records
        .iter()
        .filter_map(|record| route::split(record).remove(group))
        .collect()
}
