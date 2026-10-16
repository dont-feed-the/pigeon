//! Shared peer pages through real local RPC framing: client selection,
//! account review, mounted navigation, CSRF and live-session revocation.

use super::*;
use futures_util::StreamExt;
use pigeon_proto::{Envelope, LineDecoder, PROTOCOL_VERSION, Request as Rpc, Response as Answer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

const COOKIE: &[(&str, &str)] = &[("Cookie", "pigeon-bridge-session=browser")];
const FORM: &[(&str, &str)] = &[
    ("Cookie", "pigeon-bridge-session=browser"),
    ("Content-Type", "application/x-www-form-urlencoded"),
];

pub(crate) async fn signed_in(app: &App) {
    app.sessions.lock().await.insert(
        "browser".into(),
        auth::Session {
            user: "alice".into(),
            csrf: "alice-form".into(),
            expires: store::now() + 100,
        },
    );
}

async fn text(response: axum::response::Response) -> String {
    String::from_utf8(
        axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

/// A socket can only answer as the identity it was given. Every request is
/// recorded, so isolation assertions also cover requests that render no text.
pub(crate) async fn peer(app: &App, client: Option<Client>) -> Arc<Mutex<Vec<Rpc>>> {
    let (socket, label) = client.as_ref().map_or_else(
        || {
            (
                app.config.users["alice"].control_socket.clone(),
                "control".to_owned(),
            )
        },
        |c| (app.peers.socket(&c.id), c.id.clone()),
    );
    let listener = tokio::net::UnixListener::bind(socket).unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let recorded = calls.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let recorded = recorded.clone();
            let client = client.clone();
            let label = label.clone();
            tokio::spawn(async move {
                let mut decoder = LineDecoder::new();
                let mut buf = [0; 8192];
                loop {
                    let request: Envelope<Rpc> = loop {
                        if let Some(request) = decoder.next_line().unwrap() {
                            break request;
                        }
                        let Ok(n) = stream.read(&mut buf).await else {
                            return;
                        };
                        if n == 0 {
                            return;
                        }
                        decoder.push(&buf[..n]);
                    };
                    recorded.lock().await.push(request.body.clone());
                    let answer = match request.body {
                        Rpc::Hello { .. } => Answer::Hello {
                            version: PROTOCOL_VERSION,
                        },
                        Rpc::Bridge {
                            operation: pigeon_proto::bridge::Operation::Info,
                        } => {
                            let c = client.as_ref().unwrap();
                            Answer::Bridge {
                                result: pigeon_proto::bridge::Reply::Info {
                                    group: "group".into(),
                                    person: c.user.clone(),
                                    device: c.device.clone(),
                                    annex_uuid: c.annex_uuid.clone(),
                                    root: "/Family".into(),
                                    addresses: vec![],
                                    shared: true,
                                },
                            }
                        }
                        Rpc::GetSettings { .. } => Answer::Settings {
                            settings: vec![pigeon_proto::api::Setting {
                                key: "language".into(),
                                value: "en".into(),
                                choices: vec!["en".into(), "fr".into()],
                            }],
                        },
                        Rpc::Health => Answer::Said {
                            message: pigeon_proto::Message {
                                id: "removed-computer".into(),
                                args: [("name".into(), pigeon_proto::Arg::Text(label.clone()))]
                                    .into(),
                            },
                        },
                        Rpc::Proposals => Answer::Proposals {
                            proposals: vec![pigeon_proto::api::Suggestion {
                                id: "proposal".into(),
                                author: label.clone(),
                                device: label.clone(),
                                saved_at: "2026-10-16T00:00:00Z".into(),
                                paths: vec!["Docs/a & b.txt".into()],
                                decide: true,
                                force: false,
                                mine: false,
                                files: Vec::new(),
                            }],
                            decided: Vec::new(),
                            root: String::new(),
                        },
                        Rpc::Folders => Answer::Folders {
                            root: "/Family".into(),
                            folders: vec![],
                        },
                        Rpc::Files { .. } => Answer::Files {
                            root: "/Family".into(),
                            files: vec![pigeon_proto::api::FileEntry {
                                path: "a & b.txt".into(),
                                state: pigeon_proto::api::State::Local,
                                size: None,
                                modified: None,
                                freed: false,
                            }],
                        },
                        _ => Answer::Done,
                    };
                    if stream
                        .write_all(&pigeon_proto::encode(request.id, &answer).unwrap())
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            });
        }
    });
    calls
}

#[tokio::test]
async fn shared_pages_and_actions_select_the_client_or_personal_control_peer() {
    let (_tmp, app) = app().await;
    signed_in(&app).await;
    let a = client("a", "alice", "app-a");
    let c = client("c", "alice", "app-c");
    app.store
        .lock()
        .await
        .clients
        .insert(c.id.clone(), c.clone());
    let a_calls = peer(&app, Some(a.clone())).await;
    let c_calls = peer(&app, Some(c.clone())).await;
    let control_calls = peer(&app, None).await;
    for (selected, other) in [(&a, &c), (&c, &a)] {
        let prefix = format!("/clients/{}/peer", selected.id);
        let page = request(
            app.clone(),
            "GET",
            &format!("{prefix}/suggestions"),
            COOKIE,
            "",
        )
        .await;
        assert_eq!(page.status(), StatusCode::OK);
        assert!(
            page.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("script-src 'self'")
        );
        let page = text(page).await;
        assert!(page.contains(&selected.id) && !page.contains(&other.id));
        assert!(page.contains(&format!("action=\"{prefix}/suggestions/decide\"")));
        assert!(page.contains("name=\"form\" value=\"alice-form\""));
        assert!(page.contains("Docs/a &amp; b.txt"));
        assert_eq!(
            request(
                app.clone(),
                "POST",
                &format!("{prefix}/suggestions/accept"),
                FORM,
                "form=alice-form&id=proposal"
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    let account = text(request(app.clone(), "GET", "/peer/suggestions", COOKIE, "").await).await;
    assert!(account.contains("control") && account.contains("action=\"/peer/suggestions/decide\""));
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/peer/suggestions/accept",
            FORM,
            "form=alice-form&id=control-proposal"
        )
        .await
        .status(),
        StatusCode::OK
    );
    for calls in [&a_calls, &c_calls] {
        assert!(
            calls
                .lock()
                .await
                .iter()
                .any(|r| matches!(r, Rpc::Accept { proposal, .. } if proposal == "proposal"))
        );
        assert!(
            !calls.lock().await.iter().any(
                |r| matches!(r, Rpc::Accept { proposal, .. } if proposal == "control-proposal")
            )
        );
    }
    assert!(
        control_calls
            .lock()
            .await
            .iter()
            .any(|r| matches!(r, Rpc::Accept { proposal, .. } if proposal == "control-proposal"))
    );
    let prefix = format!("/clients/{}/peer", a.id);
    for suffix in [
        "/",
        "/files",
        "/file?path=%2FFamily%2Fa+%26+b.txt",
        "/folder?path=%2FFamily",
        "/deleted",
        "/space",
        "/machines",
        "/settings",
        "/doctor",
    ] {
        let response = request(app.clone(), "GET", &format!("{prefix}{suffix}"), COOKIE, "").await;
        assert_eq!(response.status(), StatusCode::OK, "{suffix}");
        let page = text(response).await;
        assert!(
            !page.contains("href=\"/files") && !page.contains("action=\"/settings"),
            "{suffix}: {page}"
        );
    }
    assert!(a_calls.lock().await.contains(&Rpc::History {
        path: "/Family/a & b.txt".into()
    }));
    let home = text(request(app.clone(), "GET", &format!("{prefix}/"), COOKIE, "").await).await;
    assert!(
        home.contains(&format!("src=\"{prefix}{}\"", pigeon_web::script_path()))
            && home.contains(&format!("data-events=\"{prefix}/events\""))
    );
    // The page's script is served under the mount, with the route's own
    // parameters beside the asset's name.
    let script = request(
        app.clone(),
        "GET",
        &format!("{prefix}{}", pigeon_web::script_path()),
        COOKIE,
        "",
    )
    .await;
    assert_eq!(script.status(), StatusCode::OK);
    assert!(
        script.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/javascript")
    );
    assert!(text(script).await.contains("EventSource"));
    let canonical = request(app, "GET", &prefix, COOKIE, "").await;
    assert_eq!(canonical.status(), StatusCode::SEE_OTHER);
    assert_eq!(canonical.headers()["location"], format!("{prefix}/"));
}

#[tokio::test]
async fn authentication_ownership_origin_csrf_and_revocation_guard_all_peer_routes() {
    let (_tmp, app) = app().await;
    signed_in(&app).await;
    let a = client("a", "alice", "app-a");
    let calls = peer(&app, Some(a.clone())).await;
    let prefix = format!("/clients/{}/peer", a.id);
    for headers in [
        vec![],
        vec![("Cookie", "pigeon-session=local")],
        vec![("Authorization", "Basic YWxpY2U6YXBwLWE=")],
        vec![
            ("Cookie", "pigeon-bridge-session=browser"),
            ("Origin", "https://attacker.invalid"),
        ],
    ] {
        assert_eq!(
            request(app.clone(), "GET", &format!("{prefix}/files"), &headers, "")
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let rebound = router(app.clone())
        .oneshot(
            Request::builder()
                .uri(format!("{prefix}/files"))
                .header("Host", "attacker.invalid")
                .header("Cookie", "pigeon-bridge-session=browser")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rebound.status(), StatusCode::FORBIDDEN);
    assert!(calls.lock().await.is_empty());
    for suffix in ["/files", "/events", "/settings", "/suggestions/view"] {
        assert_eq!(
            request(
                app.clone(),
                "GET",
                &format!("/clients/{}/peer{suffix}", "b".repeat(32)),
                COOKIE,
                ""
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        request(
            app.clone(),
            "POST",
            &format!("{prefix}/suggestions/accept"),
            FORM,
            "form=wrong&id=forged"
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert!(
        !calls
            .lock()
            .await
            .iter()
            .any(|r| matches!(r, Rpc::Accept { .. }))
    );
    // Forged setup/local pin routes cannot switch a managed peer's group,
    // write arbitrary folders on the host, or promise offline app copies.
    for route in [
        "join",
        "create",
        "leave",
        "files/keep",
        "files/free",
        "login",
    ] {
        assert_eq!(
            request(
                app.clone(),
                "POST",
                &format!("{prefix}/{route}"),
                FORM,
                "form=alice-form"
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    app.store
        .lock()
        .await
        .clients
        .get_mut(&a.id)
        .unwrap()
        .revoked = true;
    let before = calls.lock().await.len();
    for (method, suffix) in [
        ("GET", "/files"),
        ("GET", "/events"),
        ("GET", "/files?sort=size"),
        ("POST", "/files/view"),
        ("POST", "/settings"),
    ] {
        assert_eq!(
            request(
                app.clone(),
                method,
                &format!("{prefix}{suffix}"),
                FORM,
                "form=alice-form"
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(calls.lock().await.len(), before);
}

#[tokio::test]
async fn mismatched_identity_is_rejected_before_page_requests() {
    let (_tmp, app) = app().await;
    signed_in(&app).await;
    let mut wrong = client("a", "alice", "app-a");
    wrong.device = "somebody-else".into();
    let calls = peer(&app, Some(wrong.clone())).await;
    assert_eq!(
        request(
            app,
            "GET",
            &format!("/clients/{}/peer/files", wrong.id),
            COOKIE,
            ""
        )
        .await
        .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(
        !calls
            .lock()
            .await
            .iter()
            .any(|r| matches!(r, Rpc::Folders | Rpc::Files { .. }))
    );
}

#[tokio::test]
async fn an_open_live_stream_stops_when_its_session_expires_or_client_is_revoked() {
    for revoke in [false, true] {
        let (_tmp, app) = app().await;
        signed_in(&app).await;
        let a = client("a", "alice", "app-a");
        let calls = peer(&app, Some(a.clone())).await;
        let response = request(
            app.clone(),
            "GET",
            &format!("/clients/{}/peer/events", a.id),
            COOKIE,
            "",
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let mut stream = response.into_body().into_data_stream();
        let first = stream.next().await.unwrap().unwrap();
        assert!(String::from_utf8_lossy(&first).contains("event: headline"));
        if revoke {
            app.store
                .lock()
                .await
                .clients
                .get_mut(&a.id)
                .unwrap()
                .revoked = true;
        } else {
            app.sessions
                .lock()
                .await
                .get_mut("browser")
                .unwrap()
                .expires = 0;
        }
        let before = calls.lock().await.len();
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(15), stream.next())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            calls.lock().await.len(),
            before,
            "an open stream must not reach a revoked peer"
        );
    }
}
