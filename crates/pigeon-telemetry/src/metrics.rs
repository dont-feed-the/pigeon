//! Fixed metric instruments for the operator dashboard (ADR 0025).
//! SDK aggregation bounds point cardinality; no file names become labels.

use opentelemetry::KeyValue;
use opentelemetry::metrics::{Counter, Histogram, MeterProvider};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Default)]
pub(crate) struct Instruments {
    histograms: Mutex<HashMap<String, Histogram<f64>>>,
    counters: Mutex<HashMap<String, Counter<u64>>>,
}

fn labels(labels: &[(&str, &str)]) -> Vec<KeyValue> {
    let mut out: Vec<_> = labels
        .iter()
        .map(|(k, v)| KeyValue::new((*k).to_owned(), (*v).to_owned()))
        .collect();
    if !labels.iter().any(|(k, _)| *k == "group")
        && let Some(group) = super::group()
    {
        out.push(KeyValue::new("group", group));
    }
    out
}

/// Records a duration or lag histogram in seconds.
pub fn histogram(name: &str, value: f64, attributes: &[(&str, &str)]) {
    let Some(p) = super::PIPELINE.get() else {
        return;
    };
    let mut instruments = p
        .instruments
        .histograms
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    instruments
        .entry(name.to_owned())
        .or_insert_with(|| {
            p.metrics
                .meter("pigeon")
                .f64_histogram(name.to_owned())
                .with_unit("s")
                .build()
        })
        .record(value, &labels(attributes));
}

/// Adds to a monotonic counter, such as bytes transferred.
pub fn counter(name: &str, value: u64, attributes: &[(&str, &str)]) {
    let Some(p) = super::PIPELINE.get() else {
        return;
    };
    let mut instruments = p
        .instruments
        .counters
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    instruments
        .entry(name.to_owned())
        .or_insert_with(|| {
            p.metrics
                .meter("pigeon")
                .u64_counter(name.to_owned())
                .build()
        })
        .add(value, &labels(attributes));
}

/// An annex call's timer. Failed spawning, deadline and I/O paths all
/// record failure when the guard drops, rather than disappearing.
pub struct AnnexCall {
    command: String,
    started: Instant,
    success: bool,
}
impl AnnexCall {
    /// Starts one call, labelled by the command alone, never its arguments.
    pub fn start(command: &str) -> Self {
        Self {
            command: command.to_owned(),
            started: Instant::now(),
            success: false,
        }
    }
    /// The annex process exited successfully.
    pub fn succeeded(&mut self) {
        self.success = true;
    }
}
impl Drop for AnnexCall {
    fn drop(&mut self) {
        histogram(
            "pigeon.annex.call.duration",
            self.started.elapsed().as_secs_f64(),
            &[
                ("command", &self.command),
                ("outcome", if self.success { "ok" } else { "error" }),
            ],
        );
    }
}
