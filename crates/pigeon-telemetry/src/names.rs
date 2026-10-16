//! Readable names for the operator dashboard (ADR 0025): next to each
//! device or group ID the collector forwards, the name its signed roster
//! gives it, so Grafana shows "rtx" rather than a 64-digit key. Names come
//! from the collector's roster, never from what a sender claims.

use serde_json::{Value, json};

/// Which roster lookup an ID needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A device ID, named by its roster entry.
    Device,
    /// A group ID, named by its roster's `group`.
    Group,
}

/// The attributes holding an ID, and the attribute that gets its name.
const NAMED: [(&str, &str, Kind); 5] = [
    ("pigeon.device", "pigeon.device.name", Kind::Device),
    ("pigeon.group", "pigeon.group.name", Kind::Group),
    ("device", "device_name", Kind::Device),
    ("peer", "peer_name", Kind::Device),
    ("group", "group_name", Kind::Group),
];

fn text<'a>(attrs: &'a [Value], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|a| a["key"] == key)
        .and_then(|a| a["value"]["stringValue"].as_str())
}

/// Adds the name of each ID attribute it can name, replacing an earlier
/// name, and `extra` attributes, to one attribute list.
fn label(
    attrs: &mut Vec<Value>,
    extra: &[(&str, String)],
    name: &impl Fn(Kind, &str) -> Option<String>,
) {
    let mut add: Vec<(String, String)> = NAMED
        .iter()
        .filter_map(|(id, named, kind)| {
            let found = name(*kind, text(attrs, id)?)?;
            Some(((*named).to_owned(), found))
        })
        .collect();
    add.extend(extra.iter().map(|(k, v)| ((*k).to_owned(), v.clone())));
    attrs.retain(|a| !add.iter().any(|(k, _)| a["key"] == k.as_str()));
    attrs.extend(
        add.into_iter()
            .map(|(k, v)| json!({"key": k, "value": {"stringValue": v}})),
    );
}

fn attributes(item: &mut Value) -> Option<&mut Vec<Value>> {
    if !item["attributes"].is_array() {
        item["attributes"] = json!([]);
    }
    item["attributes"].as_array_mut()
}

/// Names the IDs of OTLP-JSON `records` with `name`, which answers a
/// device's or a group's name. Each metric point and span also gets the
/// sending device's name as `device_name`, so a panel can split by it.
pub fn name(records: &mut [Value], name: impl Fn(Kind, &str) -> Option<String>) {
    for record in records {
        for (resources, scopes, items) in [
            ("resourceMetrics", "scopeMetrics", "metrics"),
            ("resourceSpans", "scopeSpans", "spans"),
        ] {
            let spans = items == "spans";
            let Some(resources) = record.get_mut(resources).and_then(Value::as_array_mut) else {
                continue;
            };
            for r in resources {
                let sender = r["resource"]["attributes"]
                    .as_array()
                    .and_then(|a| text(a, "pigeon.device"))
                    .and_then(|id| name(Kind::Device, id));
                if let Some(resource) = r.get_mut("resource").filter(|v| v.is_object())
                    && let Some(attrs) = attributes(resource)
                {
                    label(attrs, &[], &name);
                }
                let extra: Vec<(&str, String)> =
                    sender.into_iter().map(|s| ("device_name", s)).collect();
                // `get_mut`, never `value[key]`, which inserts a missing key
                // and would give a metric every data kind.
                let scopes = r.get_mut(scopes).and_then(Value::as_array_mut);
                for scope in scopes.into_iter().flatten() {
                    let items = scope.get_mut(items).and_then(Value::as_array_mut);
                    for item in items.into_iter().flatten() {
                        if spans {
                            if let Some(attrs) = attributes(item) {
                                label(attrs, &extra, &name);
                            }
                            continue;
                        }
                        for kind in [
                            "gauge",
                            "sum",
                            "histogram",
                            "exponentialHistogram",
                            "summary",
                        ] {
                            let points = item
                                .get_mut(kind)
                                .and_then(|k| k.get_mut("dataPoints"))
                                .and_then(Value::as_array_mut);
                            for point in points.into_iter().flatten() {
                                if let Some(attrs) = attributes(point) {
                                    label(attrs, &extra, &name);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attr(k: &str, v: &str) -> Value {
        json!({"key": k, "value": {"stringValue": v}})
    }

    fn get<'a>(attrs: &'a Value, k: &str) -> Option<&'a str> {
        text(attrs.as_array().unwrap(), k)
    }

    #[test]
    fn ids_get_their_roster_names_and_points_their_sender() {
        let mut records = vec![json!({
            "resourceMetrics": [{
                "resource": {"attributes": [attr("pigeon.device", "d1"), attr("pigeon.group", "g1")]},
                "scopeMetrics": [{"metrics": [
                    {"name": "pigeon.transfer.bytes", "sum": {"dataPoints": [
                        {"attributes": [attr("group", "g1"), attr("peer", "d2"), attr("peer_name", "forged")]}
                    ]}},
                    {"name": "pigeon.versions", "gauge": {"dataPoints": [
                        {"attributes": [attr("group", "g1"), attr("peer", "unknown")]}
                    ]}}
                ]}]
            }]
        })];
        name(&mut records, |kind, id| match (kind, id) {
            (Kind::Device, "d1") => Some("rtx".into()),
            (Kind::Device, "d2") => Some("air13".into()),
            (Kind::Group, "g1") => Some("48hfp-2026-anim".into()),
            _ => None,
        });
        let r = &records[0]["resourceMetrics"][0];
        assert_eq!(
            get(&r["resource"]["attributes"], "pigeon.device.name"),
            Some("rtx")
        );
        assert_eq!(
            get(&r["resource"]["attributes"], "pigeon.group.name"),
            Some("48hfp-2026-anim")
        );
        let point = &r["scopeMetrics"][0]["metrics"][0]["sum"]["dataPoints"][0]["attributes"];
        assert_eq!(
            get(point, "peer_name"),
            Some("air13"),
            "the roster wins over a sent name"
        );
        assert_eq!(get(point, "group_name"), Some("48hfp-2026-anim"));
        assert_eq!(get(point, "device_name"), Some("rtx"));
        assert_eq!(get(point, "peer"), Some("d2"), "IDs stay");
        let sum = r["scopeMetrics"][0]["metrics"][0].as_object().unwrap();
        assert_eq!(
            sum.keys().collect::<Vec<_>>(),
            ["name", "sum"],
            "naming adds no data kind or key to a metric"
        );
        let other = &r["scopeMetrics"][0]["metrics"][1]["gauge"]["dataPoints"][0]["attributes"];
        assert_eq!(
            get(other, "peer_name"),
            None,
            "an ID the roster lacks gets no name"
        );
    }

    #[test]
    fn spans_are_named_too_and_other_records_are_left_alone() {
        let mut records = vec![
            json!({"resourceSpans": [{
                "resource": {"attributes": [attr("pigeon.device", "d1")]},
                "scopeSpans": [{"spans": [{"name": "api.request", "attributes": [attr("peer", "d1")]}]}]
            }]}),
            json!({"something": "else"}),
        ];
        let before = records[1].clone();
        name(&mut records, |kind, id| {
            (kind == Kind::Device && id == "d1").then(|| "rtx".to_owned())
        });
        let span = &records[0]["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["attributes"];
        assert_eq!(get(span, "peer_name"), Some("rtx"));
        assert_eq!(get(span, "device_name"), Some("rtx"));
        assert_eq!(records[1], before);
    }
}
