//! Keyboard and screen-reader use (usability-web-5 P4): a skip link to the
//! content, buttons named for the row they act on, and a folder's page
//! named and navigated from that folder.

use super::*;

/// A group whose `Photos/Enfant` holds a file and a subfolder.
struct Nested;

impl Backend for Nested {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        match request {
            pigeon_proto::Request::Files { .. } => Box::pin(async {
                vec![Response::Files {
                    root: "/Family".into(),
                    files: ["Photos/Enfant/graine.txt", "Photos/Enfant/deep/note.txt"]
                        .into_iter()
                        .map(|path| pigeon_proto::api::FileEntry {
                            path: path.into(),
                            state: State::Protected,
                        })
                        .collect(),
                }]
            }),
            other => Fixed.request(other),
        }
    }
}

async fn page_of(w: &Arc<Web>, path: &str) -> String {
    let app = w.router();
    let cookie = logged_in(&app, w).await;
    body(app.oneshot(get(path, HOST, Some(&cookie))).await.unwrap()).await
}

#[tokio::test]
async fn the_first_link_skips_to_the_content_in_both_languages() {
    for (lang, text) in [("en", "Skip to content"), ("fr", "Aller au contenu")] {
        let w = Web::new(Arc::new(Fixed), lang.parse().unwrap(), 4321);
        let page = page_of(&w, "/files").await;
        let first = page.split("<body>").nth(1).unwrap();
        assert!(
            first.starts_with(&format!("<a class=\"skip\" href=\"#main\">{text}</a>")),
            "{first}"
        );
        assert!(page.contains("<main id=\"main\""), "{page}");
    }
}

#[tokio::test]
async fn repeated_buttons_say_which_row_they_act_on() {
    let w = web();
    let file = page_of(&w, "/file?path=%2Fg%2Ftarte.docx").await;
    for n in [1, 2] {
        assert!(
            file.contains(&format!(
                "Open a copy<span class=\"visually-hidden\"> — version {n}, saved"
            )),
            "{file}"
        );
    }
    assert!(
        file.contains("Restore this version<span class=\"visually-hidden\"> — version 1"),
        "{file}"
    );
    let machines = page_of(&w, "/machines").await;
    assert!(
        machines.contains("Remove<span class=\"visually-hidden\"> — paul-laptop</span>"),
        "{machines}"
    );
    let suggestions = page_of(&w, "/suggestions").await;
    for label in ["Accept selected", "Decline selected"] {
        assert!(
            suggestions.contains(&format!(
                "{label}<span class=\"visually-hidden\"> — the change of tarte.docx by Paul</span>"
            )),
            "{suggestions}"
        );
    }
}

#[tokio::test]
async fn a_folder_page_is_named_and_navigated_from_that_folder() {
    let w = Web::new(Arc::new(Nested), "en".parse().unwrap(), 4321);
    let page = page_of(&w, "/folder?path=%2FFamily%2FPhotos%2FEnfant").await;
    assert!(page.contains("<h1 id=\"title\">Enfant</h1>"), "{page}");
    assert!(page.contains("<p>/Family/Photos/Enfant</p>"), "{page}");
    assert!(page.contains(">graine.txt: "), "{page}");
    assert!(page.contains(">deep/note.txt: "), "{page}");
    assert!(
        page.contains("href=\"/folder?path=%2FFamily%2FPhotos\">Back</a>"),
        "{page}"
    );
    assert!(
        page.contains("Free up space<span class=\"visually-hidden\"> — Enfant</span>"),
        "{page}"
    );
    let top = page_of(&w, "/folder?path=%2FFamily%2FPhotos").await;
    assert!(top.contains("href=\"/files\">Back</a>"), "{top}");
}
