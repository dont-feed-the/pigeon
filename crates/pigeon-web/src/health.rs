//! Home's group-health view (12 §12.2): replicated facts and their times,
//! with explicit unavailable and stale observations. Live updates replace
//! text only; names are escaped and membership changes reload the view.

use maud::{Markup, html};
use pigeon_i18n::{Message, render::Renderer};
use pigeon_proto::health::{Counts, HealthReport, MachineHealth};
use std::collections::BTreeMap;

fn number(n: u64) -> i64 {
    n.try_into().unwrap_or(i64::MAX)
}

fn counts(r: &Renderer, counts: Option<&Counts>) -> String {
    let Some(c) = counts else {
        return r.id("web-health-unavailable");
    };
    r.catalogue().format(
        &Message::new("web-health-counts")
            .arg("protected", number(c.protected))
            .arg("published", number(c.published))
            .arg("local", number(c.local))
            .arg("uncertain", number(c.uncertain))
            .arg("missing", number(c.missing))
            .arg("erased", number(c.erased)),
    )
}

fn ago(r: &Renderer, time: Option<&str>) -> String {
    time.and_then(pigeon_proto::time::parse_rfc3339)
        .map_or_else(
            || r.id("web-health-no-report"),
            |t| r.ago(crate::now().saturating_sub(t)),
        )
}

fn size(r: &Renderer, bytes: u64) -> String {
    if bytes < 1000 {
        return r
            .catalogue()
            .format(&Message::new("web-health-bytes").arg("bytes", number(bytes)));
    }
    if bytes < 1_000_000 {
        return r
            .catalogue()
            .format(&Message::new("web-health-kb").arg("kb", number(bytes / 1000)));
    }
    r.catalogue()
        .format(&Message::new("web-health-size").arg("mb", number(bytes / 1_000_000)))
}

fn disk(r: &Renderer, machine: &MachineHealth) -> String {
    if machine.machine.recovery {
        return r.id("web-health-paper");
    }
    let Some(disk) = &machine.disk else {
        return r.id("web-health-no-report");
    };
    let limit = if disk.maxsize == 0 {
        r.id("web-health-no-limit")
    } else {
        size(r, disk.maxsize)
    };
    let mut text = r.catalogue().format(
        &Message::new("web-health-disk")
            .arg("used", size(r, disk.used))
            .arg("free", size(r, disk.free))
            .arg("limit", limit)
            .arg("reserve", size(r, disk.reserve)),
    );
    if disk.full {
        text.push(' ');
        text.push_str(&r.id("web-health-full"));
    }
    text
}

fn work(r: &Renderer, machine: &MachineHealth) -> String {
    if machine.machine.recovery {
        return String::new();
    }
    if machine.repository_lost {
        return r.id("web-health-lost");
    }
    if machine.checking {
        return r.id("web-health-checking");
    }
    let Some(queue) = &machine.queue else {
        return r.id("web-health-no-report");
    };
    r.catalogue().format(
        &Message::new("web-health-queue")
            .arg("send", size(r, queue.send_bytes))
            .arg("receive", size(r, queue.receive_bytes))
            .arg("deferred", number(queue.deferred_keys))
            .arg("missing", size(r, queue.missing_bytes)),
    )
}

fn reported(r: &Renderer, machine: &MachineHealth) -> String {
    if machine.machine.recovery {
        return String::new();
    }
    if machine.reported_at.is_none() {
        return r.id("web-health-no-report");
    }
    let time = ago(r, machine.reported_at.as_deref());
    let id = if machine.report_fresh {
        "web-health-report"
    } else {
        "web-health-report-old"
    };
    r.catalogue().format(&Message::new(id).arg("when", time))
}

fn values(r: &Renderer, h: &HealthReport) -> BTreeMap<String, String> {
    let mut values: BTreeMap<String, String> = [
        ("health-group", h.group.clone()),
        ("health-current", counts(r, h.current.as_ref())),
        ("health-retained", counts(r, h.retained.as_ref())),
        (
            "health-suggestions",
            h.suggestions.map_or_else(
                || r.id("web-health-unavailable"),
                |n| {
                    r.catalogue()
                        .format(&Message::new("web-health-suggestions").arg("count", number(n)))
                },
            ),
        ),
        (
            "health-repair",
            r.catalogue()
                .format(&Message::new("web-health-repair").arg("count", number(h.endangered))),
        ),
        (
            "health-updated",
            r.catalogue()
                .format(&Message::new("web-health-updated").arg("when", ago(r, Some(&h.at)))),
        ),
        (
            "health-omitted",
            if h.omitted_paths == 0 {
                String::new()
            } else {
                r.catalogue().format(
                    &Message::new("web-health-omitted").arg("count", number(h.omitted_paths)),
                )
            },
        ),
    ]
    .into_iter()
    .map(|(id, text)| (id.into(), text))
    .collect();
    for machine in &h.machines {
        let id = &machine.machine.device;
        values.insert(format!("health-machine-{id}"), r.machine(&machine.machine));
        values.insert(format!("health-report-{id}"), reported(r, machine));
        values.insert(format!("health-disk-{id}"), disk(r, machine));
        values.insert(format!("health-work-{id}"), work(r, machine));
        values.insert(
            format!("health-scrub-{id}"),
            if machine.machine.custodian {
                r.scrub(machine.scrub.as_ref()).join(" ")
            } else {
                String::new()
            },
        );
    }
    for epoch in h.repair_epochs.iter().flatten() {
        values.insert(
            format!("health-epoch-{}-{}", epoch.commit, epoch.device),
            r.catalogue().format(
                &Message::new("web-health-epoch")
                    .arg("name", epoch.name.as_str())
                    .arg("when", ago(r, Some(&epoch.started_at))),
            ),
        );
    }
    values
}

fn membership(h: &HealthReport) -> String {
    serde_json::json!([
        h.group_id,
        h.machines
            .iter()
            .map(|m| m.machine.device.as_str())
            .collect::<Vec<_>>(),
        h.repair_epochs
            .as_ref()
            .map(|epochs| epochs.iter().map(|e| e.commit.as_str()).collect::<Vec<_>>())
    ])
    .to_string()
}

pub(crate) fn event(r: &Renderer, h: &HealthReport) -> String {
    let explanations: Vec<_> = r
        .response(&pigeon_proto::Response::Health(Box::new(h.clone())))
        .0
        .into_iter()
        .skip(1)
        .collect();
    serde_json::json!({"membership": membership(h), "values": values(r,h), "explanations": explanations}).to_string()
}

pub(crate) fn section(r: &crate::Renderer, h: &HealthReport) -> Markup {
    let values = values(r, h);
    let text = |id: &str| values.get(id).map_or("", String::as_str);
    html! {
        section id="health" data-membership=(membership(h)) aria-labelledby="health-title" {
            h2 id="health-title" { (r.id("web-health")) ": " span id="health-group" { (text("health-group")) } }
            h3 { (r.id("web-health-current")) }
            p id="health-current" { (text("health-current")) }
            h3 { (r.id("web-health-retained")) }
            p id="health-retained" { (text("health-retained")) }
            p { a href=(r.url("/suggestions")) id="health-suggestions" { (text("health-suggestions")) } }
            p id="health-repair" { (text("health-repair")) }
            p id="health-omitted" { (text("health-omitted")) }
            details {
                summary { (r.id("web-health-machines")) }
                @for machine in &h.machines {
                    @let id = &machine.machine.device;
                    article {
                        h3 id=(format!("health-machine-{id}")) { (text(&format!("health-machine-{id}"))) }
                        p id=(format!("health-report-{id}")) { (text(&format!("health-report-{id}"))) }
                        p id=(format!("health-disk-{id}")) { (text(&format!("health-disk-{id}"))) }
                        p id=(format!("health-work-{id}")) { (text(&format!("health-work-{id}"))) }
                        @if machine.machine.custodian {
                            p id=(format!("health-scrub-{id}")) { (text(&format!("health-scrub-{id}"))) }
                        }
                    }
                }
                p { a href=(r.url("/machines")) { (r.id("web-nav-machines")) } }
            }
            details {
                summary { (r.id("web-health-epochs")) }
                @if let Some(epochs) = &h.repair_epochs {
                    @if epochs.is_empty() { p { (r.id("web-health-no-epochs")) } }
                    @for epoch in epochs.iter().rev() {
                        @let id = format!("health-epoch-{}-{}", epoch.commit, epoch.device);
                        p id=(&id) { (text(&id)) }
                    }
                } @else { p { (r.id("web-health-unavailable")) } }
            }
            p id="health-updated" { (text("health-updated")) }
        }
    }
}
