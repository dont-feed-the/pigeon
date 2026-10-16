//! Browser diagnostics use the normal authenticated read-only Doctor
//! request and render the localized remedy without exposing raw details.
use super::*;

struct MissingHelper;
impl Backend for MissingHelper {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        let responses = match request {
            pigeon_proto::Request::Doctor { details: false } => {
                vec![Response::Problem(pigeon_proto::Problem {
                    code: "transport-helper-unavailable".into(),
                    message: Message {
                        id: "problem-transport-helper-unavailable".into(),
                        args: Default::default(),
                    },
                    hint: None,
                    details: Some("private executable detail".into()),
                })]
            }
            _ => vec![Response::Done],
        };
        Box::pin(async move { responses })
    }
}

#[tokio::test]
async fn settings_links_to_an_authenticated_localized_installation_check() {
    let web = Web::new(Arc::new(MissingHelper), "fr".parse().unwrap(), 4321);
    let app = web.router();
    let refused = app
        .clone()
        .oneshot(get("/doctor", HOST, None))
        .await
        .unwrap();
    assert_ne!(refused.status(), StatusCode::OK);
    let cookie = logged_in(&app, &web).await;
    let settings = body(
        app.clone()
            .oneshot(get("/settings", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(settings.contains("href=\"/doctor\""));
    let page = body(
        app.oneshot(get("/doctor", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(page.contains("P-TRANSPORT-HELPER-UNAVAILABLE") && page.contains("Réinstallez"));
    assert!(
        !page.contains("private executable detail")
            && !page.contains("Tout ce dont pigeon a besoin est en place")
    );
}
