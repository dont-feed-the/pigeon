//! Regressions from the browser usability study: a refused join or create
//! keeps what was typed, folders and pasted invitations are read as a
//! person means them, and every page has its own title.

use std::sync::Mutex;

use super::*;

/// Refuses every join and create, and records them.
#[derive(Default)]
struct Refusing(Mutex<Vec<pigeon_proto::Request>>);

impl Backend for Refusing {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        self.0.lock().unwrap().push(request.clone());
        let answer = match request {
            pigeon_proto::Request::Join { .. } | pigeon_proto::Request::Create { .. } => {
                vec![Response::Problem(pigeon_proto::Problem {
                    code: "invite-unknown".into(),
                    message: Message {
                        id: "problem-invite-unknown".into(),
                        args: Default::default(),
                    },
                    hint: None,
                    details: None,
                })]
            }
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

async fn setup() -> (axum::Router, Arc<Refusing>, String, String) {
    let backend = Arc::new(Refusing::default());
    let w = Web::new(backend.clone(), "en".parse().unwrap(), 4321);
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = body(
        app.clone()
            .oneshot(get("/join", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    let form = page
        .split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    (app, backend, cookie, form)
}

#[tokio::test]
async fn a_refused_join_shows_the_forms_again_with_what_was_typed() {
    let (app, _, cookie, form) = setup().await;
    let page = body(
        app.oneshot(post(
            "/join",
            &cookie,
            &format!("form={form}&invitation=pgn1-nope&folder=%2Fhome%2Fc%2FFamily&name=carol"),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert!(page.contains("not one the group knows"), "{page}");
    assert!(page.contains("action=\"/join\"") && page.contains("action=\"/create\""));
    assert!(page.contains("value=\"pgn1-nope\"") && page.contains("value=\"/home/c/Family\""));
    assert!(page.contains("value=\"carol\""), "{page}");
}

#[tokio::test]
async fn a_home_folder_is_expanded_and_names_and_invitations_are_cleaned() {
    let (app, backend, cookie, form) = setup().await;
    let home = std::env::var("HOME").unwrap();
    let json = br#"{"group":"g","name":"Family"}"#;
    let code = format!(
        "pgn1-{}",
        data_encoding::BASE32_NOPAD.encode(json).to_lowercase()
    );
    let (a, b) = code.split_at(12);
    let wrapped = format!("{a}%0A{b}");
    app.clone()
        .oneshot(post(
            "/join",
            &cookie,
            &format!("form={form}&invitation={wrapped}&folder=~%2FFamily"),
        ))
        .await
        .unwrap();
    app.oneshot(post(
        "/create",
        &cookie,
        &format!("form={form}&group=+Family+&folder=~&name=+laptop+&k=abc"),
    ))
    .await
    .unwrap();
    let sent = backend.0.lock().unwrap();
    assert!(
        sent.iter().any(|r| matches!(r,
        pigeon_proto::Request::Join { invitation, folder: Some(f), .. }
            if *invitation == code && *f == format!("{home}/Family"))),
        "{sent:?}"
    );
    assert!(
        sent.iter().any(|r| matches!(r,
        pigeon_proto::Request::Create { name, folder, k: 0, device_name }
            if name == "Family" && *folder == home && device_name == "laptop")),
        "{sent:?}"
    );
}

#[tokio::test]
async fn an_unknown_page_says_so_and_every_page_has_its_own_title() {
    let (app, _, cookie, _) = setup().await;
    let missing = app
        .clone()
        .oneshot(get("/nope", HOST, Some(&cookie)))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let missing = body(missing).await;
    assert!(missing.contains("There is no page here.") && missing.contains("<nav"));
    let settings = body(
        app.oneshot(get("/settings", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(
        settings.contains("<title>Settings – pigeon</title>"),
        "{settings}"
    );
}
