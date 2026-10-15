//! The actions of the web interface (10 §10.3): each is a `POST` that
//! carries the session's form token and becomes one request of 10 §10.1;
//! the page then says its result, which says how to undo it (ux.md §3.5).

use std::sync::Arc;

use axum::Extension;
use axum::Form;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use pigeon_proto::Request;

use crate::{FormToken, Web, pages};

/// The form that decides a suggestion.
#[derive(serde::Deserialize)]
pub struct Decision {
    /// The session's form token.
    form: String,
    /// The suggestion's id.
    id: String,
    /// Why it is declined, for its author.
    #[serde(default)]
    note: String,
}

/// *Accept*: `pigeon accept ID`.
pub async fn accept(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(d): Form<Decision>,
) -> Response {
    let request = Request::Accept {
        proposal: d.id.clone(),
        reason: None,
    };
    act(&web, &token, &d.form, request, "/suggestions").await
}

/// *Decline*: `pigeon decline ID [--note TEXT]`.
pub async fn decline(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(d): Form<Decision>,
) -> Response {
    let note = d.note.trim();
    let request = Request::Reject {
        proposal: d.id.clone(),
        note: (!note.is_empty()).then(|| note.to_owned()),
    };
    act(&web, &token, &d.form, request, "/suggestions").await
}

/// The form that keeps or frees a folder.
#[derive(serde::Deserialize)]
pub struct FolderForm {
    /// The session's form token.
    form: String,
    /// The folder, as an absolute path.
    folder: String,
}

/// *Keep on this computer*: `pigeon keep FOLDER`.
pub async fn keep(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<FolderForm>,
) -> Response {
    let request = Request::Keep { folder: f.folder };
    act(&web, &token, &f.form, request, "/files").await
}

/// *Free up space*: `pigeon free FOLDER`.
pub async fn free(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<FolderForm>,
) -> Response {
    let request = Request::Evict { folder: f.folder };
    act(&web, &token, &f.form, request, "/files").await
}

/// Asks `request` if `form` is the session's token, and shows the result.
async fn act(web: &Web, token: &FormToken, form: &str, request: Request, back: &str) -> Response {
    if token.0.is_empty() || form != token.0 {
        return (
            StatusCode::FORBIDDEN,
            pages::refused(web, "web-form-expired"),
        )
            .into_response();
    }
    let r = web.renderer();
    let lines = pages::lines(web, &r, request).await;
    pages::result(&r, back, lines).into_response()
}
