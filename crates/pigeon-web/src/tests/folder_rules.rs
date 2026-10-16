//! Folder form boundaries: CSRF, repeated steward selections, escaped
//! member names and read-only controls (10 §10.3).
use super::*;
use pigeon_proto::policy;
use std::sync::Mutex;

struct Rules {
    calls: Mutex<Vec<pigeon_proto::Request>>,
    editable: bool,
}

pub(super) async fn write_pages(dir: &std::path::Path, lang: &str) {
    for editable in [true, false] {
        let web = Web::new(
            Arc::new(Rules {
                calls: Mutex::new(Vec::new()),
                editable,
            }),
            lang.parse().unwrap(),
            4321,
        );
        let app = web.router();
        let cookie = logged_in(&app, &web).await;
        let page = app
            .oneshot(get(
                "/folder-rule?path=%2FFamily%2FPhotos",
                HOST,
                Some(&cookie),
            ))
            .await
            .unwrap();
        std::fs::write(
            dir.join(format!("{lang}-folder-rule-{editable}.html")),
            body(page).await,
        )
        .unwrap();
    }
}
impl Backend for Rules {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        self.calls.lock().unwrap().push(request.clone());
        let answer = match request {
            pigeon_proto::Request::Files { .. } => vec![Response::Files {
                root: "/Family".into(),
                files: [
                    "Photos/Enfant/graine.txt",
                    "Photos/Enfant/deep/note.txt",
                    "Photos/a.txt",
                    "Photos-old/no.txt",
                ]
                .into_iter()
                .map(|path| pigeon_proto::api::FileEntry {
                    path: path.into(),
                    state: pigeon_proto::api::State::Published,
                })
                .collect(),
            }],
            pigeon_proto::Request::FolderRules { .. } => {
                vec![Response::FolderRules(Box::new(policy::Report {
                    root: "/Family".into(),
                    can_edit: self.editable,
                    rules: vec![policy::Rule {
                        path: "Photos".into(),
                        owner: None,
                        stewards: Vec::new(),
                        direct: true,
                        per_person: false,
                    }],
                    people: vec![policy::Person {
                        id: "alice".into(),
                        name: "Alice <script>".into(),
                        this_person: true,
                        admin: true,
                    }],
                }))]
            }
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

#[tokio::test]
async fn nested_folders_have_discoverable_links_to_their_own_controls() {
    let web = Web::new(
        Arc::new(Rules {
            calls: Mutex::new(Vec::new()),
            editable: true,
        }),
        "fr".parse().unwrap(),
        4321,
    );
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = body(
        app.oneshot(get("/folder?path=%2FFamily%2FPhotos", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(page.contains("Sous-dossiers"));
    assert_eq!(
        page.matches("href=\"/folder?path=%2FFamily%2FPhotos%2FEnfant\"")
            .count(),
        1
    );
    assert!(!page.contains("href=\"/folder?path=%2FFamily%2FPhotos%2FEnfant%2Fdeep\""));
    assert!(!page.contains("href=\"/folder?path=%2FFamily%2FPhotos-old"));
    assert!(page.contains("href=\"/folder-rule?path=%2FFamily%2FPhotos\""));
}

#[tokio::test]
async fn folder_rules_escape_names_and_need_a_single_checked_form() {
    let backend = Arc::new(Rules {
        calls: Mutex::new(Vec::new()),
        editable: true,
    });
    let web = Web::new(backend.clone(), "en".parse().unwrap(), 4321);
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = body(
        app.clone()
            .oneshot(get(
                "/folder-rule?path=%2FFamily%2FPhotos",
                HOST,
                Some(&cookie),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(page.contains("Alice &lt;script&gt;") && !page.contains("Alice <script>"));
    let token = page
        .split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    backend.calls.lock().unwrap().clear();
    let response = app
        .clone()
        .oneshot(post(
            "/folder-rule",
            &cookie,
            "form=forged&path=%2FFamily%2FPhotos&kind=owned&owner=alice",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(backend.calls.lock().unwrap().is_empty());
    let submitted =
        format!("form={token}&path=%2FFamily%2FPhotos&kind=add-only&steward=alice&steward=bob");
    assert_eq!(
        app.clone()
            .oneshot(post("/folder-rule", &cookie, &submitted))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    // A per-person rule says it may raise the group's rules only with the box.
    assert!(page.contains("value=\"per-person\"") && page.contains("name=\"confirm\""));
    for (extra, confirmed) in [("", false), ("&confirm=rules", true)] {
        let submitted = format!("form={token}&path=%2FFamily%2FPhotos&kind=per-person{extra}");
        assert_eq!(
            app.clone()
                .oneshot(post("/folder-rule", &cookie, &submitted))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            backend.calls.lock().unwrap().last(),
            Some(&pigeon_proto::Request::SetFolderRule {
                folder: "/Family/Photos".into(),
                rule: policy::Change::PerPerson,
                confirmed,
            })
        );
    }
    assert_eq!(
        backend.calls.lock().unwrap().first(),
        Some(&pigeon_proto::Request::SetFolderRule {
            folder: "/Family/Photos".into(),
            rule: policy::Change::AddOnly {
                stewards: vec!["alice".into(), "bob".into()]
            },
            confirmed: false,
        })
    );
}

#[tokio::test]
async fn an_ordinary_member_sees_the_rule_without_a_write_form() {
    let web = Web::new(
        Arc::new(Rules {
            calls: Mutex::new(Vec::new()),
            editable: false,
        }),
        "fr".parse().unwrap(),
        4321,
    );
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = body(
        app.oneshot(get(
            "/folder-rule?path=%2FFamily%2FPhotos",
            HOST,
            Some(&cookie),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert!(page.contains("Un administrateur peut modifier"));
    assert!(!page.contains("action=\"/folder-rule\""));
}

#[tokio::test]
async fn nested_free_and_keep_submit_only_the_selected_folder_with_csrf() {
    let backend = Arc::new(Rules {
        calls: Mutex::new(Vec::new()),
        editable: true,
    });
    let web = Web::new(backend.clone(), "fr".parse().unwrap(), 4321);
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let html = body(
        app.clone()
            .oneshot(get(
                "/folder?path=%2FFamily%2FPhotos%2FEnfant",
                HOST,
                Some(&cookie),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(html.contains("action=\"/files/free\"") && html.contains("action=\"/files/keep\""));
    assert_eq!(
        html.matches("name=\"folder\" value=\"/Family/Photos/Enfant\"")
            .count(),
        2
    );
    let token = html
        .split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    backend.calls.lock().unwrap().clear();
    let forbidden = app
        .clone()
        .oneshot(post(
            "/files/free",
            &cookie,
            "form=wrong&folder=%2FFamily%2FPhotos%2FEnfant",
        ))
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    assert!(backend.calls.lock().unwrap().is_empty());
    let form = format!("form={token}&folder=%2FFamily%2FPhotos%2FEnfant");
    for action in ["/files/free", "/files/keep"] {
        assert_eq!(
            app.clone()
                .oneshot(post(action, &cookie, &form))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        backend.calls.lock().unwrap().as_slice(),
        &[
            pigeon_proto::Request::Evict {
                folder: "/Family/Photos/Enfant".into()
            },
            pigeon_proto::Request::Keep {
                folder: "/Family/Photos/Enfant".into()
            },
        ]
    );
}
