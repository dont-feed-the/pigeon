//! Turns `pigeond`'s responses into text with the Fluent catalogue (ADR
//! 0028, 10 §10.5–10.6), for the CLI and the web interface: the daemon
//! sends message ids and arguments, times in RFC 3339, and this module
//! renders them in the person's language, past times relative to now and
//! deadlines in this computer's local time (`units.rs`).

use crate::{Catalogue, ids};
use pigeon_proto::api::{FileEntry, FreePreview, Machine, State, StatusReport, VersionStatus};
use pigeon_proto::{Arg, Message, Problem, Response};

/// The arguments that hold a past time, shown relative to now.
const PAST: &[&str] = &["ago", "when", "since", "heard", "saved"];

/// The day a recovery sheet named `YYYY-MM-DD-xxxxxx` was printed.
fn sheet_date(name: &str) -> Option<&str> {
    let date = name.get(..10)?;
    let shape = date.bytes().enumerate().all(|(i, b)| {
        if i == 4 || i == 7 {
            b == b'-'
        } else {
            b.is_ascii_digit()
        }
    });
    shape.then_some(date)
}

/// Renders responses as lines of text, for the CLI and the web pages.
pub struct Renderer {
    cat: Catalogue,
    now: i64,
    /// Minutes east of UTC of the local time zone at a time.
    offset: fn(i64) -> i32,
}

impl Renderer {
    /// The renderer of `cat`'s language at time `now`, in Unix seconds,
    /// with deadlines in this computer's time zone.
    pub fn new(cat: Catalogue, now: i64) -> Renderer {
        Renderer {
            cat,
            now,
            offset: pigeon_sys::clock::local_offset_minutes,
        }
    }

    /// The same, with deadlines in the time zone `offset` gives, in
    /// minutes east of UTC; for tests.
    #[must_use]
    pub fn with_offset(mut self, offset: fn(i64) -> i32) -> Renderer {
        self.offset = offset;
        self
    }

    /// The deadline `at`, in Unix seconds, as the argument `name` words it:
    /// `until` ("until 14:38") or `expires` ("expires tomorrow at 09:00").
    pub fn deadline(&self, name: &str, at: i64) -> String {
        crate::units::deadline(&self.cat, name, at, self.now, self.offset)
    }

    /// The local date of `at`, in Unix seconds: "16 October".
    pub fn date(&self, at: i64) -> String {
        crate::units::day(&self.cat, at, self.now, self.offset)
    }

    /// `bytes` as people read a size: "29.3 GB".
    pub fn size(&self, bytes: u64) -> String {
        crate::units::size(&self.cat, bytes)
    }

    /// This renderer's fixed observation time.
    pub(crate) fn now(&self) -> i64 {
        self.now
    }

    /// Its catalogue.
    pub fn catalogue(&self) -> &Catalogue {
        &self.cat
    }

    /// `secs` seconds ago, in words.
    pub fn ago(&self, secs: i64) -> String {
        let secs = secs.max(0);
        let m = if secs < 60 {
            crate::Message::new("time-just-now")
        } else if secs < 3600 {
            crate::Message::new("time-minutes-ago").arg("minutes", secs / 60)
        } else if secs < 86_400 {
            crate::Message::new("time-hours-ago").arg("hours", secs / 3600)
        } else {
            crate::Message::new("time-days-ago").arg("days", secs / 86_400)
        };
        self.cat.format(&m)
    }

    /// The text of `m`.
    pub fn text(&self, m: &Message) -> String {
        let mut out = crate::Message::new(m.id.clone());
        for (name, value) in &m.args {
            let v = match value {
                Arg::Number(n) => crate::Arg::Number(*n),
                Arg::Text(t) if PAST.contains(&name.as_str()) => {
                    match pigeon_proto::time::parse_rfc3339(t) {
                        Some(at) => crate::Arg::Text(self.ago(self.now - at)),
                        None => crate::Arg::Text(t.clone()),
                    }
                }
                Arg::Text(t) if crate::units::DEADLINES.iter().any(|(n, _)| n == name) => {
                    match pigeon_proto::time::parse_rfc3339(t) {
                        Some(at) => crate::Arg::Text(self.deadline(name, at)),
                        None => crate::Arg::Text(t.clone()),
                    }
                }
                Arg::Text(t) if name == "date" => match pigeon_proto::time::parse_rfc3339(t) {
                    Some(at) => crate::Arg::Text(self.date(at)),
                    None => crate::Arg::Text(t.clone()),
                },
                Arg::Text(t) => crate::Arg::Text(t.clone()),
            };
            out.args.insert(name.clone(), v);
        }
        self.cat.format(&out)
    }

    /// The text of the message `id`, which takes no argument.
    pub fn id(&self, id: &str) -> String {
        self.cat.format(&crate::Message::new(id))
    }

    /// The text of a state.
    pub fn state(&self, s: State) -> String {
        self.id(match s {
            State::Local => "state-local",
            State::Published => "state-published",
            State::Protected => "state-protected",
            State::Missing => "state-missing",
            State::Erased => "state-erased",
        })
    }

    /// The text of a file's state, and that it is not on this computer
    /// when its space is freed: "Safe, not on this computer" (08 §8.5).
    pub fn file_state(&self, f: &FileEntry) -> String {
        if f.freed {
            self.cat
                .format(&crate::Message::new("state-not-here").arg("state", self.state(f.state)))
        } else {
            self.state(f.state)
        }
    }

    /// A file's line: its path and [`Renderer::file_state`].
    pub fn file(&self, f: &FileEntry) -> String {
        self.cat.format(
            &crate::Message::new("file-row")
                .arg("path", f.path.clone())
                .arg("state", self.file_state(f)),
        )
    }

    /// What freeing a folder would do, for its confirmation (08 §8.5):
    /// which files leave this computer, where they stay, which wait, and
    /// how to bring them back.
    pub fn free_preview(&self, p: &FreePreview) -> Vec<String> {
        let n = |x: u64| i64::try_from(x).unwrap_or(i64::MAX);
        let mut out = Vec::new();
        if p.safe == 0 {
            out.push(self.id("free-preview-none"));
        } else {
            out.push(
                self.cat
                    .format(&crate::Message::new("free-preview-leave").arg("files", n(p.safe))),
            );
            if !p.backups.is_empty() {
                out.push(
                    self.cat.format(
                        &crate::Message::new("free-preview-stay")
                            .arg("files", n(p.safe))
                            .arg("machines", p.backups.join(", ")),
                    ),
                );
            }
        }
        let waiting = p.files.saturating_sub(p.safe);
        if waiting > 0 {
            out.push(
                self.cat
                    .format(&crate::Message::new("free-waiting").arg("files", n(waiting))),
            );
        }
        out.push(self.id("free-preview-undo"));
        out
    }

    /// The lines of a problem: what, what to do, and its code.
    pub fn problem(&self, p: &Problem) -> Vec<String> {
        let mut out = vec![self.text(&p.message)];
        if let Some(h) = &p.hint {
            out.push(self.text(h));
        } else if let Some(h) = ids::hint_id(&p.code) {
            let hint = Message {
                id: h.to_owned(),
                args: p.message.args.clone(),
            };
            out.push(self.text(&hint));
        }
        let code = crate::Message::new("problem-code").arg("code", ids::display_code(&p.code));
        out.push(self.cat.format(&code));
        out
    }

    /// Integrity findings with their own completion time; pending local
    /// progress cannot become a claim that a remote pass has finished.
    pub fn scrub(&self, scrub: Option<&pigeon_proto::health::Scrub>) -> Vec<String> {
        let Some(scrub) = scrub else {
            return vec![self.id("scrub-no-pass")];
        };
        let mut out = Vec::new();
        if let Some(problem) = &scrub.problem {
            out.push(self.text(problem));
        }
        if let Some(at) = &scrub.last_pass_completed {
            out.push(
                self.cat.format(
                    &crate::Message::new("scrub-last-pass")
                        .arg("time", at.as_str())
                        .arg("failed", scrub.keys_failed),
                ),
            );
        } else if scrub.problem.is_none() {
            out.push(self.id("scrub-no-pass"));
        }
        if let Some(remaining) = scrub.remaining {
            out.push(
                self.cat.format(
                    &crate::Message::new("scrub-progress")
                        .arg("remaining", remaining)
                        .arg("failed", scrub.current_failures.unwrap_or(0)),
                ),
            );
        }
        out
    }

    /// The lines of a status report: the headline, then each version waiting.
    pub fn status(&self, r: &StatusReport) -> Vec<String> {
        let mut out = vec![self.text(&r.headline)];
        for v in &r.versions {
            out.push(format!("  {}", self.waiting(v)));
            if let Some(reason) = &v.reason {
                out.push(format!("    {}", self.text(reason)));
            }
        }
        for n in &r.not_shared {
            out.push(format!("  {}", self.text(&n.reason)));
        }
        out
    }

    /// A version waiting, with its state.
    pub fn waiting(&self, v: &VersionStatus) -> String {
        let m = crate::Message::new("waiting-row")
            .arg("path", v.path.as_str())
            .arg("state", self.state(v.state));
        self.cat.format(&m)
    }

    /// A held batch: its ID, why, how many files and its first paths.
    pub fn held(&self, h: &pigeon_proto::api::HeldEntry) -> String {
        let files = i64::try_from(h.files).unwrap_or(i64::MAX);
        let since = pigeon_proto::time::parse_rfc3339(&h.since)
            .map_or_else(|| h.since.clone(), |t| self.ago(self.now - t));
        let msg = crate::Message::new("held-row")
            .arg("id", h.id.as_str())
            .arg("reason", self.text(&h.reason).as_str())
            .arg("files", files)
            .arg("since", since)
            .arg("paths", h.paths.join(", ").as_str());
        self.cat.format(&msg)
    }

    /// A person of the roster, with their machines.
    pub fn person(&self, p: &pigeon_proto::api::PersonEntry) -> String {
        let role = if p.admin {
            "person-admin"
        } else {
            "person-member"
        };
        let msg = crate::Message::new("person-row")
            .arg("name", p.name.as_str())
            .arg("role", self.id(role))
            .arg("machines", p.machines.join(", "));
        let mut line = self.cat.format(&msg);
        if p.recovery_recommended {
            line.push(' ');
            line.push_str(&self.cat.format(
                &crate::Message::new("recovery-recommendation").arg("name", p.name.as_str()),
            ));
        }
        if p.recovery_refresh_recommended {
            line.push(' ');
            line.push_str(
                &self.cat.format(
                    &crate::Message::new("recovery-refresh-recommendation")
                        .arg("name", p.name.as_str()),
                ),
            );
        }
        line
    }

    /// A machine of the roster.
    pub fn machine(&self, m: &Machine) -> String {
        if m.recovery {
            // A sheet is named `YYYY-MM-DD-xxxxxx` after the day it was
            // printed (09 §9.9).
            let msg = match sheet_date(&m.name) {
                Some(date) => crate::Message::new("machine-recovery")
                    .arg("name", m.name.as_str())
                    .arg("date", date),
                None => {
                    crate::Message::new("machine-recovery-undated").arg("name", m.name.as_str())
                }
            };
            return self.cat.format(&msg);
        }
        if m.this_device {
            let msg = crate::Message::new("machine-this").arg("name", m.name.as_str());
            return self.cat.format(&msg);
        }
        let kind = if m.custodian {
            "machine-kind-custodian"
        } else {
            "machine-kind-computer"
        };
        let heard = m
            .heard_at
            .as_deref()
            .and_then(pigeon_proto::time::parse_rfc3339)
            .map_or_else(|| self.id("time-never"), |t| self.ago(self.now - t));
        let msg = crate::Message::new("machine-row")
            .arg("name", m.name.as_str())
            .arg("kind", self.id(kind))
            .arg("heard", heard);
        self.cat.format(&msg)
    }

    /// A version in a file's history.
    pub fn version(&self, v: &VersionStatus) -> String {
        let saved = pigeon_proto::time::parse_rfc3339(&v.saved_at)
            .map_or_else(|| v.saved_at.clone(), |t| self.ago(self.now - t));
        let msg = crate::Message::new("version-row")
            .arg("number", v.number.to_string())
            .arg("saved", saved)
            .arg("state", self.state(v.state));
        self.cat.format(&msg)
    }

    /// A deleted file's line (ux.md §6.2).
    pub fn deleted(&self, f: &pigeon_proto::api::DeletedFile) -> String {
        let when = pigeon_proto::time::parse_rfc3339(&f.deleted_at)
            .map_or_else(|| f.deleted_at.clone(), |t| self.ago(self.now - t));
        let msg = crate::Message::new("deleted-row")
            .arg("path", f.path.clone())
            .arg("deleted", when);
        self.cat.format(&msg)
    }

    /// A top-level folder's line (ux.md §6.2).
    pub fn folder_rule(
        &self,
        rule: &pigeon_proto::policy::Rule,
        people: &[pigeon_proto::policy::Person],
    ) -> String {
        let name = |id: &str| {
            people
                .iter()
                .find(|p| p.id == id)
                .map_or_else(|| id.to_owned(), |p| p.name.clone())
        };
        let path = if rule.path.is_empty() {
            "."
        } else {
            &rule.path
        };
        let message = match &rule.owner {
            None if rule.per_person => crate::Message::new("folder-rule-per-person")
                .arg("folder", path)
                .arg(
                    "folders",
                    people
                        .iter()
                        .map(|p| format!("{path}/{}", p.id))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            Some(owner) => crate::Message::new("folder-rule-owned")
                .arg("folder", path)
                .arg("owner", name(owner)),
            None => crate::Message::new("folder-rule-add-only")
                .arg("folder", path)
                .arg(
                    "stewards",
                    if rule.stewards.is_empty() {
                        people
                            .iter()
                            .filter(|p| p.admin)
                            .map(|p| p.name.clone())
                            .collect::<Vec<_>>()
                    } else {
                        rule.stewards.iter().map(|id| name(id)).collect::<Vec<_>>()
                    }
                    .join(", "),
                ),
        };
        self.cat.format(&message)
    }

    /// A top-level folder's line (ux.md §6.2).
    pub fn folder(&self, f: &pigeon_proto::api::Folder) -> String {
        let id = if f.freed {
            "folder-row-freed"
        } else {
            "folder-row"
        };
        let files = i64::try_from(f.files).unwrap_or(i64::MAX);
        self.cat.format(
            &crate::Message::new(id)
                .arg("path", f.path.clone())
                .arg("state", self.state(f.state))
                .arg("files", files),
        )
    }

    /// A suggestion waiting for a decision.
    pub fn suggestion(&self, p: &pigeon_proto::api::Suggestion) -> String {
        let saved = pigeon_proto::time::parse_rfc3339(&p.saved_at)
            .map_or_else(|| p.saved_at.clone(), |t| self.ago(self.now - t));
        let id = if p.mine {
            "suggestion-yours"
        } else if p.decide {
            "suggestion-decide"
        } else {
            "suggestion-other"
        };
        let msg = crate::Message::new(id)
            .arg("id", p.id.get(..12).unwrap_or(&p.id).to_owned())
            .arg("author", p.author.clone())
            .arg("paths", p.paths.join(", "))
            .arg("saved", saved);
        self.cat.format(&msg)
    }

    /// The lines of a response, and whether they go to standard error.
    pub fn response(&self, r: &Response) -> (Vec<String>, bool) {
        match r {
            Response::FolderRules(report) => (
                report
                    .rules
                    .iter()
                    .map(|rule| self.folder_rule(rule, &report.people))
                    .collect(),
                false,
            ),
            Response::Said { message } if message.id == "recovery-sheet-text" => {
                let sheet = match message.args.get("sheet") {
                    Some(Arg::Text(sheet)) => crate::recovery::terminal(&self.cat, sheet),
                    _ => self.text(message),
                };
                (vec![sheet], false)
            }
            Response::Said { message } => (vec![self.text(message)], false),
            Response::Progress { message, .. } => (vec![self.text(message)], true),
            Response::Problem(p) => (self.problem(p), true),
            Response::Status(s) => {
                let mut lines = self.status(s);
                // The pause, when the headline is about something else
                // (15 §15.6).
                if let Some(p) = s.paused.as_ref().filter(|p| **p != s.headline) {
                    lines.insert(1.min(lines.len()), self.text(p));
                }
                (lines, false)
            }
            Response::Erasure { answer } => (crate::erase::lines(self, answer), false),
            Response::Capacity(report) => (crate::capacity::lines(self, report), false),
            Response::Health(h) => (self.status(&h.status), false),
            Response::Machines { machines, .. } => {
                (machines.iter().map(|m| self.machine(m)).collect(), false)
            }
            Response::Held { held } if held.is_empty() => (vec![self.id("held-none")], false),
            Response::Held { held } => (held.iter().map(|h| self.held(h)).collect(), false),
            Response::People { people } => (people.iter().map(|p| self.person(p)).collect(), false),
            Response::History { versions } => {
                (versions.iter().map(|v| self.version(v)).collect(), false)
            }
            Response::Proposals {
                proposals, decided, ..
            } => {
                let mut lines: Vec<String> = proposals
                    .iter()
                    .flat_map(|p| self.suggestion_lines(p))
                    .collect();
                if proposals.is_empty() {
                    lines.push(self.id("suggestions-none"));
                }
                lines.extend(decided.iter().flat_map(|o| self.outcome(o)));
                (lines, false)
            }
            Response::Conflicts { conflicts } if conflicts.is_empty() => {
                (vec![self.id("conflicts-none")], false)
            }
            Response::Conflicts { conflicts } => (
                conflicts
                    .iter()
                    .map(|c| {
                        let id = if c.state == "waiting" {
                            "conflict-waiting"
                        } else {
                            "conflict-choose"
                        };
                        self.cat
                            .format(&crate::Message::new(id).arg("path", c.path.clone()))
                    })
                    .collect(),
                false,
            ),
            Response::Folders { folders, .. } if folders.is_empty() => {
                (vec![self.id("folders-none")], false)
            }
            Response::Folders { folders, .. } => {
                (folders.iter().map(|f| self.folder(f)).collect(), false)
            }
            Response::Sync { folders, .. } if folders.is_empty() => {
                (vec![self.id("sync-none")], false)
            }
            Response::Sync { folders, .. } => (
                folders
                    .iter()
                    .map(|f| {
                        let id = if f.kept { "sync-kept" } else { "sync-not-kept" };
                        let mut line = self
                            .cat
                            .format(&crate::Message::new(id).arg("folder", f.path.clone()));
                        if f.chosen {
                            line.push(' ');
                            line.push_str(&self.id("sync-chosen"));
                        } else if let Some(owner) = &f.others {
                            line.push(' ');
                            line.push_str(&self.cat.format(
                                &crate::Message::new("sync-others").arg("owner", owner.clone()),
                            ));
                        }
                        line
                    })
                    .collect(),
                false,
            ),
            Response::Files { files, .. } if files.is_empty() => {
                (vec![self.id("files-none")], false)
            }
            Response::Files { files, .. } => (files.iter().map(|f| self.file(f)).collect(), false),
            Response::FreePreview(p) => (self.free_preview(p), false),
            Response::Deleted { files, .. } if files.is_empty() => {
                (vec![self.id("deleted-none")], false)
            }
            Response::Deleted { files, .. } => {
                (files.iter().map(|f| self.deleted(f)).collect(), false)
            }
            Response::Settings { settings } => (
                settings
                    .iter()
                    .map(|s| {
                        self.cat.format(
                            &crate::Message::new("setting-row")
                                .arg("key", s.key.clone())
                                .arg("value", s.value.clone()),
                        )
                    })
                    .collect(),
                false,
            ),
            Response::Text { text, .. } => (vec![text.clone()], false),
            Response::Done
            | Response::Bridge { .. }
            | Response::Hello { .. }
            | Response::Address { .. }
            | Response::Connected
            | Response::Listening => (Vec::new(), false),
        }
    }
}
