//! Erasure has separate read and write boundaries: GET never submits,
//! CSRF failures make no backend call, and exact history tokens survive
//! the form. English/French pages escape names and explain shared bytes.

use super::*;
use pigeon_proto::erase::{Answer as Result, Operation, Preview, Readiness, Selection, Waiting};
use std::sync::Mutex;

struct Erase {
    calls: Mutex<Vec<pigeon_proto::Request>>,
    ready: bool,
    split: bool,
}
impl Backend for Erase {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        self.calls.lock().unwrap().push(request.clone());
        let result = match request {
            pigeon_proto::Request::Erase {
                operation: Operation::Preview { selection },
            } => Result::Preview(Preview {
                selection,
                token: "exact-history-token".into(),
                current_files: 1,
                versions: 2,
                bytes: 42,
                shared_versions: 1,
                retained_elsewhere: vec!["Other/<script>.txt".into()],
                retention_days: 30,
                needs_approval: false,
                readiness: Readiness {
                    ready: self.ready,
                    can_prepare: true,
                    preparation_needed: !self.ready,
                    waiting: vec![Waiting {
                        device: "b".repeat(64),
                        name: "Backup <script>".into(),
                    }],
                    open_decisions: Vec::new(),
                    problem: self.split.then(|| Message {
                        id: "erase-preparation-split".into(),
                        args: Default::default(),
                    }),
                },
            }),
            _ => Result::Submitted {
                id: "committed".into(),
                needs_approval: false,
            },
        };
        Box::pin(async move {
            vec![Response::Erasure {
                answer: Box::new(result),
            }]
        })
    }
}

#[tokio::test]
async fn erase_get_only_previews_and_post_requires_both_form_and_history_tokens() {
    for lang in ["en", "fr"] {
        let backend = Arc::new(Erase {
            calls: Mutex::new(Vec::new()),
            ready: true,
            split: false,
        });
        let web = Web::new(backend.clone(), lang.parse().unwrap(), 4321);
        let app = web.router();
        let cookie = logged_in(&app, &web).await;
        let html = body(
            app.clone()
                .oneshot(get("/erase?path=%2FFamily%2Fold.txt", HOST, Some(&cookie)))
                .await
                .unwrap(),
        )
        .await;
        assert!(html.contains("exact-history-token"));
        assert!(html.contains("Other/&lt;script&gt;.txt") && !html.contains("Other/<script>.txt"));
        assert!(html.contains("value=\"confirm\""));
        assert_eq!(
            backend.calls.lock().unwrap().as_slice(),
            &[pigeon_proto::Request::Erase {
                operation: Operation::Preview {
                    selection: Selection::Path("/Family/old.txt".into())
                }
            }]
        );
        let token = html
            .split("name=\"form\" value=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        backend.calls.lock().unwrap().clear();
        let forged = app
            .clone()
            .oneshot(post(
                "/erase",
                &cookie,
                "form=forged&history=exact-history-token&path=%2FFamily%2Fold.txt&action=confirm",
            ))
            .await
            .unwrap();
        assert_eq!(forged.status(), StatusCode::FORBIDDEN);
        assert!(backend.calls.lock().unwrap().is_empty());
        let missing = app
            .clone()
            .oneshot(post(
                "/erase",
                &cookie,
                &format!("form={token}&path=%2FFamily%2Fold.txt&action=confirm"),
            ))
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(backend.calls.lock().unwrap().is_empty());
        let form = format!(
            "form={token}&history=exact-history-token&path=%2FFamily%2Fold.txt&action=confirm"
        );
        assert_eq!(
            app.oneshot(post("/erase", &cookie, &form))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            backend.calls.lock().unwrap().as_slice(),
            &[pigeon_proto::Request::Erase {
                operation: Operation::Confirm {
                    selection: Selection::Path("/Family/old.txt".into()),
                    token: "exact-history-token".into()
                }
            }]
        );
    }
}

#[tokio::test]
async fn unready_erase_offers_preparation_without_any_confirmation_form() {
    let backend = Arc::new(Erase {
        calls: Mutex::new(Vec::new()),
        ready: false,
        split: false,
    });
    let web = Web::new(backend.clone(), "fr".parse().unwrap(), 4321);
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let html = body(
        app.clone()
            .oneshot(get("/erase?suggestion=abcd", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(html.contains("Backup &lt;script&gt;"));
    assert!(html.contains("value=\"prepare\""));
    assert!(!html.contains("value=\"confirm\""));
    let token = html
        .split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    backend.calls.lock().unwrap().clear();
    let form = format!("form={token}&history=exact-history-token&suggestion=abcd&action=prepare");
    assert_eq!(
        app.clone()
            .oneshot(post("/erase", &cookie, &form))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        backend.calls.lock().unwrap().as_slice(),
        &[pigeon_proto::Request::Erase {
            operation: Operation::Prepare {
                selection: Selection::Suggestion("abcd".into())
            }
        }]
    );
    backend.calls.lock().unwrap().clear();
    let malformed = format!(
        "form={token}&history=exact-history-token&path=%2FFamily&suggestion=abcd&action=confirm"
    );
    assert_eq!(
        app.oneshot(post("/erase", &cookie, &malformed))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert!(backend.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn split_preparation_explains_explicit_recovery_without_retry_or_erasure_buttons() {
    for lang in ["en", "fr"] {
        let backend = Arc::new(Erase {
            calls: Mutex::new(Vec::new()),
            ready: false,
            split: true,
        });
        let web = Web::new(backend.clone(), lang.parse().unwrap(), 4321);
        let app = web.router();
        let cookie = logged_in(&app, &web).await;
        let html = body(
            app.oneshot(get("/erase?path=%2FFamily%2Fold.txt", HOST, Some(&cookie)))
                .await
                .unwrap(),
        )
        .await;
        assert!(html.contains("pigeon refound"));
        assert!(
            !html.contains("erase-preparation-split"),
            "the message must be translated"
        );
        assert!(!html.contains("value=\"prepare\"") && !html.contains("value=\"confirm\""));
        assert!(
            !html.contains("Erasing is waiting for the current computers")
                && !html.contains("L’effacement attend la fin de la préparation")
        );
        assert!(matches!(
            backend.calls.lock().unwrap().as_slice(),
            [pigeon_proto::Request::Erase {
                operation: Operation::Preview { .. }
            }]
        ));
    }
}

#[tokio::test]
async fn mounted_erase_keeps_review_forms_and_results_in_the_selected_peer() {
    let prefix = "/clients/selected/peer";
    let backend = Arc::new(Erase {
        calls: Mutex::new(Vec::new()),
        ready: true,
        split: false,
    });
    let web = Web::embedded(
        backend.clone(),
        "fr".parse().unwrap(),
        crate::Mount {
            path: prefix.into(),
            label: "Client <script>".into(),
            back: "/clients/selected".into(),
            storage_controls: false,
        },
    );
    let app = web.embedded_router("bridge-csrf".into());
    let html = body(
        app.clone()
            .oneshot(get(
                "/erase?path=%2FFamily%2Fold.txt",
                "bridge.example",
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(html.contains("Client &lt;script&gt;"));
    assert!(html.contains(&format!("action=\"{prefix}/erase\"")));
    for attribute in ["href=\"", "action=\""] {
        for value in html
            .split(attribute)
            .skip(1)
            .map(|rest| rest.split('"').next().unwrap())
        {
            assert!(
                value.starts_with(prefix)
                    || value == "/clients/selected"
                    // The file manager's accessibility link stays on this page.
                    || value.starts_with('#'),
                "{attribute}{value}"
            );
        }
    }
    backend.calls.lock().unwrap().clear();
    let form =
        "form=bridge-csrf&history=exact-history-token&path=%2FFamily%2Fold.txt&action=confirm";
    let response = app.oneshot(post("/erase", "", form)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = body(response).await;
    assert!(html.contains(&format!("href=\"{prefix}/erase\"")));
    assert!(!html.contains("pigeon erase"));
    assert!(!html.contains("/erase?path="));
    assert_eq!(
        backend.calls.lock().unwrap().as_slice(),
        &[pigeon_proto::Request::Erase {
            operation: Operation::Confirm {
                selection: Selection::Path("/Family/old.txt".into()),
                token: "exact-history-token".into()
            }
        }]
    );
}
