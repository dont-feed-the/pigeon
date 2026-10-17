//! Which folders are on this computer, through the router (08 §8.5, 10
//! §10.3): the browser says where each folder is and whose it is, the tree
//! of *Folders on this computer* turns its boxes into the fewest requests,
//! and *Use the default* forgets a choice, each only with the form token.

use std::sync::Mutex;

use super::demo::{Demo, ROOT};
use super::*;

#[derive(Default)]
struct Recorded(Mutex<Vec<pigeon_proto::Request>>, bool);

impl Backend for Recorded {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        self.0.lock().unwrap().push(request.clone());
        if self.1 && matches!(request, pigeon_proto::Request::FreePreview { .. }) {
            return Box::pin(async {
                vec![Response::FreePreview(pigeon_proto::api::FreePreview {
                    files: 6,
                    safe: 6,
                    backups: vec!["quiet-nas".into()],
                    heard_at: Some(pigeon_proto::time::rfc3339(crate::now() - 7200)),
                    quiet: true,
                })]
            });
        }
        Demo.request(request)
    }
}

impl Recorded {
    fn changes(&self) -> Vec<pigeon_proto::Request> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    pigeon_proto::Request::Keep { .. }
                        | pigeon_proto::Request::Evict { .. }
                        | pigeon_proto::Request::ForgetChoice { .. }
                )
            })
            .cloned()
            .collect()
    }
}

fn enc(s: &str) -> String {
    crate::browse::enc(s)
}

async fn session() -> (axum::Router, String, String, Arc<Recorded>) {
    session_with_quiet(false).await
}

async fn session_with_quiet(quiet: bool) -> (axum::Router, String, String, Arc<Recorded>) {
    let backend = Arc::new(Recorded(Mutex::default(), quiet));
    let w = Web::new(backend.clone(), "en".parse().unwrap(), 4321);
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = body(
        app.clone()
            .oneshot(get("/folders", HOST, Some(&cookie)))
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
    (app, cookie, form, backend)
}

#[tokio::test]
async fn the_browser_says_where_each_folder_is_and_whose_it_is() {
    let (app, cookie, _, _) = session().await;
    let page = |path: String| {
        let app = app.clone();
        let cookie = cookie.clone();
        async move { body(app.oneshot(get(&path, HOST, Some(&cookie))).await.unwrap()).await }
    };
    let top = page("/files".into()).await;
    assert!(
        top.contains("6 files · Kept on this computer"),
        "Photos is here"
    );
    assert!(
        top.contains("Not on this computer · Your choice"),
        "Photos 2019 was chosen"
    );
    assert!(top.contains("href=\"/folders\""), "the tree is a link away");
    let projets = page(format!("/folder?path={}", enc(&format!("{ROOT}/Projets")))).await;
    assert!(projets.contains("Robin’s folder, not on this computer"));
    let robin = format!("{ROOT}/Projets/robin");
    assert!(
        projets.contains("action=\"/files/keep\" data-inline>"),
        "a nested folder that is not here offers to keep it"
    );
    assert!(projets.contains(&format!("value=\"{robin}\"")));
    let inside = page(format!("/folder?path={}", enc(&robin))).await;
    assert!(
        inside.contains("This is Robin’s folder"),
        "the folder's own page says so"
    );
    let chosen = page(format!(
        "/files?selected={}",
        enc(&format!("{ROOT}/Photos 2019"))
    ))
    .await;
    assert!(
        chosen.contains("action=\"/files/forget\""),
        "Use the default"
    );
}

#[tokio::test]
async fn the_tree_marks_partly_kept_folders_and_applies_the_fewest_requests() {
    let (app, cookie, form, backend) = session().await;
    let page = body(
        app.clone()
            .oneshot(get("/folders", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(page.contains("data-tree") && page.contains("name=\"shown\" value=\"Projets/robin\""));
    assert!(
        page.contains("some folders in it are not on this computer"),
        "Projets is partly kept"
    );
    assert!(
        page.contains("value=\"Photos\" checked")
            && !page.contains("value=\"Projets/robin\" checked")
    );
    // Untick Photos (and all in it), tick Robin's folder, leave the rest.
    let mut fields = vec![format!("form={form}")];
    for f in super::demo::sync_folders() {
        fields.push(format!("shown={}", enc(&f.path)));
        let keep = if f.path.starts_with("Photos") {
            false
        } else {
            f.kept || f.path == "Projets/robin"
        };
        if keep {
            fields.push(format!("keep={}", enc(&f.path)));
        }
    }
    let done = app
        .clone()
        .oneshot(post("/folders", &cookie, &fields.join("&")))
        .await
        .unwrap();
    assert_eq!(done.status(), StatusCode::OK);
    let preview = body(done).await;
    assert!(preview.contains("paul-nas"), "{preview}");
    assert!(backend.changes().is_empty(), "the preview changes nothing");
    fields.push("confirm=1".into());
    let done = app
        .clone()
        .oneshot(post("/folders", &cookie, &fields.join("&")))
        .await
        .unwrap();
    assert_eq!(done.status(), StatusCode::OK);
    assert_eq!(
        backend.changes(),
        [
            pigeon_proto::Request::Evict {
                folder: format!("{ROOT}/Photos"),
                confirmed: false
            },
            pigeon_proto::Request::Keep {
                folder: format!("{ROOT}/Projets/robin")
            },
        ]
    );
    let forged = app
        .clone()
        .oneshot(post("/folders", &cookie, "form=forged&shown=Photos"))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    let twice = app
        .oneshot(post(
            "/folders",
            &cookie,
            &format!("form={form}&form={form}&shown=Photos"),
        ))
        .await
        .unwrap();
    assert_eq!(
        twice.status(),
        StatusCode::FORBIDDEN,
        "one form token, no more"
    );
    assert_eq!(backend.changes().len(), 2);
}

#[tokio::test]
async fn use_the_default_forgets_a_choice_only_with_the_form_token() {
    let (app, cookie, form, backend) = session().await;
    let folder = format!("{ROOT}/Photos 2019");
    let forged = app
        .clone()
        .oneshot(post(
            "/files/forget",
            &cookie,
            &format!("form=x&folder={}", enc(&folder)),
        ))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    assert!(backend.changes().is_empty());
    let done = app
        .oneshot(post(
            "/files/forget",
            &cookie,
            &format!("form={form}&folder={}", enc(&folder)),
        ))
        .await
        .unwrap();
    let text = body(done).await;
    assert!(
        text.contains("follows its default on this computer again"),
        "{text}"
    );
    assert_eq!(
        backend.changes(),
        [pigeon_proto::Request::ForgetChoice { folder }]
    );
}

#[tokio::test]
async fn freeing_from_the_tree_previews_everything_and_confirms_quiet_backups_twice() {
    let (app, cookie, form, backend) = session_with_quiet(true).await;
    let fields = format!("form={form}&shown=Photos");
    for confirmed in [0, 1] {
        let page = body(
            app.clone()
                .oneshot(post(
                    "/folders",
                    &cookie,
                    &format!("{fields}&confirm={confirmed}"),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert!(page.contains("quiet-nas"), "{page}");
        assert!(
            page.contains(&format!("name=\"confirm\" value=\"{}\"", confirmed + 1)),
            "{page}"
        );
        assert!(
            backend.changes().is_empty(),
            "neither preview changes any choice"
        );
    }
    let result = app
        .oneshot(post("/folders", &cookie, &format!("{fields}&confirm=2")))
        .await
        .unwrap();
    assert_eq!(result.status(), StatusCode::OK);
    assert_eq!(
        backend.changes(),
        [pigeon_proto::Request::Evict {
            folder: format!("{ROOT}/Photos"),
            confirmed: true
        }]
    );
}
