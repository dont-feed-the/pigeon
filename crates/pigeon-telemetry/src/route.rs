//! Group isolation for optional collection (ADR 0025). Each transmitted
//! OTLP request contains records of exactly one group. Ungrouped records
//! stay local; an incoming request with mixed or absent group IDs is refused.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn group(item: &Value) -> Option<&str> {
    item.get("attributes")?.as_array()?.iter().find_map(|a| {
        (a["key"] == "group")
            .then(|| a["value"]["stringValue"].as_str())
            .flatten()
    })
}

fn items(line: &mut Value, mut visit: impl FnMut(&mut Vec<Value>)) {
    if let Some(resources) = line.get_mut("resourceSpans").and_then(Value::as_array_mut) {
        for resource in resources {
            if let Some(scopes) = resource.get_mut("scopeSpans").and_then(Value::as_array_mut) {
                for scope in scopes {
                    if let Some(spans) = scope.get_mut("spans").and_then(Value::as_array_mut) {
                        visit(spans);
                    }
                }
            }
        }
    }
    if let Some(resources) = line
        .get_mut("resourceMetrics")
        .and_then(Value::as_array_mut)
    {
        for resource in resources {
            if let Some(scopes) = resource
                .get_mut("scopeMetrics")
                .and_then(Value::as_array_mut)
            {
                for scope in scopes {
                    if let Some(metrics) = scope.get_mut("metrics").and_then(Value::as_array_mut) {
                        for metric in metrics {
                            for kind in [
                                "gauge",
                                "sum",
                                "histogram",
                                "exponentialHistogram",
                                "summary",
                            ] {
                                if let Some(points) = metric
                                    .get_mut(kind)
                                    .and_then(|k| k.get_mut("dataPoints"))
                                    .and_then(Value::as_array_mut)
                                {
                                    visit(points);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn split(line: &Value) -> BTreeMap<String, Value> {
    let mut groups = BTreeSet::new();
    items(&mut line.clone(), |records| {
        groups.extend(records.iter().filter_map(group).map(str::to_owned));
    });
    groups
        .into_iter()
        .map(|g| {
            let mut part = line.clone();
            items(&mut part, |records| {
                records.retain(|r| group(r) == Some(g.as_str()))
            });
            (g, part)
        })
        .collect()
}

pub(crate) fn validate(line: &Value, expected: &str) -> bool {
    // Deserialize the complete request as a supported OTLP wire type.
    let valid = if line.get("resourceSpans").is_some() && line.get("resourceMetrics").is_none() {
        serde_json::from_value::<
            opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest,
        >(line.clone())
        .is_ok()
    } else if line.get("resourceMetrics").is_some() && line.get("resourceSpans").is_none() {
        serde_json::from_value::<
            opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest,
        >(line.clone())
        .is_ok()
    } else {
        false
    };
    let mut count = 0;
    let mut isolated = true;
    items(&mut line.clone(), |records| {
        count += records.len();
        isolated &= records.iter().all(|r| group(r) == Some(expected));
    });
    valid && count > 0 && isolated
}

/// Replaces resource identity with the authenticated stream's source,
/// retaining the sender's other resource attributes.
pub(crate) fn identify(line: &mut Value, group: &str, device: &str) {
    for kind in ["resourceSpans", "resourceMetrics"] {
        if let Some(resources) = line.get_mut(kind).and_then(Value::as_array_mut) {
            for r in resources {
                if !r["resource"].is_object() {
                    r["resource"] = serde_json::json!({});
                }
                if !r["resource"]["attributes"].is_array() {
                    r["resource"]["attributes"] = serde_json::json!([]);
                }
                let attrs = r["resource"]["attributes"].as_array_mut();
                if let Some(attrs) = attrs {
                    attrs.retain(|a| a["key"] != "pigeon.device" && a["key"] != "pigeon.group");
                    for (key, value) in [("pigeon.device", device), ("pigeon.group", group)] {
                        attrs.push(serde_json::json!({"key":key,"value":{"stringValue":value}}));
                    }
                }
            }
        }
    }
}
