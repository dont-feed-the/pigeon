//! Capacity text shared by the command line and Space page (08 §8.9).
//! Signed disk observations keep their age; unknown values stay unknown.
//! Sizes read as "29.3 GB" and times as "5 minutes ago" (ux.md §4).

use crate::{Message, render::Renderer};
use pigeon_proto::capacity::Report;

/// `at`, an RFC 3339 time, as long ago; a time to come, or one that is not
/// a time, stays as it is.
fn ago(r: &Renderer, at: &str) -> String {
    pigeon_proto::time::parse_rfc3339(at).map_or_else(
        || at.to_owned(),
        |t| {
            if t > r.now() {
                at.to_owned()
            } else {
                r.ago(r.now().saturating_sub(t))
            }
        },
    )
}

pub(crate) fn lines(r: &Renderer, report: &Report) -> Vec<String> {
    let say = |m: Message| r.catalogue().format(&m);
    let mut lines = vec![
        say(Message::new("space-snapshot")
            .arg("group", report.group.clone())
            .arg("when", ago(r, &report.at))),
        say(Message::new("space-need")
            .arg("copies", i64::from(report.k))
            .arg("current", r.size(report.current))
            .arg("history", r.size(report.history))
            .arg("suggestions", r.size(report.suggestions))),
        say(Message::new("space-offered").arg("bytes", r.size(report.configured))),
        report.overflow.map_or_else(
            || r.id("space-overflow-unknown"),
            |bytes| say(Message::new("space-overflow").arg("bytes", r.size(bytes))),
        ),
    ];
    if !report.limited.is_empty() {
        lines.push(say(
            Message::new("space-limit-low").arg("machines", report.limited.join(", "))
        ));
    }
    if report.no_spare {
        lines.push(r.id("space-no-spare"));
    }
    for machine in report.machines.iter().filter(|m| m.machine.custodian) {
        lines.push(r.machine(&machine.machine));
        let Some(disk) = &machine.disk else {
            lines.push(r.id("web-health-no-report"));
            continue;
        };
        let limit = if disk.maxsize == 0 {
            r.id("web-health-no-limit")
        } else {
            r.size(disk.maxsize)
        };
        lines.push(say(Message::new("space-disk")
            .arg("used", r.size(disk.used))
            .arg("free", r.size(disk.free))
            .arg("limit", limit)
            .arg("reserve", r.size(disk.reserve))));
        let id = if machine.report_fresh {
            "web-health-report"
        } else {
            "web-health-report-old"
        };
        lines.push(match &machine.reported_at {
            Some(at) => say(Message::new(id).arg("when", ago(r, at))),
            None => r.id("web-health-no-report"),
        });
        if machine.repository_lost {
            lines.push(r.id("web-health-lost"));
        }
        if machine.checking {
            lines.push(r.id("web-health-checking"));
        }
    }
    for proposal in &report.proposals {
        let state = match proposal.state.as_str() {
            "open" => "space-open",
            "adopted" => "space-adopted",
            "rejected" => "space-rejected",
            "withdrawn" => "space-withdrawn",
            _ => "web-health-unavailable",
        };
        lines.push(say(Message::new("space-proposal")
            .arg("id", proposal.id.clone())
            .arg("state", r.id(state))
            .arg("when", ago(r, &proposal.saved_at))
            .arg("bytes", r.size(proposal.additional_bytes))));
    }
    lines.push(r.id("space-shared"));
    lines
}
