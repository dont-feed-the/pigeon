//! `pigeon join` (09 §9.6, ADR 0075): on a terminal, asks what the command
//! line did not give, the folder (the group's name in the home folder by
//! default; none on a backup machine) and the person's name (the account's
//! by default); then, when the group has a person of that name, whether this
//! computer is theirs. `--yes` answers yes, `--no-input` asks nothing.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use pigeon_proto::{Arg, Message, Request, Response};

use crate::args::{Line, absolute};
use crate::render::Renderer;

/// Asks, sends the join, and asks again if its name is a person's.
pub fn run(out: &Renderer, line: &Line, request: &Request) -> ExitCode {
    let Request::Join {
        invitation,
        folder,
        keep,
        name,
        person,
        existing,
        custodian,
    } = request.clone()
    else {
        return crate::exchange(out, line.json, request);
    };
    let group = pigeon_proto::join::decode(&pigeon_proto::join::invitation(&invitation))
        .map(|i| i.name)
        .unwrap_or_default();
    let asking = !line.no_input && std::io::stdin().is_terminal();
    let folder = match folder {
        None if asking && !custodian && !group.is_empty() => {
            let home = std::env::var("HOME").unwrap_or_default();
            let default = PathBuf::from(&home).join(&group);
            let shown = default.display().to_string();
            ask(
                out,
                "ask-join-folder",
                &[("group", &group), ("folder", &shown)],
            )
            .map(|typed| absolute(&expand(&typed, &home)))
        }
        given => given,
    };
    let person = match person {
        None if asking => ask(out, "ask-join-person", &[("person", &account())]),
        given => given,
    };
    let mut request = Request::Join {
        invitation,
        folder,
        keep,
        name,
        person: person.clone(),
        existing: existing || line.yes,
        custodian,
    };
    let first = match crate::answer(out, line.json, &request) {
        Response::Problem(p) if p.code == "person-exists" => {
            let named = match p.message.args.get("person") {
                Some(Arg::Text(t)) => t.clone(),
                _ => person.unwrap_or_else(account),
            };
            let question = message(
                "confirm-join-existing",
                &[("person", &named), ("group", &group)],
            );
            match crate::confirmation(out, line, &question) {
                Some(true) => None,
                Some(false) => Some(Response::Said {
                    message: message("action-cancelled", &[]),
                }),
                None => Some(Response::Problem(p)),
            }
        }
        r => Some(r),
    };
    let last = match first {
        Some(r) => r,
        None => {
            if let Request::Join { existing, .. } = &mut request {
                *existing = true;
            }
            crate::answer(out, line.json, &request)
        }
    };
    let (_, problem) = crate::show(out, line.json, &last);
    ExitCode::from(u8::from(problem))
}

fn message(id: &str, args: &[(&str, &str)]) -> Message {
    Message {
        id: id.to_owned(),
        args: args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), Arg::Text((*v).to_owned())))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// Asks `id` on the terminal: the answer, or `None` for the default.
fn ask(out: &Renderer, id: &str, args: &[(&str, &str)]) -> Option<String> {
    let _ = write!(std::io::stderr(), "{} ", out.text(&message(id, args)));
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok()?;
    Some(answer.trim().to_owned()).filter(|a| !a.is_empty())
}

/// `~` or `~/…` as the home folder, which no shell expanded.
fn expand(typed: &str, home: &str) -> String {
    match typed.strip_prefix('~') {
        Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
            format!("{}{rest}", home.trim_end_matches('/'))
        }
        _ => typed.to_owned(),
    }
}

/// The name of this computer's account, as the daemon gives a person
/// who gives none.
fn account() -> String {
    std::env::var("USER")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "Me".to_owned())
}
