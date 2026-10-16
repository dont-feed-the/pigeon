//! Folder form boundaries: CSRF, repeated steward selections, escaped
//! member names and read-only controls (10 §10.3).
use super::*;
use pigeon_proto::policy;
use std::sync::Mutex;

struct Rules {
    calls: Mutex<Vec<pigeon_proto::Request>>,
    editable: bool,
}
impl Backend for Rules {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        self.calls.lock().unwrap().push(request.clone());
        let answer = match request {
            pigeon_proto::Request::FolderRules { .. } => {
                vec![Response::FolderRules(Box::new(policy::Report {
                    root: "/Family".into(),
                    can_edit: self.editable,
                    rules: vec![policy::Rule {
                        path: "Photos".into(),
                        owner: None,
                        stewards: Vec::new(),
                        direct: true,
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
        app.oneshot(post("/folder-rule", &cookie, &submitted))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        backend.calls.lock().unwrap().as_slice(),
        &[pigeon_proto::Request::SetFolderRule {
            folder: "/Family/Photos".into(),
            rule: policy::Change::AddOnly {
                stewards: vec!["alice".into(), "bob".into()]
            },
        }]
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
