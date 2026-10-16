//! *Free up space* asks first, says which files leave this computer and
//! where they stay, asks again while the backup machines are quiet, and
//! freed files read "Safe, not on this computer" (08 §8.5).
use super::*;
use pigeon_proto::api::{FileEntry, FreePreview};
use std::sync::Mutex;

struct Freeing {
    calls: Mutex<Vec<pigeon_proto::Request>>,
}

impl Backend for Freeing {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        self.calls.lock().unwrap().push(request.clone());
        let heard = pigeon_proto::time::rfc3339(crate::now() - 3 * 3600);
        let answer = match request {
            pigeon_proto::Request::FreePreview { .. } => {
                vec![Response::FreePreview(FreePreview {
                    files: 3,
                    safe: 2,
                    backups: vec!["nas".into()],
                    heard_at: Some(heard),
                    quiet: true,
                })]
            }
            pigeon_proto::Request::Evict {
                confirmed: false, ..
            } => vec![Response::Problem(pigeon_proto::Problem {
                code: "backups-quiet".into(),
                message: Message {
                    id: "problem-backups-quiet".into(),
                    args: [
                        ("machines".to_owned(), pigeon_proto::Arg::Text("nas".into())),
                        ("backups".to_owned(), pigeon_proto::Arg::Number(1)),
                        ("heard".to_owned(), pigeon_proto::Arg::Text(heard)),
                    ]
                    .into(),
                },
                hint: Some(Message {
                    id: "hint-backups-quiet".into(),
                    args: Default::default(),
                }),
                details: None,
            })],
            pigeon_proto::Request::Evict {
                confirmed: true, ..
            } => vec![Response::Said {
                message: Message {
                    id: "free-result".into(),
                    args: [
                        ("safe".to_owned(), pigeon_proto::Arg::Number(2)),
                        ("total".to_owned(), pigeon_proto::Arg::Number(3)),
                    ]
                    .into(),
                },
            }],
            pigeon_proto::Request::Files { .. } => vec![Response::Files {
                root: "/g".into(),
                files: vec![
                    FileEntry {
                        path: "Photos/a.jpg".into(),
                        state: State::Protected,
                        freed: true,
                    },
                    FileEntry {
                        path: "Photos/b.jpg".into(),
                        state: State::Protected,
                        freed: false,
                    },
                ],
            }],
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

/// Writes both confirmations of *Free up space*, for `scripts/check-a11y`.
pub(super) async fn write_pages(dir: &std::path::Path, lang: &str) {
    let web = Web::new(
        Arc::new(Freeing {
            calls: Mutex::new(Vec::new()),
        }),
        lang.parse().unwrap(),
        4321,
    );
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let folder = body(
        app.clone()
            .oneshot(get("/folder?path=%2Fg%2FPhotos", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    let token = folder
        .split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    for (name, confirm) in [("free", 0), ("free-quiet", 1)] {
        let form = format!("form={token}&folder=%2Fg%2FPhotos&confirm={confirm}");
        let page = app
            .clone()
            .oneshot(post("/files/free", &cookie, &form))
            .await
            .unwrap();
        std::fs::write(dir.join(format!("{lang}-{name}.html")), body(page).await).unwrap();
    }
}

#[tokio::test]
async fn freeing_asks_first_then_again_while_the_backups_are_quiet() {
    let backend = Arc::new(Freeing {
        calls: Mutex::new(Vec::new()),
    });
    let web = Web::new(backend.clone(), "en".parse().unwrap(), 4321);
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let folder = body(
        app.clone()
            .oneshot(get("/folder?path=%2Fg%2FPhotos", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    // The folder page names its files from that folder.
    assert!(
        folder.contains(">a.jpg: Safe, not on this computer<"),
        "{folder}"
    );
    assert!(folder.contains(">b.jpg: Safe<"), "{folder}");
    let file = body(
        app.clone()
            .oneshot(get("/file?path=%2Fg%2FPhotos%2Fa.jpg", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(file.contains("<p>Safe, not on this computer</p>"), "{file}");
    let token = folder
        .split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    backend.calls.lock().unwrap().clear();
    let form = format!("form={token}&folder=%2Fg%2FPhotos");
    let first = body(
        app.clone()
            .oneshot(post("/files/free", &cookie, &form))
            .await
            .unwrap(),
    )
    .await;
    assert!(first.contains("Free up space in /g/Photos?"), "{first}");
    assert!(first.contains("These 2 files will be removed from this computer."));
    assert!(first.contains("They stay safe on the backup machines: nas."));
    assert!(first.contains("1 file is not safe yet"));
    assert!(first.contains("“Keep on this computer” brings them back"));
    assert!(first.contains("name=\"confirm\" value=\"1\""));
    assert!(
        !backend
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| matches!(c, pigeon_proto::Request::Evict { .. })),
        "nothing is freed before the person confirms"
    );
    let second = body(
        app.clone()
            .oneshot(post("/files/free", &cookie, &format!("{form}&confirm=1")))
            .await
            .unwrap(),
    )
    .await;
    assert!(second.contains("The backup machines have not been in touch"));
    assert!(
        second.contains("The backup machine nas was last in touch 3 hours ago"),
        "{second}"
    );
    assert!(second.contains("name=\"confirm\" value=\"2\""));
    assert!(second.contains("Free up space anyway"));
    let done = body(
        app.clone()
            .oneshot(post("/files/free", &cookie, &format!("{form}&confirm=2")))
            .await
            .unwrap(),
    )
    .await;
    assert!(done.contains("Files safe and eligible to free: 2 of 3"));
    assert_eq!(
        backend
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| matches!(c, pigeon_proto::Request::Evict { .. }))
            .cloned()
            .collect::<Vec<_>>(),
        [false, true].map(|confirmed| pigeon_proto::Request::Evict {
            folder: "/g/Photos".into(),
            confirmed,
        })
    );
    let forged = app
        .oneshot(post(
            "/files/free",
            &cookie,
            "form=forged&folder=%2Fg%2FPhotos&confirm=2",
        ))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
}
