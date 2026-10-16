//! Turns `pigeond`'s responses into text with the Fluent catalogue (ADR
//! 0028, 10 §10.5–10.6), for the CLI and the web interface: the daemon
//! sends message ids and arguments, times in RFC 3339, and this module
//! renders them in the person's language, with times relative to now.

use crate::{Catalogue, ids};
use pigeon_proto::api::{Machine, State, StatusReport, VersionStatus};
use pigeon_proto::{Arg, Message, Problem, Response};

/// The arguments that hold a past time, shown relative to now.
const PAST: &[&str] = &["ago", "when", "since", "heard", "saved"];

/// Renders responses as lines of text, for the CLI and the web pages.
pub struct Renderer {
    cat: Catalogue,
    now: i64,
}

impl Renderer {
    /// The renderer of `cat`'s language at time `now`, in Unix seconds.
    pub fn new(cat: Catalogue, now: i64) -> Renderer {
        Renderer { cat, now }
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
        let msg = crate::Message::new("held-row")
            .arg("id", h.id.as_str())
            .arg("reason", self.text(&h.reason).as_str())
            .arg("files", files)
            .arg("since", h.since.as_str())
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
            return self
                .cat
                .format(&crate::Message::new("machine-recovery").arg("name", m.name.as_str()));
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
            Response::Status(s) => (self.status(s), false),
            Response::Capacity(report) => (crate::capacity::lines(self, report), false),
            Response::Health(h) => (self.status(&h.status), false),
            Response::Machines { machines } => {
                (machines.iter().map(|m| self.machine(m)).collect(), false)
            }
            Response::Held { held } if held.is_empty() => (vec![self.id("held-none")], false),
            Response::Held { held } => (held.iter().map(|h| self.held(h)).collect(), false),
            Response::People { people } => (people.iter().map(|p| self.person(p)).collect(), false),
            Response::History { versions } => {
                (versions.iter().map(|v| self.version(v)).collect(), false)
            }
            Response::Proposals { proposals } if proposals.is_empty() => {
                (vec![self.id("suggestions-none")], false)
            }
            Response::Proposals { proposals } => (
                proposals.iter().map(|p| self.suggestion(p)).collect(),
                false,
            ),
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
            Response::Files { files, .. } => (
                files
                    .iter()
                    .map(|f| {
                        self.cat.format(
                            &crate::Message::new("file-row")
                                .arg("path", f.path.clone())
                                .arg("state", self.state(f.state)),
                        )
                    })
                    .collect(),
                false,
            ),
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
