//! The Suggestions page decides file by file (ADR 0076): each file with
//! what changes in it, a review button and a ticked checkbox; *Accept
//! selected* and *Decline selected* become one `DecideFiles`; *Accept all*
//! and *Decline all* decide every file of each; the author sees what was
//! decided, on the page and on the file's page. Everything is a plain form.

use std::sync::Mutex;

use pigeon_proto::api::{FileChange, Outcome, SuggestedFile};

use super::*;

/// A backend that keeps the requests and answers as a group with Bob's
/// suggestion of three files, an admin override one of Cid's, and Ana's
/// own decided one.
#[derive(Default)]
struct Kitchen(Mutex<Vec<pigeon_proto::Request>>);

fn file(path: &str, change: FileChange, before: Option<u64>, after: Option<u64>) -> SuggestedFile {
    SuggestedFile {
        path: path.into(),
        change,
        from: None,
        before,
        after,
        lines: None,
    }
}

fn bob() -> Suggestion {
    let mut recipes = file("recipes.txt", FileChange::Changed, Some(1_200), Some(1_400));
    recipes.lines = Some((3, 1));
    Suggestion {
        id: "b0b".repeat(10),
        author: "Bob".into(),
        device: "bob-laptop".into(),
        saved_at: "2026-10-15T10:00:00Z".into(),
        paths: vec![
            "groceries.txt".into(),
            "old soup.txt".into(),
            "recipes.txt".into(),
            "soup.txt".into(),
        ],
        decide: true,
        force: false,
        mine: false,
        files: vec![
            file("groceries.txt", FileChange::Deleted, Some(300), None),
            recipes,
            SuggestedFile {
                from: Some("old soup.txt".into()),
                ..file("soup.txt", FileChange::Renamed, Some(80), Some(80))
            },
        ],
    }
}

fn cid() -> Suggestion {
    Suggestion {
        id: "c1d".repeat(10),
        author: "Cid".into(),
        device: "cid-laptop".into(),
        saved_at: "2026-10-15T11:00:00Z".into(),
        paths: vec!["Photos/a.jpg".into()],
        decide: true,
        force: true,
        mine: false,
        files: vec![file(
            "Photos/a.jpg",
            FileChange::Added,
            None,
            Some(2_345_678),
        )],
    }
}

fn dan() -> Suggestion {
    Suggestion {
        id: "da0".repeat(10),
        author: "Dan".into(),
        files: vec![file("notes.txt", FileChange::Changed, Some(10), Some(12))],
        paths: vec!["notes.txt".into()],
        force: false,
        ..cid()
    }
}

fn mine() -> Suggestion {
    Suggestion {
        id: "a0a".repeat(10),
        author: "Ana".into(),
        paths: vec!["Docs/plan.txt".into()],
        decide: false,
        force: false,
        mine: true,
        files: vec![file("Docs/plan.txt", FileChange::Changed, Some(5), Some(9))],
        ..cid()
    }
}

impl Backend for Kitchen {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        self.0.lock().unwrap().push(request.clone());
        let answer = match request {
            pigeon_proto::Request::Proposals => vec![Response::Proposals {
                proposals: vec![bob(), cid(), dan(), mine()],
                decided: vec![Outcome {
                    id: "e1".repeat(20),
                    by: "Roy".into(),
                    decided_at: "2026-10-15T09:00:00Z".into(),
                    accepted: vec!["Docs/a.txt".into()],
                    declined: vec!["Docs/b.txt".into()],
                    note: Some("Garde l’ancienne <version>".into()),
                }],
                root: "/Family".into(),
            }],
            pigeon_proto::Request::DecideFiles { .. } => vec![Response::Said {
                message: Message {
                    id: "decided-some".into(),
                    args: [
                        ("author", pigeon_proto::Arg::Text("Bob".into())),
                        ("accepted", pigeon_proto::Arg::Number(2)),
                        (
                            "accepted-paths",
                            pigeon_proto::Arg::Text("recipes.txt, soup.txt".into()),
                        ),
                        ("declined", pigeon_proto::Arg::Number(1)),
                        (
                            "declined-paths",
                            pigeon_proto::Arg::Text("groceries.txt".into()),
                        ),
                    ]
                    .map(|(k, v)| (k.to_owned(), v))
                    .into(),
                },
            }],
            pigeon_proto::Request::History { .. } => vec![Response::History {
                versions: Vec::new(),
            }],
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

fn kitchen(lang: &str) -> (Arc<Web>, Arc<Kitchen>) {
    let backend = Arc::new(Kitchen::default());
    (
        Web::new(backend.clone(), lang.parse().unwrap(), 4321),
        backend,
    )
}

fn token(page: &str) -> String {
    page.split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned()
}

async fn page_of(app: &axum::Router, cookie: &str, path: &str) -> String {
    body(
        app.clone()
            .oneshot(get(path, HOST, Some(cookie)))
            .await
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn each_file_says_what_changes_and_is_ticked_with_its_own_review() {
    let (web, _) = kitchen("en");
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = page_of(&app, &cookie, "/suggestions").await;
    for text in [
        "For you to decide",
        "groceries.txt: deleted (it was 300 bytes)",
        "recipes.txt: changed, 1.2 kB → 1.4 kB, 3 lines added and 1 removed",
        "soup.txt: renamed from old soup.txt, content unchanged (80 bytes)",
        "Photos/a.jpg: new file, 2.3 MB",
        "Every file is selected.",
        "Accept selected",
        "Decline selected",
        "Note for Bob about the files you decline (optional)",
        "Review recipes.txt",
        "Your suggestions",
        "Docs/plan.txt: changed, 5 bytes → 9 bytes",
        "Your suggestions decided lately",
        "Roy accepted 1 of your changes (Docs/a.txt) and declined 1 (Docs/b.txt)",
        "Note from Roy: “Garde l’ancienne &lt;version&gt;”",
        "The files declined stay as you saved them on this computer",
    ] {
        assert!(page.contains(text), "{text}\n{page}");
    }
    // Every file is ticked, and a deletion has nothing to review.
    assert_eq!(page.matches("type=\"checkbox\" name=\"path\"").count(), 5);
    assert_eq!(page.matches(" checked").count(), 5);
    assert!(!page.contains("Review groceries.txt"));
    // Each review button belongs to a form of its own, which exists.
    for owner in page
        .split("form=\"")
        .skip(1)
        .map(|rest| rest.split('"').next().unwrap())
    {
        assert!(page.contains(&format!("id=\"{owner}\"")), "{owner}");
    }
    // Enter in the note decides nothing: the first button is disabled.
    let decide = page.split("action=\"/suggestions/decide\"").nth(1).unwrap();
    let first = decide.split("<button").nth(1).unwrap();
    assert!(first.contains("disabled"), "{first}");
    // Cid's needs the admin's reason to accept, not to decline.
    assert!(page.contains("name=\"reason\" required"));
    assert!(page.contains("value=\"decline\" formnovalidate"));
}

#[tokio::test]
async fn accept_selected_decides_the_ticked_files_and_says_what_happened() {
    let (web, backend) = kitchen("en");
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = page_of(&app, &cookie, "/suggestions").await;
    let form = token(&page);
    let id = "b0b".repeat(10);
    let fields =
        format!("form=00&id={id}&path=recipes.txt&path=soup.txt&decision=accept&note=Non+merci");
    backend.0.lock().unwrap().clear();
    let forged = app
        .clone()
        .oneshot(post("/suggestions/decide", &cookie, &fields))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
    assert!(backend.0.lock().unwrap().is_empty());
    let fields = fields.replace("form=00", &format!("form={form}"));
    let done = app
        .clone()
        .oneshot(post("/suggestions/decide", &cookie, &fields))
        .await
        .unwrap();
    assert_eq!(done.status(), StatusCode::OK);
    let text = body(done).await;
    assert!(
        text.contains(
            "You accepted 2 of Bob's changes (recipes.txt, soup.txt) and declined 1 \
             (groceries.txt). Bob will be told."
        ),
        "{text}"
    );
    assert_eq!(
        backend.0.lock().unwrap().as_slice(),
        &[pigeon_proto::Request::DecideFiles {
            proposal: id.clone(),
            paths: vec!["recipes.txt".into(), "soup.txt".into()],
            accept: true,
            note: Some("Non merci".into()),
            reason: None,
        }]
    );
    // Decline selected is the same request the other way round.
    backend.0.lock().unwrap().clear();
    let fields = format!("form={form}&id={id}&path=groceries.txt&decision=decline&note=");
    let done = app
        .clone()
        .oneshot(post("/suggestions/decide", &cookie, &fields))
        .await
        .unwrap();
    assert_eq!(done.status(), StatusCode::OK);
    assert_eq!(
        backend.0.lock().unwrap().as_slice(),
        &[pigeon_proto::Request::DecideFiles {
            proposal: id,
            paths: vec!["groceries.txt".into()],
            accept: false,
            note: None,
            reason: None,
        }]
    );
    // Without a choice of decision, nothing is asked.
    backend.0.lock().unwrap().clear();
    let fields = format!("form={form}&id=x&path=a&note=hello");
    let none = app
        .oneshot(post("/suggestions/decide", &cookie, &fields))
        .await
        .unwrap();
    assert_eq!(none.status(), StatusCode::BAD_REQUEST);
    assert!(backend.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn accept_all_decides_every_file_of_each_and_skips_an_override() {
    let (web, backend) = kitchen("en");
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = page_of(&app, &cookie, "/suggestions").await;
    assert!(page.contains("2 suggestions wait for you."), "{page}");
    assert!(page.contains("Accept all") && page.contains("Decline all"));
    let form = token(&page);
    let (b, c, d) = ("b0b".repeat(10), "c1d".repeat(10), "da0".repeat(10));
    backend.0.lock().unwrap().clear();
    let fields = format!("form={form}&id={b}&id={c}&id={d}&decision=decline&note=Plus+tard");
    let done = app
        .oneshot(post("/suggestions/all", &cookie, &fields))
        .await
        .unwrap();
    assert_eq!(done.status(), StatusCode::OK);
    let text = body(done).await;
    assert!(text.contains("You decided 2 suggestions."), "{text}");
    assert!(
        text.contains("Cid's suggestion changes something nobody may normally change"),
        "{text}"
    );
    let asked: Vec<_> = backend
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|r| matches!(r, pigeon_proto::Request::DecideFiles { .. }))
        .cloned()
        .collect();
    assert_eq!(
        asked,
        [
            pigeon_proto::Request::DecideFiles {
                proposal: b,
                paths: bob().paths,
                accept: false,
                note: Some("Plus tard".into()),
                reason: None,
            },
            pigeon_proto::Request::DecideFiles {
                proposal: d,
                paths: vec!["notes.txt".into()],
                accept: false,
                note: Some("Plus tard".into()),
                reason: None,
            },
        ]
    );
}

#[tokio::test]
async fn a_files_page_shows_its_authors_waiting_and_decided_suggestions() {
    let (web, _) = kitchen("en");
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = page_of(&app, &cookie, "/file?path=%2FFamily%2FDocs%2Fplan.txt").await;
    assert!(page.contains("Your change to this file, saved"), "{page}");
    let page = page_of(&app, &cookie, "/file?path=%2FFamily%2FDocs%2Fb.txt").await;
    assert!(page.contains("Roy accepted 1 of your changes"), "{page}");
    assert!(page.contains("Note from Roy"), "{page}");
    let page = page_of(&app, &cookie, "/file?path=%2FFamily%2Fother.txt").await;
    assert!(
        !page.contains("Roy") && !page.contains("Your change"),
        "{page}"
    );
}

#[tokio::test]
async fn the_page_is_in_french_too() {
    let (web, _) = kitchen("fr");
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = page_of(&app, &cookie, "/suggestions").await;
    for text in [
        "À vous de décider",
        "recipes.txt : modifié, 1,2 ko → 1,4 ko, 3 lignes ajoutées et 1 retirée",
        "groceries.txt : supprimé (il faisait 300 octets)",
        "Accepter la sélection",
        "Refuser la sélection",
        "Tout accepter",
        "Roy a accepté 1 de vos modifications (Docs/a.txt) et en a refusé 1 (Docs/b.txt)",
        "Note de Roy : « Garde l’ancienne &lt;version&gt; »",
    ] {
        assert!(page.contains(text), "{text}\n{page}");
    }
}
