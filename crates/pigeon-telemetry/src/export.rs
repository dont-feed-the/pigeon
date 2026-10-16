//! SDK exporters to bounded local OTLP-JSON files (12 §12.2). Conversion
//! uses OpenTelemetry's generated wire types, shared with OTLP back ends.

use super::file::FileSink;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::metrics::{
    Temporality, data::ResourceMetrics, exporter::PushMetricExporter,
};
use opentelemetry_sdk::trace::{SpanData, SpanExporter};
use serde_json::Value;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub(crate) struct Local {
    sink: Arc<FileSink>,
    pending: Arc<Mutex<VecDeque<(String, Value, usize)>>>,
    resource: Arc<Mutex<Resource>>,
}
impl Local {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self {
            pending: Arc::default(),
            sink: Arc::new(FileSink {
                dir,
                file: Mutex::new(None),
            }),
            resource: Arc::new(Mutex::new(Resource::builder_empty().build())),
        }
    }
    fn write(&self, line: &Value) {
        self.store(line);
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        for (group, part) in super::route::split(line) {
            let n = serde_json::to_vec(&part).map_or(256 * 1024, |b| b.len());
            // At most 256 KiB per request; 1 MiB queued for the device.
            if n + 2 <= 256 * 1024 {
                pending.push_back((group, part, n));
            }
        }
        let mut size: usize = pending.iter().map(|p| p.2).sum();
        while size > 1024 * 1024 {
            if let Some((_, _, n)) = pending.pop_front() {
                size -= n;
            } else {
                break;
            }
        }
    }
    pub(crate) fn store(&self, line: &Value) {
        self.sink.write(line);
    }
    pub(crate) fn take(&self, group: &str) -> Vec<Value> {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        let mut size = 2;
        pending.retain(|(g, line, n)| {
            if g == group && size + n < 256 * 1024 {
                out.push(line.clone());
                size += n + 1;
                false
            } else {
                true
            }
        });
        out
    }
}
impl SpanExporter for Local {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let resource = self.resource.lock().unwrap_or_else(|e| e.into_inner());
        let request = ExportTraceServiceRequest {
            resource_spans: group_spans_by_resource_and_scope(batch, &(&*resource).into()),
        };
        if let Ok(line) = serde_json::to_value(request) {
            self.write(&line);
        }
        Ok(())
    }
    fn set_resource(&mut self, resource: &Resource) {
        *self.resource.lock().unwrap_or_else(|e| e.into_inner()) = resource.clone();
    }
}
impl PushMetricExporter for Local {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        if let Ok(line) = serde_json::to_value(ExportMetricsServiceRequest::from(metrics)) {
            self.write(&line);
        }
        Ok(())
    }
    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }
    fn shutdown_with_timeout(&self, _: Duration) -> OTelSdkResult {
        Ok(())
    }
    fn temporality(&self) -> Temporality {
        Temporality::Cumulative
    }
}
