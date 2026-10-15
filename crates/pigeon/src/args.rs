//! The command line of the safety slice (13 §13.12): which request each
//! command sends to `pigeond`, or why it sends none. A hand parser, as the
//! slice's commands take few options (13 §13.12); every other command
//! answers `not-in-this-build`.

use std::path::Path;

use pigeon_proto::Request;

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Send this request to `pigeond`.
    Send(Request),
    /// Print the commands.
    Help,
    /// A command this build does not have yet.
    NotInThisBuild(String),
    /// The arguments do not fit this command.
    Usage(String),
}

/// A parsed command line.
#[derive(Debug, PartialEq, Eq)]
pub struct Line {
    /// What to do.
    pub action: Action,
    /// Print JSON responses rather than text.
    pub json: bool,
    /// `--yes`: a dangerous request needs no question.
    pub yes: bool,
    /// `--no-input`: never ask; a question fails with `needs-confirmation`.
    pub no_input: bool,
}

/// The commands of `pigeon help`: their synopsis and message id.
pub const COMMANDS: &[(&str, &str)] = &[
    (
        "create NAME [FOLDER | --folder FOLDER] [--k N] [--name NAME]",
        "cmd-create",
    ),
    ("invite [--admin] PERSON", "cmd-invite-person"),
    ("invite --backup-machine [NAME]", "cmd-invite"),
    ("join INVITATION [FOLDER]", "cmd-join"),
    ("status [PATH] [--not-shared]", "cmd-status"),
    ("machines", "cmd-machines"),
    ("history PATH", "cmd-history"),
    ("restore PATH [--version N | --at TIME]", "cmd-restore"),
    ("free PATH", "cmd-free"),
    ("keep PATH", "cmd-keep"),
    ("suggestions", "cmd-suggestions"),
    ("accept ID [--override REASON]", "cmd-accept"),
    ("remove-computer NAME [--yes]", "cmd-remove-computer"),
    ("recovery-sheet", "cmd-recovery-sheet"),
    ("recover SHEET [FOLDER] [--from NAME]", "cmd-recover"),
    ("doctor [--details]", "cmd-doctor"),
    ("debug bundle [--from NAME…]", "cmd-debug-bundle"),
];

/// Options that take a value.
const VALUED: &[&str] = &[
    "k", "name", "folder", "version", "at", "from", "override", "note",
];

#[derive(Debug, Default)]
struct Split {
    words: Vec<String>,
    flags: Vec<String>,
    values: Vec<(String, String)>,
}

impl Split {
    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    fn all(&self, name: &str) -> Vec<String> {
        self.values
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Whether every option given is one of `allowed`.
    fn only(&self, allowed: &[&str]) -> bool {
        self.flags
            .iter()
            .chain(self.values.iter().map(|(k, _)| k))
            .all(|f| allowed.contains(&f.as_str()))
    }
}

fn split(args: &[String]) -> Option<Split> {
    let mut out = Split::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            out.words.extend(it.by_ref().cloned());
        } else if let Some(opt) = a.strip_prefix("--") {
            match opt.split_once('=') {
                Some((k, v)) => out.values.push((k.to_owned(), v.to_owned())),
                None if VALUED.contains(&opt) => {
                    out.values.push((opt.to_owned(), it.next()?.clone()));
                }
                None => out.flags.push(opt.to_owned()),
            }
        } else if a == "-h" {
            out.flags.push("help".to_owned());
        } else {
            out.words.push(a.clone());
        }
    }
    Some(out)
}

/// `path` made absolute against the current directory, as `pigeond` runs
/// elsewhere.
pub fn absolute(path: &str) -> String {
    std::path::absolute(Path::new(path))
        .unwrap_or_else(|_| Path::new(path).to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "computer".to_owned())
}

/// Parses the arguments after the program's name.
pub fn parse(args: &[String]) -> Line {
    let Some(mut s) = split(args) else {
        let cmd = args.first().cloned().unwrap_or_default();
        return Line {
            action: Action::Usage(cmd),
            json: false,
            yes: false,
            no_input: false,
        };
    };
    let (json, yes, no_input) = (s.flag("json"), s.flag("yes"), s.flag("no-input"));
    s.flags
        .retain(|f| !matches!(f.as_str(), "json" | "yes" | "no-input"));
    let action = if s.flag("help") || s.words.is_empty() {
        Action::Help
    } else {
        let cmd = s.words.remove(0);
        command(&cmd, &s).unwrap_or(Action::Usage(cmd))
    };
    Line {
        action,
        json,
        yes,
        no_input,
    }
}

/// The action of `cmd`, `None` when its arguments do not fit.
fn command(cmd: &str, s: &Split) -> Option<Action> {
    let w = &s.words;
    let n = w.len();
    let fits = |lo: usize, hi: usize, allowed: &[&str]| (lo..=hi).contains(&n) && s.only(allowed);
    let send = |r: Request| Some(Action::Send(r));
    match cmd {
        "help" => Some(Action::Help),
        "create" if fits(1, 2, &["k", "name", "folder"]) => {
            let k = match s.value("k") {
                Some(v) => v.parse().ok().filter(|k| *k >= 1)?,
                None => 2,
            };
            send(Request::Create {
                name: w.first()?.clone(),
                folder: absolute(
                    s.value("folder")
                        .or(w.get(1).map(String::as_str))
                        .unwrap_or("."),
                ),
                k,
                device_name: s.value("name").map_or_else(hostname, str::to_owned),
            })
        }
        "invite" if fits(0, 1, &["backup-machine", "admin"]) => {
            if s.flag("backup-machine") {
                return send(Request::Invite {
                    person: String::new(),
                    admin: false,
                    custodian: true,
                    name: w.first().cloned(),
                });
            }
            send(Request::Invite {
                person: w.first()?.clone(),
                admin: s.flag("admin"),
                custodian: false,
                name: None,
            })
        }
        "join" if fits(1, 2, &["backup-machine", "keep"]) => send(Request::Join {
            invitation: w.first()?.clone(),
            folder: w.get(1).map(|f| absolute(f)),
            keep: s.flag("keep"),
        }),
        "status" if fits(0, 1, &["not-shared"]) => send(Request::Status {
            path: w.first().map(|p| absolute(p)),
            not_shared: s.flag("not-shared"),
        }),
        "machines" if fits(0, 0, &[]) => send(Request::Machines),
        "suggestions" if fits(0, 0, &[]) => send(Request::Proposals),
        "accept" if fits(1, 1, &["override"]) => send(Request::Accept {
            proposal: w.first()?.clone(),
            reason: s.value("override").map(str::to_owned),
        }),
        "history" if fits(1, 1, &[]) => send(Request::History {
            path: absolute(w.first()?),
        }),
        "restore" if fits(1, 1, &["version", "at"]) => {
            let version = match s.value("version") {
                Some(v) => Some(v.parse().ok()?),
                None => None,
            };
            send(Request::Restore {
                path: absolute(w.first()?),
                version,
                at: s.value("at").map(str::to_owned),
            })
        }
        "free" if fits(1, 1, &[]) => send(Request::Evict {
            folder: absolute(w.first()?),
        }),
        "keep" if fits(1, 1, &[]) => send(Request::Keep {
            folder: absolute(w.first()?),
        }),
        "remove-computer" if fits(1, 1, &[]) => send(Request::RemoveDevice {
            name: w.first()?.clone(),
        }),
        "recovery-sheet" if fits(0, 0, &[]) => send(Request::RecoverySheet),
        "recover" if fits(1, 2, &["from"]) => {
            let sheet = w.first()?;
            let text = std::fs::read_to_string(sheet).unwrap_or_else(|_| sheet.clone());
            send(Request::Recover {
                sheet: text,
                folder: w.get(1).map(|f| absolute(f)),
                from: s.value("from").map(str::to_owned),
            })
        }
        "doctor" if fits(0, 0, &["details"]) => send(Request::Doctor {
            details: s.flag("details"),
        }),
        "debug" if n == 1 && w.first().is_some_and(|b| b == "bundle") && s.only(&["from"]) => {
            send(Request::DebugBundle {
                peers: s.all("from"),
            })
        }
        "create" | "invite" | "join" | "status" | "machines" | "history" | "restore" | "free"
        | "keep" | "remove-computer" | "recovery-sheet" | "recover" | "doctor" | "debug"
        | "suggestions" | "accept" => None,
        other => Some(Action::NotInThisBuild(other.to_owned())),
    }
}
