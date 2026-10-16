//! Shared erasure text for the CLI and browser (08 §8.6, ADR 0028).
//! Counts refer to versions; warnings never promise shared bytes as free.

use crate::{Message, render::Renderer};
use pigeon_proto::erase::{Answer, Readiness};

fn readiness(r: &Renderer, state: &Readiness) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(problem) = &state.problem {
        out.push(r.text(problem));
    }
    if state.ready {
        return out;
    }
    out.push(r.id(if state.preparation_needed {
        "erase-prepare-needed"
    } else {
        "erase-readiness-wait"
    }));
    if !state.waiting.is_empty() {
        out.push(
            r.catalogue().format(
                &Message::new("erase-waiting").arg(
                    "names",
                    state
                        .waiting
                        .iter()
                        .map(|w| w.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ),
        );
    }
    if !state.open_decisions.is_empty() {
        out.push(r.id("erase-old-decisions"));
    }
    out
}

pub(crate) fn lines(r: &Renderer, answer: &Answer) -> Vec<String> {
    let text = |message: Message| r.catalogue().format(&message);
    match answer {
        Answer::Preview(p) => {
            let mut out = vec![
                text(
                    Message::new("erase-preview-counts")
                        .arg("files", p.current_files)
                        .arg("versions", p.versions)
                        .arg("bytes", p.bytes),
                ),
                text(Message::new("erase-warning").arg("days", u64::from(p.retention_days))),
            ];
            if p.shared_versions > 0 {
                out.push(text(
                    Message::new("erase-shared")
                        .arg("versions", p.shared_versions)
                        .arg("paths", p.retained_elsewhere.join(", ")),
                ));
            }
            if p.needs_approval {
                out.push(r.id("erase-needs-approval"));
            }
            out.extend(readiness(r, &p.readiness));
            out
        }
        Answer::Preparing(state) => {
            let mut out = vec![r.id("erase-preparing")];
            out.extend(readiness(r, state));
            out
        }
        Answer::Submitted { id, needs_approval } => vec![text(
            Message::new(if *needs_approval {
                "erase-submitted-suggestion"
            } else {
                "erase-submitted"
            })
            .arg("id", id.as_str()),
        )],
        Answer::Decisions { decisions, .. } if decisions.is_empty() => vec![r.id("erase-none")],
        Answer::Decisions { decisions, .. } => decisions
            .iter()
            .flat_map(|d| {
                let mut out = vec![text(
                    Message::new("erase-decision")
                        .arg("id", d.id.as_str())
                        .arg("paths", d.paths.join(", "))
                        .arg("time", d.not_before.as_str()),
                )];
                out.push(r.id(if d.completed {
                    "erase-completed"
                } else if d.membership_changed {
                    "erase-membership-changed"
                } else {
                    "erase-pending"
                }));
                if !d.waiting.is_empty() {
                    out.push(text(
                        Message::new("erase-waiting").arg(
                            "names",
                            d.waiting
                                .iter()
                                .map(|w| w.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", "),
                        ),
                    ));
                }
                out
            })
            .collect(),
        Answer::CancelPreview { decision, .. } => vec![text(
            Message::new("erase-cancel-warning").arg("id", decision.id.as_str()),
        )],
        Answer::Cancelled { id } => {
            vec![text(Message::new("erase-cancelled").arg("id", id.as_str()))]
        }
    }
}
