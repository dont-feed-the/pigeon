//! The CLI's parser and renderer: each command maps to its request, and
//! every message the renderer builds fits the catalogue.

use pigeon_proto::api::{Level, Machine, State, StatusReport, VersionStatus};
use pigeon_proto::{Message, Request, Response};

use crate::args::{Action, absolute, parse};

fn line(args: &str) -> crate::args::Line {
    let v: Vec<String> = args.split_whitespace().map(str::to_owned).collect();
    parse(&v)
}

#[test]
fn commands_map_to_requests() {
    assert_eq!(
        line("status --capacity").action,
        Action::Send(Request::Capacity)
    );
    assert_eq!(
        line("status some/path --capacity").action,
        Action::Usage("status".into())
    );
    assert_eq!(
        line("status --not-shared --capacity").action,
        Action::Usage("status".into())
    );
    let l = line("free docs --json");
    assert!(l.json);
    assert_eq!(
        l.action,
        Action::Send(Request::Evict {
            folder: absolute("docs")
        })
    );
    assert_eq!(
        line("create Family /tmp/f --k 1 --name lap").action,
        Action::Send(Request::Create {
            name: "Family".into(),
            folder: "/tmp/f".into(),
            k: 1,
            device_name: "lap".into(),
        })
    );
    assert_eq!(
        line("invite --backup-machine nas").action,
        Action::Send(Request::Invite {
            person: String::new(),
            admin: false,
            custodian: true,
            name: Some("nas".into()),
        })
    );
    assert_eq!(
        line("invite --admin Paul").action,
        Action::Send(Request::Invite {
            person: "Paul".into(),
            admin: true,
            custodian: false,
            name: None,
        })
    );
    assert_eq!(line("invite").action, Action::Usage("invite".into()));
    assert!(matches!(
        line("create F --folder /tmp/f").action,
        Action::Send(Request::Create { ref name, ref folder, k: 2, .. })
            if name == "F" && folder == "/tmp/f"
    ));
    assert_eq!(
        line("debug bundle --from=nas").action,
        Action::Send(Request::DebugBundle {
            peers: vec!["nas".into()]
        })
    );
    assert_eq!(
        line("status").action,
        Action::Send(Request::Status {
            path: None,
            not_shared: false
        })
    );
}

#[test]
fn other_commands_are_not_in_this_build_and_bad_arguments_are_usage() {
    assert_eq!(line("erase").action, Action::NotInThisBuild("erase".into()));
    assert_eq!(line("free").action, Action::Usage("free".into()));
    assert_eq!(line("free a b").action, Action::Usage("free".into()));
    assert_eq!(
        line("status --bogus").action,
        Action::Usage("status".into())
    );
    assert_eq!(
        line("create x --k 0").action,
        Action::Usage("create".into())
    );
    let l = line("remove-computer nas --yes");
    assert!(l.yes && !l.no_input);
    assert_eq!(
        l.action,
        Action::Send(Request::RemoveDevice { name: "nas".into() })
    );
    assert!(!line("remove-computer nas").yes);
    assert!(line("remove-computer nas --no-input").no_input);
    assert_eq!(line("").action, Action::Help);
    assert_eq!(line("keep --help").action, Action::Help);
    assert_eq!(line("--version").action, Action::Version);
    assert_eq!(line("update").action, Action::Update(vec![]));
    assert_eq!(
        line("update --version v2.0.0").action,
        Action::Update(vec!["--version".into(), "v2.0.0".into()])
    );
    assert_eq!(
        line("update --rollback").action,
        Action::Update(vec!["--rollback".into()])
    );
    assert_eq!(
        line("update --rollback --version v2.0.0").action,
        Action::Usage("update".into())
    );
    assert_eq!(
        line("update --main").action,
        Action::Update(vec!["--main".into()])
    );
    assert_eq!(
        line("update --main --rollback").action,
        Action::Usage("update".into())
    );
    assert_eq!(
        line("update --main --version v0.3.1").action,
        Action::Usage("update".into())
    );
}

#[test]
fn the_definition_is_sound_and_has_the_help_s_commands() {
    let cmd = crate::args::command();
    cmd.clone().debug_assert();
    let defined: std::collections::BTreeSet<&str> = cmd
        .get_subcommands()
        .map(clap::Command::get_name)
        .filter(|n| *n != "help")
        .collect();
    let helped = crate::args::COMMANDS
        .iter()
        .filter_map(|(synopsis, _)| synopsis.split(' ').next())
        .collect();
    assert_eq!(defined, helped);
}

#[test]
fn each_form_is_checked() {
    use pigeon_proto::api::HeldChoice;
    assert_eq!(
        line("invite --list").action,
        Action::Send(Request::Invitations)
    );
    assert_eq!(
        line("invite --cancel i1").action,
        Action::Send(Request::CancelInvitation {
            invitation: "i1".into()
        })
    );
    assert_eq!(
        line("invite --list Paul").action,
        Action::Usage("invite".into())
    );
    assert_eq!(
        line("invite --admin --backup-machine").action,
        Action::Usage("invite".into())
    );
    assert_eq!(line("held").action, Action::Send(Request::Held));
    assert_eq!(
        line("held b1 --bring-back").action,
        Action::Send(Request::ReleaseHeld {
            batch: "b1".into(),
            choice: HeldChoice::BringBack,
        })
    );
    assert_eq!(line("held b1").action, Action::Usage("held".into()));
    assert_eq!(line("held --share").action, Action::Usage("held".into()));
    assert_eq!(
        line("held b1 --share --rename").action,
        Action::Usage("held".into())
    );
    assert_eq!(line("resolve a").action, Action::Usage("resolve".into()));
    assert_eq!(
        line("resolve a --take-group --suggest-again").action,
        Action::Usage("resolve".into())
    );
    assert_eq!(
        line("view a --version 2 --group").action,
        Action::Usage("view".into())
    );
    assert_eq!(
        line("settings language fr").action,
        Action::Send(Request::SetSettings {
            key: "language".into(),
            value: "fr".into(),
        })
    );
    assert_eq!(line("debug").action, Action::Usage("debug".into()));
    assert_eq!(
        line("join pgn1-x --backup-machine").action,
        Action::Send(Request::Join {
            invitation: "pgn1-x".into(),
            folder: None,
            keep: false,
            name: None,
        })
    );
    let l = line("erase --json");
    assert!(l.json);
    assert_eq!(l.action, Action::NotInThisBuild("erase".into()));
}

/// The candidates for the last word of `typed`, and their help.
fn completed(typed: &str) -> Vec<(String, String)> {
    let mut words: Vec<std::ffi::OsString> = typed.split(' ').map(Into::into).collect();
    words.insert(0, "pigeon".into());
    let index = words.len() - 1;
    let mut cmd = crate::complete::described();
    clap_complete::engine::complete(&mut cmd, words, index, None)
        .unwrap()
        .iter()
        .map(|c| {
            let help = c.get_help().map(ToString::to_string).unwrap_or_default();
            (c.get_value().to_string_lossy().into_owned(), help)
        })
        .collect()
}

#[test]
fn commands_and_options_complete_with_their_help() {
    let free = completed("fr");
    assert_eq!(free.len(), 1, "{free:?}");
    assert_eq!(free[0].0, "free");
    assert!(!free[0].1.is_empty(), "{free:?}");
    let options: Vec<String> = completed("resolve a --").into_iter().map(|c| c.0).collect();
    for o in ["--take-group", "--keep-copy", "--suggest-again", "--json"] {
        assert!(options.iter().any(|c| c == o), "{o}: {options:?}");
    }
    let shared = completed("status --not");
    assert_eq!(shared.len(), 1, "{shared:?}");
    assert!(!shared[0].1.is_empty(), "{shared:?}");
    let debug: Vec<String> = completed("debug ").into_iter().map(|c| c.0).collect();
    assert!(debug.iter().any(|c| c == "bundle"), "{debug:?}");
    assert!(debug.iter().any(|c| c == "allow"), "{debug:?}");
}

fn version(state: State) -> VersionStatus {
    VersionStatus {
        path: "a.txt".into(),
        number: 1,
        saved_at: "2026-10-14T10:00:00Z".into(),
        state,
        observed_at: None,
        reason: None,
    }
}

#[test]
fn responses_render_without_placeholders() {
    let now = pigeon_proto::time::parse_rfc3339("2026-10-14T12:00:00Z").unwrap_or_default();
    let r = crate::render::for_person(now);
    let headline = Message {
        id: "headline-safe".into(),
        args: [(
            "ago".to_owned(),
            pigeon_proto::Arg::Text("2026-10-14T11:55:00Z".into()),
        )]
        .into_iter()
        .collect(),
    };
    let responses = [
        Response::Status(StatusReport {
            headline,
            level: Level::Working,
            state: State::Published,
            versions: vec![version(State::Published), version(State::Missing)],
            not_shared: Vec::new(),
        }),
        Response::Machines {
            machines: vec![
                Machine {
                    name: "nas".into(),
                    device: "ab".into(),
                    custodian: true,
                    recovery: false,
                    heard_at: Some("2026-10-14T09:00:00Z".into()),
                    this_device: false,
                },
                Machine {
                    name: "lap".into(),
                    device: "cd".into(),
                    custodian: false,
                    recovery: false,
                    heard_at: None,
                    this_device: true,
                },
            ],
        },
        Response::History {
            versions: vec![version(State::Protected)],
        },
    ];
    let mut all = Vec::new();
    for resp in &responses {
        all.extend(r.response(resp).0);
    }
    all.extend(crate::render::help(&r));
    let text = all.join("\n");
    assert!(!text.contains('{'), "{text}");
    assert!(text.contains("5 minutes ago"), "{text}");
    assert!(
        text.contains("nas: backup machine, last heard 3 hours ago"),
        "{text}"
    );
}

#[test]
fn local_problems_fit_the_catalogue() {
    for (code, args) in [
        ("daemon-not-running", &[][..]),
        ("version-mismatch", &[]),
        ("usage", &[("command", "free")]),
        ("not-in-this-build", &[("command", "erase")]),
        ("not-installed", &[]),
        ("internal-error", &[]),
    ] {
        let p = crate::local_problem(code, args);
        let m = pigeon_i18n::Message {
            id: p.message.id.clone(),
            args: p
                .message
                .args
                .iter()
                .map(|(k, v)| match v {
                    pigeon_proto::Arg::Text(t) => (k.clone(), pigeon_i18n::Arg::Text(t.clone())),
                    pigeon_proto::Arg::Number(n) => (k.clone(), pigeon_i18n::Arg::Number(*n)),
                })
                .collect(),
        };
        assert_eq!(pigeon_i18n::check(&m), Ok(()), "{code}");
        let lines = crate::render::for_person(0).problem(&p);
        assert!(lines.iter().all(|l| !l.contains('{')), "{lines:?}");
    }
}
