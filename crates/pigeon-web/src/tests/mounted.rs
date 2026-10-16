//! The regular pages keep navigation, forms and live updates in their mount;
//! local authentication stays separate and admin review keeps its reason.

use super::*;
use crate::Mount;
use std::sync::Mutex;

const PREFIX: &str = "/clients/selected/peer";

#[tokio::test]
async fn mounted_pages_keep_all_urls_scoped_and_escape_peer_names() {
    for lang in ["en", "fr"] {
        let w = Web::embedded(
            Arc::new(Fixed),
            lang.parse().unwrap(),
            Mount {
                path: PREFIX.into(),
                label: "Phone <script> & laptop".into(),
                back: "/clients/selected".into(),
                storage_controls: false,
            },
        );
        let app = w.embedded_router("bridge-csrf".into());
        for path in [
            "/",
            "/files",
            "/folder?path=%2FFamily",
            "/file?path=%2FFamily%2Fa+%26+b.txt",
            "/deleted",
            "/space",
            "/suggestions",
            "/machines",
            "/settings",
            "/doctor",
            "/folder-rule?path=%2FFamily",
        ] {
            let response = app
                .clone()
                .oneshot(get(path, "bridge.example", None))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{lang} {path}");
            let page = body(response).await;
            assert!(page.contains("Phone &lt;script&gt; &amp; laptop"), "{page}");
            for attribute in ["href=\"", "action=\"", "src=\"", "data-events=\""] {
                for value in page
                    .split(attribute)
                    .skip(1)
                    .map(|rest| rest.split('"').next().unwrap())
                {
                    assert!(
                        value.starts_with(PREFIX)
                            || value.starts_with('#')
                            || value == "/clients/selected",
                        "{path}: {attribute}{value}"
                    );
                }
            }
            assert!(!page.contains("/join\"") && !page.contains("/leave\""));
            assert!(!page.contains("/files/keep\"") && !page.contains("/files/free\""));
        }
        let forged = Request::post("/suggestions/accept")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("form=wrong&id=proposal"))
            .unwrap();
        assert_eq!(
            app.oneshot(forged).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
}

#[derive(Default)]
struct Review(Mutex<Vec<pigeon_proto::Request>>);

impl Backend for Review {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        self.0.lock().unwrap().push(request.clone());
        let answer = match request {
            pigeon_proto::Request::Proposals => Response::Proposals {
                proposals: vec![Suggestion {
                    id: "forced".into(),
                    author: "Bob".into(),
                    device: "bob".into(),
                    saved_at: "2026-10-16T00:00:00Z".into(),
                    paths: vec!["Photos/a.jpg".into()],
                    decide: true,
                    force: true,
                    mine: true,
                }],
            },
            _ => Response::Done,
        };
        Box::pin(async move { vec![answer] })
    }
}

#[tokio::test]
async fn shared_admin_review_requires_and_forwards_the_override_reason() {
    let backend = Arc::new(Review::default());
    let web = Web::new(backend.clone(), "en".parse().unwrap(), 4321);
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = body(
        app.clone()
            .oneshot(get("/suggestions", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(page.contains("name=\"reason\" required"));
    let form = page
        .split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let request = Request::post("/suggestions/accept")
        .header(header::HOST, HOST)
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!(
            "form={form}&id=forced&reason=Restore+the+family+photo"
        )))
        .unwrap();
    assert_eq!(app.oneshot(request).await.unwrap().status(), StatusCode::OK);
    assert!(
        backend
            .0
            .lock()
            .unwrap()
            .contains(&pigeon_proto::Request::Accept {
                proposal: "forced".into(),
                reason: Some("Restore the family photo".into()),
            })
    );
}
