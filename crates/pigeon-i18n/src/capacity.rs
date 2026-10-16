//! Capacity text shared by the command line and Space page (08 §8.9).
//! Signed disk observations keep their age; unknown values stay unknown.

use crate::{Message, render::Renderer};
use pigeon_proto::capacity::Report;

fn number(n: u64) -> crate::Arg {
    i64::try_from(n).map_or_else(|_| crate::Arg::Text(n.to_string()), crate::Arg::Number)
}

pub(crate) fn lines(r: &Renderer, report: &Report) -> Vec<String> {
    let say = |m: Message| r.catalogue().format(&m);
    let mut lines = vec![
        say(Message::new("space-snapshot")
            .arg("group", report.group.clone())
            .arg("when", report.at.clone())),
        say(Message::new("space-need")
            .arg("copies", i64::from(report.k))
            .arg("current", number(report.current))
            .arg("history", number(report.history))
            .arg("suggestions", number(report.suggestions))),
        say(Message::new("space-offered").arg("bytes", number(report.configured))),
        report.overflow.map_or_else(
            || r.id("space-overflow-unknown"),
            |bytes| say(Message::new("space-overflow").arg("bytes", number(bytes))),
        ),
    ];
    if report.short {
        lines.push(r.id("space-short"));
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
            disk.maxsize.to_string()
        };
        lines.push(say(Message::new("space-disk")
            .arg("used", number(disk.used))
            .arg("free", number(disk.free))
            .arg("limit", limit)
            .arg("reserve", number(disk.reserve))));
        let id = if machine.report_fresh {
            "web-health-report"
        } else {
            "web-health-report-old"
        };
        lines.push(
            r.text(&pigeon_proto::Message {
                id: id.into(),
                args: [(
                    "when".into(),
                    pigeon_proto::Arg::Text(machine.reported_at.clone().unwrap_or_default()),
                )]
                .into(),
            }),
        );
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
        let saved = pigeon_proto::time::parse_rfc3339(&proposal.saved_at).map_or_else(
            || proposal.saved_at.clone(),
            |at| {
                if at > r.now() {
                    proposal.saved_at.clone()
                } else {
                    r.ago(r.now().saturating_sub(at))
                }
            },
        );
        lines.push(say(Message::new("space-proposal")
            .arg("id", proposal.id.clone())
            .arg("state", r.id(state))
            .arg("when", saved)
            .arg("bytes", number(proposal.additional_bytes))));
    }
    lines.push(r.id("space-shared"));
    lines
}
