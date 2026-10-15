//! The checks of the module header, through the router (10 §10.3).
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use pigeon_proto::api::{Level, Setting, State, StatusReport, Suggestion};
use pigeon_proto::{Message, Response};
use tower::ServiceExt;

use crate::{Answer, Backend, Web};

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
            pigeon_proto::Request::GetSettings => vec![Response::Settings {
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
            _ => vec![Response::Machines {
                machines: Vec::new(),
            }],
        };
        Box::pin(async move { answer })
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
    assert!(text.contains("language is now fr."), "{text}");
}
