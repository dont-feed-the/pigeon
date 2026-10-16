//! Tab completion (10 §10.2, ADR 0024): the shell runs `pigeon` with
//! `COMPLETE` set to its name, and the answer comes from the command line's
//! definition, with each command's text in the person's language. The
//! values `pigeond` lists complete too: suggestions, held batches,
//! invitations, the group's other machines, and settings with their choices.

use std::ffi::OsString;
use std::time::Duration;

use clap_complete::engine::{ArgValueCandidates, CompletionCandidate};
use clap_complete::env::CompleteEnv;
use pigeon_proto::{Request, Response};

use crate::render::Renderer;

/// How long a completion waits for `pigeond`: a busy daemon gives no
/// candidates rather than a stuck shell.
const PATIENCE: Duration = Duration::from_millis(500);

/// Prints the completions and exits when the shell asks for them;
/// otherwise returns.
pub fn answer() {
    CompleteEnv::with_factory(described).complete();
}

/// The command line's definition, each command's and option's help taken
/// from the catalogue where it has one.
pub fn described() -> clap::Command {
    let r = crate::render::for_person(crate::now());
    let option = |a: clap::Arg| match a.get_long().map(|l| format!("opt-{l}")) {
        Some(id) if r.catalogue().has(&id) => a.help(r.id(&id)),
        _ => a,
    };
    crate::args::command()
        .mut_args(option)
        .mut_subcommands(|c| {
            let about = crate::args::COMMANDS
                .iter()
                .find(|(synopsis, _)| synopsis.split(' ').next() == Some(c.get_name()))
                .map(|(_, id)| r.id(id));
            let c = c.mut_args(option);
            match about {
                Some(about) => c.about(about),
                None => c,
            }
        })
}

/// What `pigeond` answers to `request`, if it answers in time.
fn ask(request: &Request) -> Option<Response> {
    let mut client = crate::Client::open(Some(PATIENCE)).ok()?;
    client.send(2, request).ok()?;
    loop {
        let body = client.next().ok()?.body;
        if !matches!(body, Response::Progress { .. }) {
            return Some(body);
        }
    }
}

/// The candidates `pick` finds in what `pigeond` answers to `request`.
fn listed(
    request: Request,
    pick: fn(&Renderer, Response) -> Vec<CompletionCandidate>,
) -> ArgValueCandidates {
    ArgValueCandidates::new(move || {
        let r = crate::render::for_person(crate::now());
        ask(&request).map_or_else(Vec::new, |answer| pick(&r, answer))
    })
}

fn candidate(value: &str, help: String) -> CompletionCandidate {
    CompletionCandidate::new(value).help(Some(help.into()))
}

/// The suggestions waiting for a decision, for `accept` and `decline`.
pub fn suggestions() -> ArgValueCandidates {
    listed(Request::Proposals, |r, answer| match answer {
        Response::Proposals { proposals, .. } => proposals
            .iter()
            .map(|p| candidate(&p.id, r.suggestion(p)))
            .collect(),
        _ => Vec::new(),
    })
}

/// The held batches, for `held ID`.
pub fn held() -> ArgValueCandidates {
    listed(Request::Held, |r, answer| match answer {
        Response::Held { held } => held.iter().map(|h| candidate(&h.id, r.held(h))).collect(),
        _ => Vec::new(),
    })
}

/// The invitations not yet used, for `invite --cancel`.
pub fn invitations() -> ArgValueCandidates {
    listed(Request::Invitations, |r, answer| match answer {
        Response::Invitations { invitations } => invitations
            .iter()
            .map(|i| candidate(&i.id, r.invitation(i)))
            .collect(),
        _ => Vec::new(),
    })
}

/// The group's machines other than this one, for `remove-computer` and
/// `debug bundle --from`.
pub fn machines() -> ArgValueCandidates {
    listed(Request::Machines, |r, answer| match answer {
        Response::Machines { machines } => machines
            .iter()
            .filter(|m| !m.this_device)
            .map(|m| candidate(&m.name, r.machine(m)))
            .collect(),
        _ => Vec::new(),
    })
}

/// The settings' keys, each with its value.
pub fn setting_keys() -> ArgValueCandidates {
    listed(
        Request::GetSettings { key: None },
        |_, answer| match answer {
            Response::Settings { settings } => settings
                .iter()
                .map(|s| candidate(&s.key, s.value.clone()))
                .collect(),
            _ => Vec::new(),
        },
    )
}

/// The values the setting before the cursor may take.
pub fn setting_values() -> ArgValueCandidates {
    ArgValueCandidates::new(|| {
        let Some(key) = previous_word() else {
            return Vec::new();
        };
        match ask(&Request::GetSettings {
            key: Some(key.clone()),
        }) {
            Some(Response::Settings { settings }) => settings
                .into_iter()
                .filter(|s| s.key == key)
                .flat_map(|s| s.choices)
                .map(CompletionCandidate::new)
                .collect(),
            _ => Vec::new(),
        }
    })
}

/// The word before the one being completed. The shell passes the command
/// line after `--`, and bash and zsh the cursor's word index in
/// `_CLAP_COMPLETE_INDEX`; fish cuts the line at the cursor.
fn previous_word() -> Option<String> {
    let words: Vec<OsString> = std::env::args_os()
        .skip_while(|w| w != "--")
        .skip(1)
        .collect();
    let current = std::env::var("_CLAP_COMPLETE_INDEX")
        .ok()
        .and_then(|i| i.parse::<usize>().ok())
        .unwrap_or(words.len().saturating_sub(1));
    let word = words.get(current.checked_sub(1)?)?;
    Some(word.to_string_lossy().into_owned())
}
