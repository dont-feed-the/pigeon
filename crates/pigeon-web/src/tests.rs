//! The checks of the module header, through the router (10 §10.3).
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use pigeon_proto::api::{Level, Machine, Setting, State, StatusReport, Suggestion};
use pigeon_proto::{Message, Response};
use tower::ServiceExt;

use crate::{Answer, Backend, Web};

#[path = "tests/usability.rs"]
mod usability;

struct Fixed;

impl Backend for Fixed {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        let answer = match request {
            pigeon_proto::Request::Status { .. } => vec![Response::Status(StatusReport {
                headline: Message {
                    id: "headline-safe".into(),
                    args: [("ago".to_owned(), pigeon_proto::Arg::Text("x".into()))].into(),
                },
                level: Level::Safe,
                state: State::Protected,
                versions: Vec::new(),
                not_shared: Vec::new(),
            })],
            pigeon_proto::Request::Proposals => vec![Response::Proposals {
                proposals: vec![Suggestion {
                    id: "c0ffee".into(),
                    author: "Paul".into(),
                    device: "paul-laptop".into(),
                    saved_at: "2026-10-15T10:00:00Z".into(),
                    paths: vec!["tarte.docx".into()],
                    decide: true,
                    force: false,
                    mine: false,
                }],
            }],
            pigeon_proto::Request::Reject { proposal, note } => vec![Response::Said {
                message: Message {
                    id: "declined-suggestion".into(),
                    args: [(
                        "author".to_owned(),
                        pigeon_proto::Arg::Text(format!("{proposal}/{}", note.unwrap_or_default())),
                    )]
                    .into(),
                },
            }],
            pigeon_proto::Request::GetSettings { .. } => vec![Response::Settings {
                settings: vec![Setting {
                    key: "language".into(),
                    value: "auto".into(),
                    choices: vec!["auto".into(), "en".into(), "fr".into()],
                }],
            }],
            pigeon_proto::Request::SetSettings { key, value } => vec![Response::Said {
                message: Message {
                    id: "setting-changed".into(),
                    args: [
                        ("key".to_owned(), pigeon_proto::Arg::Text(key)),
                        ("value".to_owned(), pigeon_proto::Arg::Text(value)),
                    ]
                    .into(),
                },
            }],
            pigeon_proto::Request::Machines => vec![Response::Machines {
                machines: ["mine", "paul-laptop"]
                    .map(|name| Machine {
                        name: name.into(),
                        device: "d".into(),
                        custodian: false,
                        heard_at: None,
                        this_device: name == "mine",
                    })
                    .into(),
            }],
            pigeon_proto::Request::RemoveDevice { name } => {
                vec![said("removed-computer", "name", name)]
            }
            pigeon_proto::Request::Invitations => vec![Response::Invitations {
                invitations: vec![pigeon_proto::api::InvitationEntry {
                    id: "ab12".into(),
                    who: "Ana".into(),
                    custodian: false,
                    expires: "2026-10-22T10:00:00Z".into(),
                }],
            }],
            pigeon_proto::Request::CancelInvitation { invitation } => {
                vec![said("invitation-cancelled", "id", invitation)]
            }
            pigeon_proto::Request::Held => vec![Response::Held {
                held: vec![
                    pigeon_proto::api::HeldEntry {
                        id: "h1".into(),
                        reason: Message {
                            id: "held-top-folder".into(),
                            args: [(
                                "folder".to_owned(),
                                pigeon_proto::Arg::Text("Photos".into()),
                            )]
                            .into(),
                        },
                        files: 3,
                        since: "2026-10-15T10:00:00Z".into(),
                        paths: vec!["Photos/a.jpg".into()],
                        suggested: None,
                    },
                    pigeon_proto::api::HeldEntry {
                        id: "h2".into(),
                        reason: Message {
                            id: "held-name".into(),
                            args: [
                                ("path".to_owned(), pigeon_proto::Arg::Text("a:b.txt".into())),
                                (
                                    "suggested".to_owned(),
                                    pigeon_proto::Arg::Text("a-b.txt".into()),
                                ),
                            ]
                            .into(),
                        },
                        files: 1,
                        since: "2026-10-15T10:00:00Z".into(),
                        paths: vec!["a:b.txt".into()],
                        suggested: Some("a-b.txt".into()),
                    },
                ],
            }],
            pigeon_proto::Request::ReleaseHeld { batch, choice } => {
                vec![said("held-shared", "files", format!("{batch}/{choice:?}"))]
            }
            pigeon_proto::Request::Leave => vec![said("leaving", "name", "mine".into())],
            pigeon_proto::Request::Invite { person, .. } => {
                vec![said("invitation", "code", format!("pgn1-{person}"))]
            }
            pigeon_proto::Request::History { path } => vec![Response::History {
                versions: [1, 2]
                    .map(|number| pigeon_proto::api::VersionStatus {
                        path: path.clone(),
                        number,
                        saved_at: "2026-10-15T10:00:00Z".into(),
                        state: State::Protected,
                        observed_at: None,
                        reason: None,
                    })
                    .into(),
            }],
            pigeon_proto::Request::View {
                version: Some(1), ..
            } => {
                let dir =
                    std::env::temp_dir().join(format!("pigeon-web-view-{}", std::process::id()));
                std::fs::create_dir_all(&dir).unwrap();
                let copy = dir.join("tarte (version 1).docx");
                std::fs::write(&copy, b"old tarte").unwrap();
                vec![said(
                    "view-ready",
                    "path",
                    copy.to_string_lossy().into_owned(),
                )]
            }
            pigeon_proto::Request::View { version, .. } => {
                vec![said("removed-computer", "name", format!("v{version:?}"))]
            }
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

fn said(id: &str, arg: &str, value: String) -> Response {
    Response::Said {
        message: Message {
            id: id.into(),
            args: [(arg.to_owned(), pigeon_proto::Arg::Text(value))].into(),
        },
    }
}

fn web() -> Arc<Web> {
    Web::new(Arc::new(Fixed), "en".parse().unwrap(), 4321)
}

fn get(path: &str, host: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::get(path).header(header::HOST, host);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    b.body(Body::empty()).unwrap()
}

async fn body(r: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn a_token_logs_in_once_and_its_cookie_opens_the_pages() {
    let w = web();
    let app = w.router();
    let host = "127.0.0.1:4321";
    let token = w.login_token();
    let login = format!("/login?token={token}");
    let r = app.clone().oneshot(get(&login, host, None)).await.unwrap();
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
    let set = r.headers()[header::SET_COOKIE].to_str().unwrap().to_owned();
    assert!(
        set.contains("HttpOnly") && set.contains("SameSite=Strict"),
        "{set}"
    );
    let cookie = set.split(';').next().unwrap().to_owned();
    let again = app.clone().oneshot(get(&login, host, None)).await.unwrap();
    assert_eq!(again.status(), StatusCode::FORBIDDEN, "a token works once");
    let home = app
        .clone()
        .oneshot(get("/", host, Some(&cookie)))
        .await
        .unwrap();
    assert_eq!(home.status(), StatusCode::OK);
    let text = body(home).await;
    assert!(text.contains("Everything is safe"), "{text}");
}

#[tokio::test]
async fn no_cookie_another_host_or_another_origin_is_refused() {
    let w = web();
    let app = w.router();
    let token = w.login_token();
    let r = app
        .clone()
        .oneshot(get(
            &format!("/login?token={token}"),
            "127.0.0.1:4321",
            None,
        ))
        .await
        .unwrap();
    let cookie = r.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let none = app
        .clone()
        .oneshot(get("/", "127.0.0.1:4321", None))
        .await
        .unwrap();
    assert_eq!(none.status(), StatusCode::FORBIDDEN);
    let rebound = app
        .clone()
        .oneshot(get("/", "evil.example:4321", Some(&cookie)))
        .await
        .unwrap();
    assert_eq!(rebound.status(), StatusCode::FORBIDDEN, "DNS rebinding");
    let mut cross = get("/", "127.0.0.1:4321", Some(&cookie));
    cross
        .headers_mut()
        .insert(header::ORIGIN, "https://evil.example".parse().unwrap());
    let cross = app.clone().oneshot(cross).await.unwrap();
    assert_eq!(cross.status(), StatusCode::FORBIDDEN, "another site");
    let unknown = app
        .oneshot(get("/login?token=00", "127.0.0.1:4321", None))
        .await
        .unwrap();
    assert_eq!(unknown.status(), StatusCode::FORBIDDEN);
}

async fn logged_in(app: &axum::Router, w: &Web) -> String {
    let token = w.login_token();
    let r = app
        .clone()
        .oneshot(get(&format!("/login?token={token}"), HOST, None))
        .await
        .unwrap();
    let set = r.headers()[header::SET_COOKIE].to_str().unwrap();
    set.split(';').next().unwrap().to_owned()
}

const HOST: &str = "127.0.0.1:4321";

fn post(path: &str, cookie: &str, form: &str) -> Request<Body> {
    Request::post(path)
        .header(header::HOST, HOST)
        .header(header::ORIGIN, format!("http://{HOST}"))
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap()
}

#[tokio::test]
async fn a_suggestion_is_declined_only_with_the_sessions_form_token() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = app
        .clone()
        .oneshot(get("/suggestions", HOST, Some(&cookie)))
        .await
        .unwrap();
    let page = body(page).await;
    assert!(
        page.contains("Paul suggests a change of tarte.docx"),
        "{page}"
    );
    let key = "name=\"form\" value=\"";
    let at = page.find(key).unwrap() + key.len();
    let form = page[at..at + 64].to_owned();
    let forged = app
        .clone()
        .oneshot(post(
            "/suggestions/decline",
            &cookie,
            "form=00&id=c0ffee&note=no",
        ))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    let other = logged_in(&app, &w).await;
    let stolen = app
        .clone()
        .oneshot(post(
            "/suggestions/decline",
            &other,
            &format!("form={form}&id=c0ffee&note=no"),
        ))
        .await
        .unwrap();
    assert_eq!(stolen.status(), StatusCode::FORBIDDEN, "another session's");
    let ok = app
        .oneshot(post(
            "/suggestions/decline",
            &cookie,
            &format!("form={form}&id=c0ffee&note=too+long"),
        ))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    let text = body(ok).await;
    assert!(text.contains("c0ffee/too long"), "{text}");
}

#[tokio::test]
async fn a_setting_is_shown_and_saved_with_the_sessions_form_token() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = app
        .clone()
        .oneshot(get("/settings", HOST, Some(&cookie)))
        .await
        .unwrap();
    let page = body(page).await;
    assert!(page.contains("Language"), "{page}");
    assert!(
        page.contains("<option value=\"auto\" selected>Same as this computer</option>"),
        "{page}"
    );
    assert!(page.contains(">Français</option>"), "{page}");
    let key = "name=\"form\" value=\"";
    let at = page.find(key).unwrap() + key.len();
    let form = page[at..at + 64].to_owned();
    let forged = app
        .clone()
        .oneshot(post("/settings", &cookie, "form=00&key=language&value=fr"))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    let ok = app
        .oneshot(post(
            "/settings",
            &cookie,
            &format!("form={form}&key=language&value=fr"),
        ))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    let text = body(ok).await;
    assert!(text.contains("Language: Français."), "{text}");
}

/// The form token of a page.
fn form_of(page: &str) -> String {
    let key = "name=\"form\" value=\"";
    let at = page.find(key).unwrap() + key.len();
    page[at..at + 64].to_owned()
}

#[tokio::test]
async fn a_machine_is_removed_once_confirmed_and_a_person_invited() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = app
        .clone()
        .oneshot(get("/machines", HOST, Some(&cookie)))
        .await
        .unwrap();
    let page = body(page).await;
    assert!(page.contains("mine: this computer"), "{page}");
    assert_eq!(
        page.matches("action=\"/machines/remove\"").count(),
        1,
        "{page}"
    );
    let form = form_of(&page);
    let ask = app
        .clone()
        .oneshot(post(
            "/machines/remove",
            &cookie,
            &format!("form={form}&name=paul-laptop"),
        ))
        .await
        .unwrap();
    let ask = body(ask).await;
    assert!(ask.contains("Remove paul-laptop from the group?"), "{ask}");
    assert!(!ask.contains("is removed"), "{ask}");
    let done = app
        .clone()
        .oneshot(post(
            "/machines/remove",
            &cookie,
            &format!("form={form}&name=paul-laptop&yes=true"),
        ))
        .await
        .unwrap();
    let done = body(done).await;
    assert!(done.contains("paul-laptop is removed"), "{done}");
    let forged = app
        .clone()
        .oneshot(post("/machines/invite", &cookie, "form=00&person=Ana"))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    let invited = app
        .oneshot(post(
            "/machines/invite",
            &cookie,
            &format!("form={form}&person=Ana&admin=true"),
        ))
        .await
        .unwrap();
    let invited = body(invited).await;
    assert!(invited.contains("pgn1-Ana"), "{invited}");
}

#[tokio::test]
async fn a_pending_invitation_is_listed_and_cancelled() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = app
        .clone()
        .oneshot(get("/machines", HOST, Some(&cookie)))
        .await
        .unwrap();
    let page = body(page).await;
    assert!(page.contains("Pending invitations"), "{page}");
    assert!(page.contains("ab12: for Ana, until"), "{page}");
    let form = form_of(&page);
    let forged = app
        .clone()
        .oneshot(post(
            "/machines/cancel-invitation",
            &cookie,
            "form=00&invitation=ab12",
        ))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    let done = app
        .oneshot(post(
            "/machines/cancel-invitation",
            &cookie,
            &format!("form={form}&invitation=ab12"),
        ))
        .await
        .unwrap();
    let done = body(done).await;
    assert!(
        done.contains("Invitation ab12 can no longer be used."),
        "{done}"
    );
}

#[tokio::test]
async fn a_held_deletion_is_shared_once_confirmed_or_brought_back() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let home = body(
        app.clone()
            .oneshot(get("/", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(home.contains("Changes waiting for you"), "{home}");
    assert!(home.contains("the folder Photos removed"), "{home}");
    let form = form_of(&home);
    let forged = app
        .clone()
        .oneshot(post("/held/share", &cookie, "form=00&batch=h1"))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    let ask = app
        .clone()
        .oneshot(post(
            "/held/share",
            &cookie,
            &format!("form={form}&batch=h1"),
        ))
        .await
        .unwrap();
    let ask = body(ask).await;
    assert!(
        ask.contains("Delete the files of h1 for everyone?"),
        "{ask}"
    );
    assert!(!ask.contains("h1/Share"), "{ask}");
    let done = app
        .clone()
        .oneshot(post(
            "/held/share",
            &cookie,
            &format!("form={form}&batch=h1&yes=true"),
        ))
        .await
        .unwrap();
    let done = body(done).await;
    assert!(done.contains("h1/Share"), "{done}");
    let back = app
        .oneshot(post(
            "/held/bring-back",
            &cookie,
            &format!("form={form}&batch=h1"),
        ))
        .await
        .unwrap();
    let back = body(back).await;
    assert!(back.contains("h1/BringBack"), "{back}");
}

#[tokio::test]
async fn a_held_name_is_renamed_to_the_suggestion() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let home = body(
        app.clone()
            .oneshot(get("/", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(home.contains("Rename to a-b.txt"), "{home}");
    let form = form_of(&home);
    let done = app
        .oneshot(post(
            "/held/rename",
            &cookie,
            &format!("form={form}&batch=h2"),
        ))
        .await
        .unwrap();
    let done = body(done).await;
    assert!(done.contains("h2/Rename"), "{done}");
}

#[tokio::test]
async fn leaving_from_settings_is_asked_again_first() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = app
        .clone()
        .oneshot(get("/settings", HOST, Some(&cookie)))
        .await
        .unwrap();
    let page = body(page).await;
    assert!(page.contains("Leave the group"), "{page}");
    let form = form_of(&page);
    let ask = app
        .clone()
        .oneshot(post("/leave", &cookie, &format!("form={form}")))
        .await
        .unwrap();
    let ask = body(ask).await;
    assert!(
        ask.contains("Remove this computer from the group?"),
        "{ask}"
    );
    let done = app
        .oneshot(post("/leave", &cookie, &format!("form={form}&yes=true")))
        .await
        .unwrap();
    let done = body(done).await;
    assert!(done.contains("mine leaves the group"), "{done}");
}

#[tokio::test]
async fn a_version_is_opened_as_a_read_only_download() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = app
        .clone()
        .oneshot(get("/file?path=%2Fg%2Ftarte.docx", HOST, Some(&cookie)))
        .await
        .unwrap();
    let page = body(page).await;
    assert_eq!(page.matches("Open a copy").count(), 2, "{page}");
    assert_eq!(page.matches("Restore this version").count(), 1, "{page}");
    let form = form_of(&page);
    let forged = app
        .clone()
        .oneshot(post(
            "/files/view",
            &cookie,
            "form=00&path=%2Fg%2Ftarte.docx&version=1",
        ))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    let copy = app
        .clone()
        .oneshot(post(
            "/files/view",
            &cookie,
            &format!("form={form}&path=%2Fg%2Ftarte.docx&version=1"),
        ))
        .await
        .unwrap();
    assert_eq!(copy.status(), StatusCode::OK);
    let disposition = copy.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        disposition.starts_with("attachment; filename=\"tarte (version 1).docx\""),
        "{disposition}"
    );
    assert_eq!(body(copy).await, "old tarte");
    let other = app
        .oneshot(post(
            "/files/view",
            &cookie,
            &format!("form={form}&path=%2Fg%2Ftarte.docx&version=2"),
        ))
        .await
        .unwrap();
    let other = body(other).await;
    assert!(other.contains("vSome(2) is removed"), "{other}");
}

/// Writes every page, in English and in French, to `$PIGEON_WEB_PAGES` for
/// `scripts/check-a11y`, which runs axe-core on them (10 §10.3); without
/// the variable it does nothing.
#[tokio::test]
async fn the_pages_are_written_for_axe() {
    let Some(dir) = std::env::var_os("PIGEON_WEB_PAGES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pages = [
        ("home", "/"),
        ("machines", "/machines"),
        ("suggestions", "/suggestions"),
        ("files", "/files"),
        ("folder", "/folder?path=%2Fg%2FPhotos"),
        ("deleted", "/deleted"),
        ("file", "/file?path=%2Fg%2Ftarte.docx"),
        ("settings", "/settings"),
    ];
    for lang in ["en", "fr"] {
        let w = Web::new(Arc::new(Fixed), lang.parse().unwrap(), 4321);
        let app = w.router();
        let cookie = logged_in(&app, &w).await;
        for (name, path) in pages {
            let r = app
                .clone()
                .oneshot(get(path, HOST, Some(&cookie)))
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::OK, "{path}");
            std::fs::write(dir.join(format!("{lang}-{name}.html")), body(r).await).unwrap();
        }
    }
}
