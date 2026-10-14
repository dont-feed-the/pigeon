//! Turns `pigeond`'s responses into text with the Fluent catalogue (ADR
//! 0028, 10 §10.5–10.6): the daemon sends message ids and arguments, times in RFC
//! 3339, and this module renders them in the person's language, with
//! times relative to now.

use std::io::Write;

use pigeon_i18n::{Catalogue, ids};
use pigeon_proto::api::{Machine, State, StatusReport, VersionStatus};
use pigeon_proto::{Arg, Message, Problem, Response};

/// The arguments that hold a past time, shown relative to now.
const PAST: &[&str] = &["ago", "when", "since", "heard", "saved"];

/// Renders responses for a terminal.
pub struct Renderer {
    cat: Catalogue,
    now: i64,
}

impl Renderer {
    /// The renderer of the person's language at time `now`.
    pub fn new(now: i64) -> Renderer {
        let system = pigeon_i18n::system_languages(|k| std::env::var(k).ok());
        let lang = pigeon_i18n::choose(&pigeon_i18n::Setting::Auto, &system);
        Renderer {
            cat: Catalogue::new(&lang, false),
            now,
        }
    }

    /// `secs` seconds ago, in words.
    pub fn ago(&self, secs: i64) -> String {
        let secs = secs.max(0);
        let m = if secs < 60 {
            pigeon_i18n::Message::new("time-just-now")
        } else if secs < 3600 {
            pigeon_i18n::Message::new("time-minutes-ago").arg("minutes", secs / 60)
        } else if secs < 86_400 {
            pigeon_i18n::Message::new("time-hours-ago").arg("hours", secs / 3600)
        } else {
            pigeon_i18n::Message::new("time-days-ago").arg("days", secs / 86_400)
        };
        self.cat.format(&m)
    }

    /// The text of `m`.
    pub fn text(&self, m: &Message) -> String {
        let mut out = pigeon_i18n::Message::new(m.id.clone());
        for (name, value) in &m.args {
            let v = match value {
                Arg::Number(n) => pigeon_i18n::Arg::Number(*n),
                Arg::Text(t) if PAST.contains(&name.as_str()) => {
                    match pigeon_proto::time::parse_rfc3339(t) {
                        Some(at) => pigeon_i18n::Arg::Text(self.ago(self.now - at)),
                        None => pigeon_i18n::Arg::Text(t.clone()),
                    }
                }
                Arg::Text(t) => pigeon_i18n::Arg::Text(t.clone()),
            };
            out.args.insert(name.clone(), v);
        }
        self.cat.format(&out)
    }

    fn id(&self, id: &str) -> String {
        self.cat.format(&pigeon_i18n::Message::new(id))
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
        let code =
            pigeon_i18n::Message::new("problem-code").arg("code", ids::display_code(&p.code));
        out.push(self.cat.format(&code));
        out
    }

    fn status(&self, r: &StatusReport) -> Vec<String> {
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

    fn waiting(&self, v: &VersionStatus) -> String {
        let m = pigeon_i18n::Message::new("waiting-row")
            .arg("path", v.path.as_str())
            .arg("state", self.state(v.state));
        self.cat.format(&m)
    }

    fn machine(&self, m: &Machine) -> String {
        if m.this_device {
            let msg = pigeon_i18n::Message::new("machine-this").arg("name", m.name.as_str());
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
        let msg = pigeon_i18n::Message::new("machine-row")
            .arg("name", m.name.as_str())
            .arg("kind", self.id(kind))
            .arg("heard", heard);
        self.cat.format(&msg)
    }

    fn version(&self, v: &VersionStatus) -> String {
        let saved = pigeon_proto::time::parse_rfc3339(&v.saved_at)
            .map_or_else(|| v.saved_at.clone(), |t| self.ago(self.now - t));
        let msg = pigeon_i18n::Message::new("version-row")
            .arg("number", v.number.to_string())
            .arg("saved", saved)
            .arg("state", self.state(v.state));
        self.cat.format(&msg)
    }

    /// The lines of a response, and whether they go to standard error.
    pub fn response(&self, r: &Response) -> (Vec<String>, bool) {
        match r {
            Response::Said { message } => (vec![self.text(message)], false),
            Response::Progress { message, .. } => (vec![self.text(message)], true),
            Response::Problem(p) => (self.problem(p), true),
            Response::Status(s) => (self.status(s), false),
            Response::Machines { machines } => {
                (machines.iter().map(|m| self.machine(m)).collect(), false)
            }
            Response::History { versions } => {
                (versions.iter().map(|v| self.version(v)).collect(), false)
            }
            Response::Done
            | Response::Hello { .. }
            | Response::Address { .. }
            | Response::Connected
            | Response::Listening => (Vec::new(), false),
        }
    }

    /// `pigeon help`.
    pub fn help(&self) -> Vec<String> {
        let mut out = vec![
            self.id("help-usage"),
            String::new(),
            self.id("help-commands"),
        ];
        for (synopsis, id) in crate::args::COMMANDS {
            out.push(format!("  {synopsis}"));
            out.push(format!("      {}", self.id(id)));
        }
        out.push(format!("  --json  {}", self.id("opt-json")));
        out
    }
}

/// Writes `lines` to standard output, or standard error when `err`.
pub fn emit(lines: &[String], err: bool) {
    let write = |w: &mut dyn Write| {
        for l in lines {
            let _ = writeln!(w, "{l}");
        }
    };
    if err {
        write(&mut std::io::stderr().lock());
    } else {
        write(&mut std::io::stdout().lock());
    }
}
