//! *Free up space* (ux.md §8.5, 08 §8.5), which asks before it acts: first
//! what will happen (`FreePreview`): which files leave this computer, the
//! backup machines they stay on, and that *Keep on this computer* brings
//! them back; then, only while no backup machine has been heard from
//! recently, a second time, saying since when (`Evict` answers
//! `backups-quiet`); then `pigeon free FOLDER`. Each step is a `POST` of
//! the same form with one more confirmation.

use axum::response::{IntoResponse, Response};
use maud::{Markup, html};
use pigeon_proto::{Request, Response as Answer};

use crate::pages::{self, list, page};
use crate::{Renderer, Web};

/// The step of *Free up space* for `folder` after `confirmed`
/// confirmations, with the session's form token `form`.
pub(crate) async fn step(web: &Web, form: &str, folder: &str, confirmed: u8) -> Response {
    let r = web.renderer();
    if confirmed == 0 {
        return preview(web, &r, form, folder).await;
    }
    let request = Request::Evict {
        folder: folder.to_owned(),
        confirmed: confirmed > 1,
    };
    let answers = web.ask(request).await;
    if let Some(problem) = answers.iter().find_map(|a| match a {
        Answer::Problem(p) if p.code == "backups-quiet" && confirmed == 1 => Some(p),
        _ => None,
    }) {
        let mut lines = vec![r.text(&problem.message)];
        lines.extend(problem.hint.as_ref().map(|h| r.text(h)));
        let body = html! {
            (list(&lines))
            (ask(&r, form, folder, 2, "web-free-anyway"))
        };
        return page(&r, "/files", &r.id("web-free-quiet-title"), &body).into_response();
    }
    let lines = answers
        .iter()
        .filter(|a| !matches!(a, Answer::Progress { .. }))
        .flat_map(|a| r.response(a).0)
        .collect();
    pages::result(&r, "/files", lines).into_response()
}

/// What freeing `folder` will do, and *Free up space* to go ahead.
async fn preview(web: &Web, r: &Renderer, form: &str, folder: &str) -> Response {
    let answers = web
        .ask(Request::FreePreview {
            folder: folder.to_owned(),
        })
        .await;
    let Some(preview) = answers.iter().find_map(|a| match a {
        Answer::FreePreview(p) => Some(p),
        _ => None,
    }) else {
        let lines = answers.iter().flat_map(|a| r.response(a).0).collect();
        return pages::result(r, "/files", lines).into_response();
    };
    let title = r
        .catalogue()
        .format(&pigeon_i18n::Message::new("web-free-title").arg("folder", folder.to_owned()));
    let body = html! {
        (list(&r.free_preview(preview)))
        (ask(r, form, folder, 1, "web-free"))
    };
    page(r, "/files", &title, &body).into_response()
}

/// The form that confirms once more, labelled `label`, and the way back.
fn ask(r: &Renderer, form: &str, folder: &str, confirm: u8, label: &str) -> Markup {
    html! {
        form method="post" action=(r.url("/files/free")) {
            input type="hidden" name="form" value=(form);
            input type="hidden" name="folder" value=(folder);
            input type="hidden" name="confirm" value=(confirm);
            button type="submit" { (r.id(label)) }
        }
        p { a href=(r.url("/files")) { (r.id("web-free-cancel")) } }
    }
}
