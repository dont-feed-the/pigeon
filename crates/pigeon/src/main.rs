//! The pigeon command-line interface, a client of pigeond's socket (10
//! §10.2). It sends one request, prints the responses as they come, and
//! exits 0 when done, 1 on a problem and 2 when the arguments do not fit.
//! Text comes from the Fluent catalogue (ADR 0028); `--json` prints each
//! response as JSON instead. `pigeon update` hands over to the installer
//! that put this program here (14 §14.5); `pigeon export` reads the
//! repositories itself (09 §9.9). Run with `COMPLETE` set to a shell's
//! name, it answers that shell's Tab completion instead.

mod args;
mod complete;
mod export;
mod join;
mod render;
mod repo;
mod startup;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::time::Duration;

use pigeon_proto::api::Removal;
use pigeon_proto::{Envelope, LineDecoder, Message, PROTOCOL_VERSION, Problem, Request, Response};

use crate::args::Action;
use crate::render::{Renderer, emit};

/// A problem the CLI finds itself, before or without `pigeond`.
fn local_problem(code: &str, args: &[(&str, &str)]) -> Problem {
    Problem {
        code: code.to_owned(),
        message: Message {
            id: pigeon_i18n::ids::problem_id(code),
            args: args
                .iter()
                .map(|(k, v)| ((*k).to_owned(), pigeon_proto::Arg::Text((*v).to_owned())))
                .collect(),
        },
        hint: None,
        details: None,
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// One connection to `pigeond`.
struct Client {
    stream: UnixStream,
    decoder: LineDecoder,
}

impl Client {
    /// Connects and greets `pigeond`, each read waiting at most `patience`;
    /// the problem's code otherwise.
    fn open(patience: Option<Duration>) -> Result<Client, &'static str> {
        let stream = UnixStream::connect(pigeon_sys::paths::client_socket())
            .map_err(|_| "daemon-not-running")?;
        Self::greet(stream, patience)
    }

    fn greet(stream: UnixStream, patience: Option<Duration>) -> Result<Client, &'static str> {
        stream
            .set_read_timeout(patience)
            .map_err(|_| "daemon-not-running")?;
        let mut client = Client {
            stream,
            decoder: LineDecoder::new(),
        };
        let hello = Request::Hello {
            version: PROTOCOL_VERSION,
        };
        match client.send(1, &hello).and_then(|()| client.next()) {
            Ok(Envelope {
                body: Response::Hello { version },
                ..
            }) if version == PROTOCOL_VERSION => Ok(client),
            Ok(_) => Err("version-mismatch"),
            Err(_) => Err("daemon-not-running"),
        }
    }

    fn send(&mut self, id: u64, r: &Request) -> std::io::Result<()> {
        let line = pigeon_proto::wire::encode_with_context(id, r, pigeon_telemetry::traceparent())
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        self.stream.write_all(&line)
    }

    fn next(&mut self) -> std::io::Result<Envelope<Response>> {
        let mut buf = [0u8; 16 * 1024];
        loop {
            match self.decoder.next_line::<Response>() {
                Ok(Some(e)) => return Ok(e),
                Ok(None) => {}
                Err(e) => return Err(std::io::Error::other(e.to_string())),
            }
            let n = self.stream.read(&mut buf)?;
            if n == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            self.decoder.push(buf.get(..n).unwrap_or_default());
        }
    }
}

/// Opens `url` in the person's browser, for `pigeon open` (10 §10.3), or a
/// file with its default application, for `pigeon view`; the link or path
/// is printed anyway, for a computer without one.
fn open_browser(url: &str) {
    let opener = if cfg!(target_vendor = "apple") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Prints `r`; whether it ends the request, and whether it is a problem.
fn show(out: &Renderer, json: bool, r: &Response) -> (bool, bool) {
    if !json
        && let Response::Said { message } = r
        && let Some(pigeon_proto::Arg::Text(url)) = match message.id.as_str() {
            "web-login" => message.args.get("url"),
            "view-ready" => message.args.get("path"),
            _ => None,
        }
    {
        open_browser(url);
    }
    let last = !matches!(r, Response::Progress { .. });
    let problem = matches!(r, Response::Problem(_));
    if json {
        // The result on stdout, problems and progress on stderr (10 §10.2).
        let line = serde_json::to_string(r).unwrap_or_default();
        emit(&[line], !last || problem);
    } else {
        let (lines, err) = out.response(r);
        emit(&lines, err);
    }
    (last, problem)
}

fn exchange(out: &Renderer, json: bool, request: &Request) -> ExitCode {
    let (_, problem) = show(out, json, &answer(out, json, request));
    ExitCode::from(u8::from(problem))
}

/// [`exchange`], and the code of the daemon's problem it ended with.
fn exchange_coded(out: &Renderer, json: bool, request: &Request) -> (ExitCode, Option<String>) {
    let last = answer(out, json, request);
    let code = match &last {
        Response::Problem(p) => Some(p.code.clone()),
        _ => None,
    };
    let (_, problem) = show(out, json, &last);
    (ExitCode::from(u8::from(problem)), code)
}

/// Sends `request` and shows its progress: answers its last response,
/// unshown, or the problem of a daemon that cannot be reached.
fn answer(out: &Renderer, json: bool, request: &Request) -> Response {
    sent(out, json, request).unwrap_or_else(|p| Response::Problem(*p))
}

fn sent(out: &Renderer, json: bool, request: &Request) -> Result<Response, Box<Problem>> {
    let unreachable = |code: &str| Box::new(local_problem(code, &[]));
    let mut client = Client::open(None)
        .or_else(|code| {
            if code == "daemon-not-running"
                && matches!(request, Request::OpenWeb | Request::Join { .. })
            {
                crate::startup::connect()
                    .ok_or(code)
                    .and_then(|stream| Client::greet(stream, None))
            } else {
                Err(code)
            }
        })
        .map_err(unreachable)?;
    if client.send(2, request).is_err() {
        return Err(unreachable("daemon-not-running"));
    }
    let waiting = match request {
        Request::Restore { path, .. } => Some(("progress-restore", vec![("path", path.as_str())])),
        Request::RemoveDevice { name } => Some(("progress-remove", vec![("name", name.as_str())])),
        Request::RecoverySheet => Some(("progress-recovery-sheet", Vec::new())),
        Request::DebugBundle { .. } => Some(("progress-debug-bundle", Vec::new())),
        Request::Recover { .. } => Some(("progress-recover", Vec::new())),
        Request::Join { .. } => Some(("progress-join", Vec::new())),
        _ => None,
    };
    if let Some((id, args)) = waiting {
        let message = Message {
            id: id.to_owned(),
            args: args
                .into_iter()
                .map(|(k, v)| (k.to_owned(), pigeon_proto::Arg::Text(v.to_owned())))
                .collect(),
        };
        show(
            out,
            json,
            &Response::Progress {
                message,
                done: None,
                total: None,
            },
        );
    }
    loop {
        let env = client
            .next()
            .map_err(|_| unreachable("daemon-not-running"))?;
        if !matches!(env.body, Response::Progress { .. }) {
            return Ok(env.body);
        }
        show(out, json, &env.body);
    }
}

fn main() -> ExitCode {
    complete::answer();
    let result = run();
    pigeon_telemetry::flush();
    result
}

fn run() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let line = args::parse(&argv);
    let _span = matches!(line.action, Action::Send(_)).then(|| {
        if let Ok(dirs) = pigeon_sys::paths::Dirs::for_role(pigeon_sys::paths::Role::Person) {
            pigeon_telemetry::init(dirs.state.join("telemetry"), "pigeon", "local-cli");
        }
        pigeon_telemetry::span("cli.request")
    });
    let _context = _span.as_ref().map(pigeon_telemetry::Span::enter);
    let out = render::for_person(now());
    match &line.action {
        Action::Help => {
            emit(&render::help(&out), false);
            ExitCode::SUCCESS
        }
        Action::Usage(cmd) => {
            let p = local_problem("usage", &[("command", cmd)]);
            show(&out, line.json, &Response::Problem(p));
            ExitCode::from(2)
        }
        Action::Version => {
            // A build of main names its commit (14 §14.5).
            let version = match option_env!("PIGEON_COMMIT") {
                Some(commit) => format!("pigeon {}+g{commit}", env!("CARGO_PKG_VERSION")),
                None => format!("pigeon {}", env!("CARGO_PKG_VERSION")),
            };
            emit(&[version], false);
            ExitCode::SUCCESS
        }
        Action::Completions(shell) => match complete::register(*shell) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => {
                let p = local_problem("internal-error", &[]);
                show(&out, line.json, &Response::Problem(p));
                ExitCode::from(1)
            }
        },
        Action::Update(args) => {
            let code = match update(args).kind() {
                std::io::ErrorKind::NotFound => "not-installed",
                _ => "internal-error",
            };
            let p = local_problem(code, &[]);
            show(&out, line.json, &Response::Problem(p));
            ExitCode::from(1)
        }
        Action::Export(e) => match export::run(e) {
            Ok(messages) => {
                for message in messages {
                    show(&out, line.json, &Response::Said { message });
                }
                ExitCode::SUCCESS
            }
            Err(p) => {
                show(&out, line.json, &Response::Problem(*p));
                ExitCode::from(1)
            }
        },
        Action::NotInThisBuild(cmd) => {
            let p = local_problem("not-in-this-build", &[("command", cmd)]);
            show(&out, line.json, &Response::Problem(p));
            ExitCode::from(1)
        }
        Action::Send(Request::Drop { path, .. }) => drop(&out, &line, path),
        Action::Send(Request::Evict { folder, .. }) => free(&out, &line, folder),
        Action::Send(Request::SetFolderRule { folder, rule, .. }) => {
            folder_rule(&out, &line, folder, rule)
        }
        Action::Send(request) => {
            if !line.yes
                && let Some(question) = question(request)
                && let Err(code) = confirmed(&out, &line, &question)
            {
                return code;
            }
            match request {
                Request::Join { .. } => join::run(&out, &line, request),
                _ => exchange(&out, line.json, request),
            }
        }
    }
}

/// Replaces this process with `pigeon-install`, which a release puts beside
/// `pigeon` in pigeon's `bin/`, on the installation this program is part
/// of; returns only if it cannot run.
fn update(args: &[String]) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    let exe = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(exe) => exe,
        Err(e) => return e,
    };
    let installer = exe.with_file_name("pigeon-install");
    match exe.parent().and_then(std::path::Path::parent) {
        Some(prefix) if installer.is_file() => std::process::Command::new(installer)
            .arg("--prefix")
            .arg(prefix)
            .args(args)
            .exec(),
        _ => std::io::Error::new(
            std::io::ErrorKind::NotFound,
            installer.display().to_string(),
        ),
    }
}

/// `pigeon free --without-checking PATH` (08 §8.7): shows the risk, asks,
/// then drops; with `--yes`, drops at once and shows the risk with it.
fn drop(out: &Renderer, line: &args::Line, path: &str) -> ExitCode {
    let send = |confirmed| Request::Drop {
        path: path.to_owned(),
        confirmed,
    };
    if !line.yes {
        let shown = exchange(out, line.json, &send(false));
        if shown != ExitCode::SUCCESS {
            return shown;
        }
        let question = Message {
            id: "confirm-drop".to_owned(),
            args: std::collections::BTreeMap::new(),
        };
        if let Err(code) = confirmed(out, line, &question) {
            return code;
        }
    }
    exchange(out, line.json, &send(true))
}

/// `pigeon free PATH` (08 §8.5): while no backup machine has been heard
/// from recently, the daemon says so (`backups-quiet`), and pigeon asks
/// before it frees; with `--yes`, it frees at once.
fn free(out: &Renderer, line: &args::Line, folder: &str) -> ExitCode {
    let send = |confirmed| Request::Evict {
        folder: folder.to_owned(),
        confirmed,
    };
    let (code, problem) = exchange_coded(out, line.json, &send(line.yes));
    if problem.as_deref() != Some("backups-quiet") {
        return code;
    }
    let question = Message {
        id: "confirm-free-quiet".to_owned(),
        args: std::collections::BTreeMap::new(),
    };
    if let Err(code) = confirmed(out, line, &question) {
        return code;
    }
    exchange(out, line.json, &send(true))
}

/// `pigeon folder-rule PATH …` (04 §4.3): a rule that raises the group's
/// rules (ADR 0048) first says which computers may wait for changes, then
/// asks; `--yes` confirms at once.
fn folder_rule(
    out: &Renderer,
    line: &args::Line,
    folder: &str,
    rule: &pigeon_proto::policy::Change,
) -> ExitCode {
    let send = |confirmed| Request::SetFolderRule {
        folder: folder.to_owned(),
        rule: rule.clone(),
        confirmed,
    };
    let first = answer(out, line.json, &send(line.yes));
    let Response::Problem(raise) = &first else {
        return ExitCode::from(u8::from(show(out, line.json, &first).1));
    };
    if raise.code != "rules-raise" || line.yes {
        return ExitCode::from(u8::from(show(out, line.json, &first).1));
    }
    show(
        out,
        line.json,
        &Response::Said {
            message: raise.message.clone(),
        },
    );
    let question = Message {
        id: "confirm-rules-raise".to_owned(),
        args: std::collections::BTreeMap::new(),
    };
    if let Err(code) = confirmed(out, line, &question) {
        return code;
    }
    exchange(out, line.json, &send(true))
}

/// What a request asks before it is sent, if anything (10 §10.2).
fn question(request: &Request) -> Option<Message> {
    let ask = |id: &str, args: &[(&str, &str)]| Message {
        id: id.to_owned(),
        args: args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), pigeon_proto::Arg::Text((*v).to_owned())))
            .collect(),
    };
    match request {
        Request::DebugBundle { .. } => Some(ask("confirm-debug-bundle", &[])),
        Request::AllowDebug { peer } => Some(ask("confirm-debug-allow", &[("name", peer)])),
        Request::RemoveDevice { name } => Some(match removal(name) {
            Removal::Sheet => ask("confirm-revoke-sheet", &[("name", name)]),
            Removal::Computer => ask("confirm-remove-computer", &[("name", name)]),
            Removal::Backup { left, needed } if left < needed => {
                let mut question = ask("confirm-remove-backup-unsafe", &[("name", name)]);
                for (k, n) in [("left", left), ("needed", needed)] {
                    let n = pigeon_proto::Arg::Number(i64::from(n));
                    question.args.insert(k.to_owned(), n);
                }
                question
            }
            Removal::Backup { .. } => ask("confirm-remove-backup", &[("name", name)]),
        }),
        Request::Leave => Some(ask("confirm-leave", &[])),
        Request::Join { invitation, .. } => {
            let (group, r) = pigeon_proto::join::refounded(invitation)?;
            // The renderer words the time as a local date.
            Some(ask(
                "confirm-join-refounded",
                &[("group", &group), ("name", &r.by), ("date", &r.at)],
            ))
        }
        Request::Refound { .. } => Some(ask("confirm-refound", &[])),
        Request::ReleaseHeld {
            batch,
            choice: pigeon_proto::api::HeldChoice::Share,
        } => Some(ask("confirm-held-share", &[("batch", batch)])),
        _ => None,
    }
}

/// Reads the device's role, and the backup machines that would be left,
/// before wording a removal confirmation (09 §9.7). A failed lookup
/// retains the ordinary computer-removal confirmation.
fn removal(name: &str) -> Removal {
    let Ok(mut client) = Client::open(None) else {
        return Removal::Computer;
    };
    if client.send(2, &Request::Machines).is_err() {
        return Removal::Computer;
    }
    while let Ok(answer) = client.next() {
        match answer.body {
            Response::Progress { .. } => {}
            other => return Removal::of(&[other], name),
        }
    }
    Removal::Computer
}

/// Asks `question`, on a terminal only (10 §10.2).
fn confirmed(out: &Renderer, line: &args::Line, question: &Message) -> Result<(), ExitCode> {
    match confirmation(out, line, question) {
        Some(true) => Ok(()),
        Some(false) => {
            show(
                out,
                line.json,
                &Response::Said {
                    message: Message {
                        id: "action-cancelled".into(),
                        args: Default::default(),
                    },
                },
            );
            Err(ExitCode::SUCCESS)
        }
        None => {
            show(
                out,
                line.json,
                &Response::Problem(local_problem("needs-confirmation", &[])),
            );
            Err(ExitCode::from(1))
        }
    }
}

fn confirmation(out: &Renderer, line: &args::Line, question: &Message) -> Option<bool> {
    use std::io::IsTerminal;
    if line.no_input || !std::io::stdin().is_terminal() {
        return None;
    }
    let _ = write!(std::io::stderr(), "{} ", out.text(question));
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return None;
    }
    let yes = out.text(&Message {
        id: "confirm-yes".to_owned(),
        args: std::collections::BTreeMap::new(),
    });
    let a = answer.trim().to_lowercase();
    Some(!a.is_empty() && (yes.to_lowercase().starts_with(&a) || a == "y" || a == "yes"))
}

#[cfg(test)]
mod tests;
