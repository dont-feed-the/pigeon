//! *Download a copy* (07 §7.6, 10 §10.3): `pigeon view PATH --version N`
//! from the file's page. The daemon writes a read-only copy in
//! `<cache>/view/`; the page hands it to the browser as a download, so the
//! person opens it with the program of their choice and the folder is never
//! touched. A problem is said like any other action's.

use std::sync::Arc;

use axum::Extension;
use axum::Form;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use pigeon_proto::{Arg, Request};

use crate::{FormToken, Web, pages};

/// The form of *Download a copy*.
#[derive(serde::Deserialize)]
pub struct ViewForm {
    /// The session's form token.
    form: String,
    /// The file, as an absolute path.
    path: String,
    /// The version's number.
    #[serde(default)]
    version: Option<u64>,
    /// A pending suggestion instead of accepted history.
    #[serde(default)]
    proposal: Option<String>,
}

/// `POST /files/view`: the copy as a download, or what went wrong.
pub async fn view(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<ViewForm>,
) -> Response {
    if token.0.is_empty() || f.form != token.0 {
        return (
            StatusCode::FORBIDDEN,
            pages::refused(&web, "web-form-expired"),
        )
            .into_response();
    }
    if f.proposal.is_some() && f.version.is_some() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let (back, request) = match f.proposal {
        Some(proposal) => (
            "/suggestions".to_owned(),
            Request::ViewSuggestion {
                proposal,
                path: f.path,
            },
        ),
        None => (
            format!(
                "/file?path={}",
                form_urlencoded::byte_serialize(f.path.as_bytes()).collect::<String>()
            ),
            Request::View {
                path: f.path,
                version: f.version,
                group: false,
            },
        ),
    };
    let r = web.renderer();
    let mut lines = Vec::new();
    for answer in web.ask(request).await {
        if let pigeon_proto::Response::Said { message } = &answer
            && message.id == "view-ready"
            && let Some(Arg::Text(copy)) = message.args.get("path")
            && let Ok(bytes) = tokio::fs::read(copy).await
        {
            return download(copy, bytes);
        }
        if !matches!(answer, pigeon_proto::Response::Progress { .. }) {
            lines.extend(r.response(&answer).0);
        }
    }
    pages::result(&r, &back, lines).into_response()
}

/// `bytes` as an attachment named like the copy, `stem (version N).ext`.
fn download(copy: &str, bytes: Vec<u8>) -> Response {
    let name = std::path::Path::new(copy)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let utf8 = form_urlencoded::byte_serialize(name.as_bytes())
        .collect::<String>()
        .replace('+', "%20");
    let disposition = format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{utf8}");
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        bytes,
    )
        .into_response()
}
