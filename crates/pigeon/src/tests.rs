//! The CLI's parser and renderer: each command maps to its request, and
//! every message the renderer builds fits the catalogue.

use pigeon_proto::api::{Level, Machine, State, StatusReport, VersionStatus};
use pigeon_proto::{Message, Request, Response};

use crate::args::{Action, absolute, parse};
use crate::render::Renderer;

fn line(args: &str) -> crate::args::Line {
    let v: Vec<String> = args.split_whitespace().map(str::to_owned).collect();
    parse(&v)
}

#[test]
fn commands_map_to_requests() {
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
    assert_eq!(line("pause").action, Action::NotInThisBuild("pause".into()));
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
    let r = Renderer::new(now);
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
                    heard_at: Some("2026-10-14T09:00:00Z".into()),
                    this_device: false,
                },
                Machine {
                    name: "lap".into(),
                    device: "cd".into(),
                    custodian: false,
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
    all.extend(r.help());
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
        ("not-in-this-build", &[("command", "pause")]),
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
        let lines = Renderer::new(0).problem(&p);
        assert!(lines.iter().all(|l| !l.contains('{')), "{lines:?}");
    }
}
