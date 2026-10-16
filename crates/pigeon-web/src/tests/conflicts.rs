//! A file whose copy here differs from the group's version (07 §7.6,
//! ux.md §8.8): listed on Home, explained on its page with the choices of
//! `pigeon resolve`, each behind the form token.

use super::*;
use pigeon_proto::api::{Choice, Divergence};
use std::sync::Mutex;

/// `pigeon conflicts` with `tarte.docx` in `state`.
pub(super) fn tarte(state: &str) -> Vec<Response> {
    vec![Response::Conflicts {
        conflicts: vec![Divergence {
            path: "tarte.docx".into(),
            kind: "suggestion".into(),
            state: state.into(),
            since: "2026-10-15T10:00:00Z".into(),
        }],
    }]
}

/// The test backend, but with the suggestion of `tarte.docx` still
/// waiting, and every `Resolve` recorded.
struct Waiting(Mutex<Vec<Choice>>);

impl Backend for Waiting {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        match request {
            pigeon_proto::Request::Conflicts => Box::pin(async { tarte("waiting") }),
            pigeon_proto::Request::Resolve { choice, .. } => {
                self.0.lock().unwrap().push(choice);
                Box::pin(async { vec![Response::Done] })
            }
            other => Fixed.request(other),
        }
    }
}

#[tokio::test]
async fn home_lists_a_differing_file_with_a_link_to_its_page() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let home = body(app.oneshot(get("/", HOST, Some(&cookie))).await.unwrap()).await;
    assert!(
        home.contains("Files that differ from the group's version"),
        "{home}"
    );
    assert!(
        home.contains("href=\"/file?path=%2Fg%2Ftarte.docx\""),
        "{home}"
    );
    assert!(
        home.contains("tarte.docx: your copy here is different"),
        "{home}"
    );
}

#[tokio::test]
async fn home_says_nothing_of_differences_when_there_are_none() {
    struct None_;
    impl Backend for None_ {
        fn request(&self, request: pigeon_proto::Request) -> Answer {
            match request {
                pigeon_proto::Request::Conflicts => {
                    Box::pin(async { vec![Response::Conflicts { conflicts: vec![] }] })
                }
                other => Fixed.request(other),
            }
        }
    }
    let w = Web::new(Arc::new(None_), "en".parse().unwrap(), 4321);
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let home = body(app.oneshot(get("/", HOST, Some(&cookie))).await.unwrap()).await;
    assert!(!home.contains("differ"), "{home}");
}

#[tokio::test]
async fn a_differing_file_offers_each_choice_and_says_where_both_versions_go() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = body(
        app.clone()
            .oneshot(get("/file?path=%2Fg%2Ftarte.docx", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(
        page.contains("Your copy of tarte.docx is different from the group's version."),
        "{page}"
    );
    for label in [
        "Take the group's version",
        "Keep mine as a copy",
        "Suggest again",
    ] {
        assert!(
            page.contains(&format!("{label}</button>")),
            "{label}: {page}"
        );
    }
    assert!(page.contains("pigeon keeps it in its history"), "{page}");
    assert!(page.contains("value=\"tarte (my copy).docx\""), "{page}");
    assert_eq!(page.matches("the group's current version").count(), 1);

    let forged = app
        .clone()
        .oneshot(post(
            "/files/resolve",
            &cookie,
            "form=00&path=%2Fg%2Ftarte.docx&choice=take-group",
        ))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);

    let form = form_of(&page);
    let done = app
        .oneshot(post(
            "/files/resolve",
            &cookie,
            &format!(
                "form={form}&path=%2Fg%2Ftarte.docx&choice=keep-copy&dest=tarte+%28Ana%29.docx"
            ),
        ))
        .await
        .unwrap();
    assert_eq!(done.status(), StatusCode::OK);
    let done = body(done).await;
    assert!(done.contains("/g/tarte (Ana).docx"), "{done}");
    assert!(
        done.contains("href=\"/file?path=%2Fg%2Ftarte.docx\""),
        "the result links back to the file: {done}"
    );
}

#[tokio::test]
async fn a_waiting_suggestion_can_be_withdrawn_but_not_suggested_again() {
    let backend = Arc::new(Waiting(Mutex::new(Vec::new())));
    let w = Web::new(backend.clone(), "fr".parse().unwrap(), 4321);
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = body(
        app.clone()
            .oneshot(get("/file?path=%2Fg%2Ftarte.docx", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(page.contains("attend une décision"), "{page}");
    assert!(page.contains("Votre suggestion est retirée"), "{page}");
    assert!(!page.contains("Suggérer à nouveau"), "{page}");
    assert!(page.contains("value=\"tarte (ma copie).docx\""), "{page}");
    let form = form_of(&page);
    let done = app
        .oneshot(post(
            "/files/resolve",
            &cookie,
            &format!("form={form}&path=%2Fg%2Ftarte.docx&choice=keep-copy&dest="),
        ))
        .await
        .unwrap();
    assert_eq!(done.status(), StatusCode::OK);
    assert_eq!(
        *backend.0.lock().unwrap(),
        [Choice::KeepCopy {
            dest: "/g/tarte (ma copie).docx".into()
        }]
    );
}

#[tokio::test]
async fn a_file_that_matches_the_group_marks_its_current_version_only() {
    let w = web();
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = body(
        app.oneshot(get("/file?path=%2Fg%2Fother.txt", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(!page.contains("/files/resolve"), "{page}");
    assert!(page.contains("<strong>current version</strong>"), "{page}");
    assert_eq!(page.matches("current version").count(), 1, "{page}");
}

#[tokio::test]
async fn a_bridge_client_resolves_on_the_bridge_not_on_its_mounted_pages() {
    let app = Web::embedded(
        Arc::new(Fixed),
        "en".parse().unwrap(),
        crate::Mount {
            path: "/clients/selected/peer".into(),
            label: "Phone".into(),
            back: "/clients/selected".into(),
            storage_controls: false,
        },
    )
    .embedded_router("bridge-csrf".into());
    for path in ["/", "/file?path=%2Fg%2Ftarte.docx"] {
        let page = body(
            app.clone()
                .oneshot(get(path, "bridge.example", None))
                .await
                .unwrap(),
        )
        .await;
        assert!(!page.contains("differ"), "{path}: {page}");
        assert!(!page.contains("/files/resolve"), "{path}: {page}");
    }
}
