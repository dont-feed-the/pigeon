//! The pigeon command-line interface, a client of pigeond's socket (10
//! §10.2). It sends one request, prints the responses as they come, and
//! exits 0 when done, 1 on a problem and 2 when the arguments do not fit.
//! Text comes from the Fluent catalogue (ADR 0028); `--json` prints each
//! response as JSON instead. `pigeon update` hands over to the installer
//! that put this program here (14 §14.5); `pigeon export` reads the
//! repositories itself (09 §9.9).

mod args;
mod export;
mod render;
mod repo;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;

use pigeon_proto::{
    Envelope, LineDecoder, Message, PROTOCOL_VERSION, Problem, Request, Response, encode,
};

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
    fn connect() -> Option<Client> {
        let stream = UnixStream::connect(pigeon_sys::paths::client_socket()).ok()?;
        Some(Client {
            stream,
            decoder: LineDecoder::new(),
        })
    }

    fn send(&mut self, id: u64, r: &Request) -> std::io::Result<()> {
        let line = encode(id, r).map_err(|e| std::io::Error::other(e.to_string()))?;
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
    let fail = |p: Problem| {
        show(out, json, &Response::Problem(p));
        ExitCode::from(1)
    };
    let Some(mut client) = Client::connect() else {
        return fail(local_problem("daemon-not-running", &[]));
    };
    let hello = Request::Hello {
        version: PROTOCOL_VERSION,
    };
    let answered = client.send(1, &hello).and_then(|()| client.next());
    match answered {
        Ok(Envelope {
            body: Response::Hello { version },
            ..
        }) if version == PROTOCOL_VERSION => {}
        Ok(_) => return fail(local_problem("version-mismatch", &[])),
        Err(_) => return fail(local_problem("daemon-not-running", &[])),
    }
    if client.send(2, request).is_err() {
        return fail(local_problem("daemon-not-running", &[]));
    }
    loop {
        let Ok(env) = client.next() else {
            return fail(local_problem("daemon-not-running", &[]));
        };
        let (last, problem) = show(out, json, &env.body);
        if last {
            return ExitCode::from(u8::from(problem));
        }
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let line = args::parse(&argv);
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
            emit(&[format!("pigeon {}", env!("CARGO_PKG_VERSION"))], false);
            ExitCode::SUCCESS
        }
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
        Action::Send(request) => {
            if let Some(question) = question(request)
                && !line.yes
                && !confirm(&out, &line, &question)
            {
                let p = local_problem("needs-confirmation", &[]);
                show(&out, line.json, &Response::Problem(p));
                return ExitCode::from(1);
            }
            exchange(&out, line.json, request)
        }
    }
}

/// Replaces this process with `pigeon-install`, which a release puts beside
/// `pigeon` in pigeon's `bin/`; returns only if it cannot run.
fn update(args: &[String]) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    let installer = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map(|exe| exe.with_file_name("pigeon-install"));
    match installer {
        Ok(at) if at.is_file() => std::process::Command::new(at).args(args).exec(),
        Ok(at) => std::io::Error::new(std::io::ErrorKind::NotFound, at.display().to_string()),
        Err(e) => e,
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
        if !confirm(out, line, &question) {
            let p = local_problem("needs-confirmation", &[]);
            show(out, line.json, &Response::Problem(p));
            return ExitCode::from(1);
        }
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
        Request::RemoveDevice { name } => Some(ask("confirm-remove-computer", &[("name", name)])),
        Request::Leave => Some(ask("confirm-leave", &[])),
        Request::Join { invitation, .. } => {
            let (group, r) = pigeon_proto::join::refounded(invitation)?;
            let date = r.at.get(..10).unwrap_or(&r.at);
            Some(ask(
                "confirm-join-refounded",
                &[("group", &group), ("name", &r.by), ("date", date)],
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

/// Asks `question`, on a terminal only (10 §10.2).
fn confirm(out: &Renderer, line: &args::Line, question: &Message) -> bool {
    use std::io::IsTerminal;
    if line.no_input || !std::io::stdin().is_terminal() {
        return false;
    }
    let _ = write!(std::io::stderr(), "{} ", out.text(question));
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    let yes = out.text(&Message {
        id: "confirm-yes".to_owned(),
        args: std::collections::BTreeMap::new(),
    });
    let a = answer.trim().to_lowercase();
    !a.is_empty() && (yes.to_lowercase().starts_with(&a) || a == "y" || a == "yes")
}

#[cfg(test)]
mod tests;
