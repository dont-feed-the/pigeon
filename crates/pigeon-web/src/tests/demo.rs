//! A family's group as a backend, for the Files browser's tests, the pages
//! axe-core checks, and looking at the interface: folders and files of
//! every kind and state, a freed folder, READMEs, versions and rules.
//! `PIGEON_WEB_DEMO=PORT cargo test -p pigeon-web --lib serve_the_demo --
//! --ignored --nocapture` serves it on 127.0.0.1:PORT and prints a login.

use super::*;
use pigeon_proto::api::{FileEntry, Folder, PersonEntry, VersionStatus};
use pigeon_proto::policy;

pub(super) const ROOT: &str = "/home/martine/Famille";

pub(super) const README: &str = "# Famille Geiger\n\n\
Our shared folder. **Everything here is kept safe** on the backup machines.\n\n\
## Where things are\n\n\
| Folder | What |\n|---|---|\n| [Photos](Photos/) | every photo, by date |\n| [Budget](Budget/) | the accounts |\n\n\
- [x] Scan the old albums\n- [ ] Sort the 2019 photos\n\n\
See [the budget](budget-2026.xlsx), or <https://example.org>.\n\n\
<script>alert('no')</script>\n\n![a remote picture](https://example.org/p.png)\n";

/// Each file: its path, state, size and age in hours.
const FILES: &[(&str, State, u64, i64)] = &[
    ("README.md", State::Protected, 1_240, 80),
    ("budget-2026.xlsx", State::Protected, 48_200, 2),
    ("Notes.txt", State::Local, 2_100, 0),
    ("Photos/README.md", State::Protected, 320, 400),
    (
        "Photos/2026-10-12/IMG_2201.jpg",
        State::Published,
        3_400_000,
        96,
    ),
    (
        "Photos/2026-10-12/IMG_2202.jpg",
        State::Published,
        3_150_000,
        96,
    ),
    (
        "Photos/2026-10-12/IMG_2203.jpg",
        State::Protected,
        3_900_000,
        96,
    ),
    (
        "Photos/Vacances/plage.heic",
        State::Protected,
        2_100_000,
        1_500,
    ),
    (
        "Photos/Vacances/anniversaire.mp4",
        State::Protected,
        120_000_000,
        1_600,
    ),
    ("Budget/impots-2025.pdf", State::Protected, 812_000, 3_000),
    ("Budget/comptes-2025.ods", State::Protected, 96_000, 2_000),
    ("Recettes de Paul/tarte.docx", State::Protected, 24_000, 30),
    ("Recettes de Paul/cookies.md", State::Protected, 1_800, 700),
    (
        "Photos 2019/janvier/neige.jpg",
        State::Protected,
        4_200_000,
        9_000,
    ),
    (
        "Photos 2019/juillet/lac.jpg",
        State::Protected,
        3_800_000,
        8_000,
    ),
    ("Musique/chanson.mp3", State::Protected, 6_100_000, 5_000),
    (
        "Archives/vieux-scans.zip",
        State::Protected,
        940_000_000,
        12_000,
    ),
];

pub(super) struct Demo;

fn ago(hours: i64) -> String {
    pigeon_proto::time::rfc3339(crate::now() - hours * 3600 - 420)
}

fn listing(prefix: &str) -> Vec<FileEntry> {
    FILES
        .iter()
        .filter(|(path, ..)| prefix.is_empty() || path.starts_with(&format!("{prefix}/")))
        .map(|&(path, state, size, hours)| FileEntry {
            path: path.into(),
            state,
            size: Some(size),
            modified: (!path.starts_with("Photos 2019/")).then(|| ago(hours)),
        })
        .collect()
}

fn folders() -> Vec<Folder> {
    let mut out: Vec<Folder> = Vec::new();
    for &(path, state, size, _) in FILES {
        let Some((top, _)) = path.split_once('/') else {
            continue;
        };
        if !out.iter().any(|f| f.path == top) {
            out.push(Folder {
                path: top.into(),
                files: 0,
                not_safe: 0,
                state: State::Protected,
                freed: top == "Photos 2019",
                arriving: 0,
                size: Some(0),
            });
        }
        let Some(f) = out.iter_mut().find(|f| f.path == top) else {
            continue;
        };
        f.files += 1;
        f.size = Some(f.size.unwrap_or(0) + size);
        if state != State::Protected {
            f.not_safe += 1;
            f.state = state;
        }
    }
    out
}

fn text(path: &str) -> Response {
    let rel = path
        .strip_prefix(ROOT)
        .unwrap_or(path)
        .trim_start_matches('/');
    match rel {
        "README.md" => Response::Text {
            text: README.into(),
            size: README.len() as u64,
            truncated: false,
        },
        "Photos/README.md" => Response::Text {
            text: "Photos of the family, one folder per day. Ask Paul before deleting any.\n"
                .into(),
            size: 72,
            truncated: false,
        },
        _ => Response::Problem(pigeon_proto::Problem {
            code: "content-not-here".into(),
            message: Message {
                id: "problem-content-not-here".into(),
                args: [("path".to_owned(), pigeon_proto::Arg::Text(rel.into()))].into(),
            },
            hint: None,
            details: None,
        }),
    }
}

fn person(name: &str, admin: bool, me: bool, machines: &[&str]) -> PersonEntry {
    PersonEntry {
        name: name.into(),
        admin,
        this_person: me,
        recovery_recommended: false,
        recovery_refresh_recommended: false,
        machines: machines.iter().map(|m| (*m).to_owned()).collect(),
    }
}

impl Backend for Demo {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        let answer = match request {
            pigeon_proto::Request::Folders => vec![Response::Folders {
                root: ROOT.into(),
                folders: folders(),
            }],
            pigeon_proto::Request::Files { folder } => {
                let prefix = folder
                    .strip_prefix(ROOT)
                    .unwrap_or_default()
                    .trim_matches('/');
                vec![Response::Files {
                    root: ROOT.into(),
                    files: listing(prefix),
                }]
            }
            pigeon_proto::Request::ReadText { path, .. } => vec![text(&path)],
            pigeon_proto::Request::History { path } => vec![Response::History {
                versions: [
                    (3, 2, State::Protected),
                    (2, 30, State::Protected),
                    (1, 200, State::Protected),
                ]
                .map(|(number, hours, state)| VersionStatus {
                    path: path.clone(),
                    number,
                    saved_at: ago(hours),
                    state,
                    observed_at: None,
                    reason: None,
                })
                .into(),
            }],
            pigeon_proto::Request::FolderRules { folder } => {
                let paul = folder
                    .as_deref()
                    .is_some_and(|f| f.contains("Recettes de Paul"));
                vec![Response::FolderRules(Box::new(policy::Report {
                    root: ROOT.into(),
                    can_edit: true,
                    rules: vec![policy::Rule {
                        path: if paul {
                            "Recettes de Paul".into()
                        } else {
                            String::new()
                        },
                        owner: paul.then(|| "paul".into()),
                        stewards: Vec::new(),
                        direct: paul,
                        per_person: false,
                    }],
                    people: vec![
                        policy::Person {
                            id: "martine".into(),
                            name: "Martine".into(),
                            this_person: true,
                            admin: false,
                        },
                        policy::Person {
                            id: "paul".into(),
                            name: "Paul".into(),
                            this_person: false,
                            admin: true,
                        },
                    ],
                }))]
            }
            pigeon_proto::Request::Deleted { .. } => vec![Response::Deleted {
                root: ROOT.into(),
                files: vec![
                    pigeon_proto::api::DeletedFile {
                        path: "Recettes/gratin.docx".into(),
                        deleted_at: ago(5),
                    },
                    pigeon_proto::api::DeletedFile {
                        path: "Recettes/soupe.docx".into(),
                        deleted_at: ago(5),
                    },
                    pigeon_proto::api::DeletedFile {
                        path: "brouillon.txt".into(),
                        deleted_at: ago(100),
                    },
                ],
            }],
            pigeon_proto::Request::People => vec![Response::People {
                people: vec![
                    person("Martine", false, true, &["martine-laptop"]),
                    person("Paul", true, false, &["paul-laptop", "paul-nas"]),
                ],
            }],
            pigeon_proto::Request::Machines => vec![Response::Machines {
                machines: [
                    ("martine-laptop", false, true),
                    ("paul-laptop", false, false),
                    ("paul-nas", true, false),
                ]
                .map(|(name, custodian, me)| pigeon_proto::api::Machine {
                    name: name.into(),
                    device: name.into(),
                    custodian,
                    recovery: false,
                    heard_at: Some(ago(1)),
                    this_device: me,
                })
                .into(),
            }],
            pigeon_proto::Request::Held => vec![Response::Held { held: Vec::new() }],
            other => return Fixed.request(other),
        };
        Box::pin(async move { answer })
    }
}

#[allow(clippy::print_stdout, reason = "the login links are the output")]
#[tokio::test]
#[ignore = "serves the demo until stopped; set PIGEON_WEB_DEMO to a port"]
async fn serve_the_demo() {
    let Some(port) = std::env::var("PIGEON_WEB_DEMO")
        .ok()
        .and_then(|p| p.parse().ok())
    else {
        return;
    };
    let web = Web::new(Arc::new(Demo), "en".parse().unwrap(), port);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    for _ in 0..20 {
        println!("{}", web.login_url());
    }
    axum::serve(listener, web.router()).await.unwrap();
}
