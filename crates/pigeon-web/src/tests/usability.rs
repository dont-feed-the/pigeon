//! Regressions from the independent first-use and French browser sessions.

use std::sync::Mutex;

use super::*;

#[derive(Default)]
struct Recording(Mutex<Vec<pigeon_proto::Request>>);

impl Backend for Recording {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        self.0.lock().unwrap().push(request.clone());
        let answer = match request {
            pigeon_proto::Request::Status { .. } | pigeon_proto::Request::Health => {
                vec![Response::Problem(pigeon_proto::Problem {
                    code: "not-in-group".into(),
                    message: Message {
                        id: "problem-not-in-group".into(),
                        args: Default::default(),
                    },
                    hint: None,
                    details: None,
                })]
            }
            pigeon_proto::Request::Folders => vec![Response::Folders {
                root: "/Family".into(),
                folders: vec![],
            }],
            pigeon_proto::Request::Files { .. } => vec![Response::Files {
                root: "/Family".into(),
                files: vec![pigeon_proto::api::FileEntry {
                    path: "holiday.txt".into(),
                    state: State::Published,
                    size: None,
                    modified: None,
                    freed: false,
                }],
            }],
            pigeon_proto::Request::Join {
                person: Some(person),
                existing: false,
                ..
            } if person == "Ana" => vec![Response::Problem(pigeon_proto::Problem {
                code: "person-exists".into(),
                message: Message {
                    id: "problem-person-exists".into(),
                    args: [("person".to_owned(), pigeon_proto::Arg::Text(person))].into(),
                },
                hint: None,
                details: None,
            })],
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

fn recording() -> (Arc<Web>, Arc<Recording>) {
    let backend = Arc::new(Recording::default());
    (
        Web::new(backend.clone(), "en".parse().unwrap(), 4321),
        backend,
    )
}

fn form_token(page: &str) -> &str {
    page.split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
}

#[test]
fn an_action_result_keeps_its_explanation_without_live_home_updates() {
    let (web, _) = recording();
    let renderer = web.renderer();
    let title = renderer.id("problem-join-history-unverified");
    let result = crate::pages::result(&renderer, "/", vec![title.clone()]).0;
    assert!(result.contains(&title));
    assert!(!result.contains("EventSource"));
    assert!(!result.contains("id=\"headline\""));
}

#[tokio::test]
async fn root_files_link_to_versions_instead_of_an_empty_folder_message() {
    let (w, _) = recording();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = body(
        app.oneshot(get("/files", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(
        page.contains("holiday.txt") && page.contains("/file?path=%2FFamily%2Fholiday.txt"),
        "{page}"
    );
    assert!(!page.contains("no folder yet"), "{page}");
}

#[tokio::test]
async fn a_new_computer_can_discover_setup_and_forged_forms_do_not_join() {
    let (w, backend) = recording();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let home = body(
        app.clone()
            .oneshot(get("/", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(
        home.contains("href=\"/join\"") && home.contains("Join or create a group"),
        "{home}"
    );
    let page = body(
        app.clone()
            .oneshot(get("/join", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(page.contains("action=\"/join\"") && page.contains("action=\"/create\""));
    let bad = app
        .clone()
        .oneshot(post(
            "/join",
            &cookie,
            "form=bad&invitation=pgn2-any&folder=%2FFamily",
        ))
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::FORBIDDEN);
    assert!(
        !backend
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|r| matches!(r, pigeon_proto::Request::Join { .. }))
    );
    let good = format!(
        "form={}&invitation=pgn2-any&folder=%2FFamily&name=laptop",
        form_token(&page)
    );
    let result = app.oneshot(post("/join", &cookie, &good)).await.unwrap();
    assert_eq!(result.status(), StatusCode::OK);
    assert!(backend.0.lock().unwrap().iter().any(|r| matches!(r, pigeon_proto::Request::Join { folder: Some(f), name: Some(n), .. } if f == "/Family" && n == "laptop")));
}

#[tokio::test]
async fn joining_a_refounded_group_waits_for_the_second_confirmation() {
    let (w, backend) = recording();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = body(
        app.clone()
            .oneshot(get("/join", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    let code = invitation(Some(pigeon_proto::join::Refounded {
        from: "old".into(),
        by: "Ana".into(),
        at: "2026-10-15T12:00:00Z".into(),
    }));
    let fields = format!(
        "form={}&invitation={code}&folder=%2FFamily",
        form_token(&page)
    );
    let answer = body(
        app.clone()
            .oneshot(post("/join", &cookie, &fields))
            .await
            .unwrap(),
    )
    .await;
    assert!(
        answer.contains("Ana") && answer.contains("name=\"yes\" value=\"true\""),
        "{answer}"
    );
    assert!(
        !backend
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|r| matches!(r, pigeon_proto::Request::Join { .. }))
    );
    let confirmed = format!("{fields}&yes=true");
    app.oneshot(post("/join", &cookie, &confirmed))
        .await
        .unwrap();
    assert_eq!(
        backend
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|r| matches!(r, pigeon_proto::Request::Join { .. }))
            .count(),
        1
    );
}

#[tokio::test]
async fn joining_as_a_person_of_the_group_waits_for_the_second_confirmation() {
    let (w, backend) = recording();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = body(
        app.clone()
            .oneshot(get("/join", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(page.contains("Your name in the group"), "{page}");
    let fields = format!(
        "form={}&invitation={}&folder=%2FFamily&person=Ana",
        form_token(&page),
        invitation(None)
    );
    let answer = body(
        app.clone()
            .oneshot(post("/join", &cookie, &fields))
            .await
            .unwrap(),
    )
    .await;
    assert!(
        answer.contains("Ana is already in") && answer.contains("name=\"existing\" value=\"true\""),
        "{answer}"
    );
    app.oneshot(post("/join", &cookie, &format!("{fields}&existing=true")))
        .await
        .unwrap();
    let joins: Vec<bool> = backend
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|r| match r {
            pigeon_proto::Request::Join { existing, .. } => Some(*existing),
            _ => None,
        })
        .collect();
    assert_eq!(joins, [false, true]);
}

#[tokio::test]
async fn diagnostic_disclosure_requires_a_checked_local_consent_and_valid_form() {
    let (web, backend) = recording();
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = body(
        app.clone()
            .oneshot(get("/settings", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    let form = form_token(&page);
    for (fields, status) in [
        (format!("form={form}&peer=peer"), StatusCode::BAD_REQUEST),
        (
            format!("form={form}&peer=peer&consent=false"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "form=forged&peer=peer&consent=true".to_owned(),
            StatusCode::FORBIDDEN,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(post("/settings/diagnostics", &cookie, &fields))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    assert!(
        !backend
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|r| matches!(r, pigeon_proto::Request::AllowDebug { .. }))
    );
    app.oneshot(post(
        "/settings/diagnostics",
        &cookie,
        &format!("form={form}&peer=peer&consent=true"),
    ))
    .await
    .unwrap();
    assert_eq!(
        backend
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|r| matches!(r, pigeon_proto::Request::AllowDebug { .. }))
            .count(),
        1
    );
}

#[tokio::test]
async fn suggestion_review_uses_the_exact_proposal_path_and_checked_form() {
    let (web, backend) = recording();
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = body(
        app.clone()
            .oneshot(get("/join", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    let token = form_token(&page);
    backend.0.lock().unwrap().clear();
    let params = "proposal=c0ffee&path=Photos%2F2019%2Fa.jpg";
    let response = app
        .clone()
        .oneshot(post(
            "/suggestions/view",
            &cookie,
            &format!("form=forged&{params}"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(backend.0.lock().unwrap().is_empty());
    let response = app
        .clone()
        .oneshot(post(
            "/suggestions/view",
            &cookie,
            &format!("form={token}&{params}"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        backend.0.lock().unwrap().as_slice(),
        &[pigeon_proto::Request::ViewSuggestion {
            proposal: "c0ffee".into(),
            path: "Photos/2019/a.jpg".into(),
        }]
    );
    backend.0.lock().unwrap().clear();
    let response = app
        .oneshot(post(
            "/suggestions/view",
            &cookie,
            &format!("form={token}&{params}&version=1"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(backend.0.lock().unwrap().is_empty());
}
