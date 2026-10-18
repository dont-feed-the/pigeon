//! A pause shows on every page with *Share again*, also when something
//! else needs the person, and Settings offers only the action that applies
//! (15 §15.6).
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

struct Pausing {
    paused: AtomicBool,
}

fn message(id: &str, args: &[(&str, String)]) -> Message {
    Message {
        id: id.into(),
        args: args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), pigeon_proto::Arg::Text(v.clone())))
            .collect(),
    }
}

impl Backend for Pausing {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        let until = pigeon_proto::time::rfc3339(crate::now() + 3600);
        let paused = self.paused.load(Ordering::SeqCst);
        let answer = match request {
            pigeon_proto::Request::Paused if paused => vec![Response::Said {
                message: message("headline-paused", &[("until", until)]),
            }],
            pigeon_proto::Request::Status { .. } | pigeon_proto::Request::Health => {
                vec![Response::Status(StatusReport {
                    headline: message("headline-disk-full", &[]),
                    level: Level::NeedsYou,
                    state: State::Local,
                    versions: Vec::new(),
                    not_shared: Vec::new(),
                    paused: None,
                    scan_only: None,
                })]
            }
            pigeon_proto::Request::Resume => {
                self.paused.store(false, Ordering::SeqCst);
                vec![Response::Said {
                    message: message("resumed", &[]),
                }]
            }
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

/// Writes Home and Settings while paused, for `scripts/check-a11y`.
pub(super) async fn write_pages(dir: &std::path::Path, lang: &str) {
    let web = Web::new(
        Arc::new(Pausing {
            paused: AtomicBool::new(true),
        }),
        lang.parse().unwrap(),
        4321,
    );
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    for (name, path) in [("home", "/"), ("settings", "/settings")] {
        let page = app
            .clone()
            .oneshot(get(path, HOST, Some(&cookie)))
            .await
            .unwrap();
        std::fs::write(
            dir.join(format!("{lang}-paused-{name}.html")),
            body(page).await,
        )
        .unwrap();
    }
}

#[tokio::test]
async fn every_page_says_the_pause_until_sharing_again() {
    let backend = Arc::new(Pausing {
        paused: AtomicBool::new(true),
    });
    let web = Web::new(backend.clone(), "en".parse().unwrap(), 4321);
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = |path: &'static str| {
        let app = app.clone();
        let cookie = cookie.clone();
        async move { body(app.oneshot(get(path, HOST, Some(&cookie))).await.unwrap()).await }
    };
    for path in ["/", "/files", "/machines", "/space", "/suggestions"] {
        let html = page(path).await;
        assert!(
            html.contains("Sharing is paused until ")
                && html.contains("Changes are saved on this computer only."),
            "{path}: {html}"
        );
        assert!(html.contains("action=\"/resume\""), "{path}");
        let raw = format!("{}T", pigeon_proto::time::date(crate::now() + 3600));
        assert!(!html.contains(&raw), "{path}: a raw time");
    }
    // Something else needs the person: the pause still shows.
    let home = page("/").await;
    assert!(home.contains("Your disk is full"), "{home}");
    assert!(home.contains("class=\"pause-banner\""));
    let settings = page("/settings").await;
    assert!(settings.contains("action=\"/resume\"") && !settings.contains("action=\"/pause\""));
    let token = settings
        .split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    let resumed = body(
        app.clone()
            .oneshot(post("/resume", &cookie, &format!("form={token}")))
            .await
            .unwrap(),
    )
    .await;
    assert!(resumed.contains("Sharing again.") && !resumed.contains("class=\"pause-banner\""));
    let settings = page("/settings").await;
    assert!(settings.contains("action=\"/pause\"") && !settings.contains("action=\"/resume\""));
    assert!(!page("/files").await.contains("class=\"pause-banner\""));
}
