//! Files (ux.md §6.2): the group's folders, each with its state and a
//! *Keep on this computer* or *Free up space* button (08 §8.5); a
//! folder's page lists its files, and a file's page its versions, each
//! with *Restore this version* (`pigeon restore PATH --version N`), which
//! makes a new version. Deleted files come later.

use std::sync::Arc;

use axum::Extension;
use axum::extract::{Query, State};
use axum::response::Html;
use maud::{Markup, html};
use pigeon_i18n::render::Renderer;
use pigeon_proto::api::{Folder, VersionStatus};
use pigeon_proto::{Request, Response};

use crate::pages::{list, page};
use crate::{FormToken, Web};

/// The Files page.
pub async fn files(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let mut root = String::new();
    let mut rows = Vec::new();
    let mut other = Vec::new();
    for answer in web.ask(Request::Folders).await {
        match answer {
            Response::Folders { root: at, folders } => {
                root = at;
                rows = folders;
            }
            Response::Progress { .. } => {}
            a => other.extend(r.response(&a).0),
        }
    }
    let body = html! {
        @if rows.is_empty() && other.is_empty() { p { (r.id("folders-none")) } }
        (list(&other))
        @for f in &rows { (folder(&r, &root, f, &form.0)) }
    };
    page(&r, "/files", &r.id("web-nav-files"), &body)
}

/// One folder, and the form that keeps or frees it.
fn folder(r: &Renderer, root: &str, f: &Folder, form: &str) -> Markup {
    let (action, label) = if f.freed {
        ("/files/keep", "web-keep")
    } else {
        ("/files/free", "web-free")
    };
    let path = std::path::Path::new(root).join(&f.path);
    html! {
        article {
            p { a href=(format!("/folder?path={}", enc(&path.to_string_lossy()))) { (r.folder(f)) } }
            form method="post" action=(action) {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="folder" value=(path.to_string_lossy());
                button type="submit" { (r.id(label)) }
            }
        }
    }
}

/// A path in a query.
fn enc(path: &str) -> String {
    form_urlencoded::byte_serialize(path.as_bytes()).collect()
}

/// The query of a folder's or a file's page.
#[derive(serde::Deserialize)]
pub struct At {
    /// The folder or file, as an absolute path.
    path: String,
}

/// A folder's page: its files, each a link to its page.
pub async fn folder_page(State(web): State<Arc<Web>>, Query(at): Query<At>) -> Html<String> {
    let r = web.renderer();
    let mut root = String::new();
    let mut rows = Vec::new();
    let mut other = Vec::new();
    for answer in web
        .ask(Request::Files {
            folder: at.path.clone(),
        })
        .await
    {
        match answer {
            Response::Files { root: here, files } => {
                root = here;
                rows = files;
            }
            Response::Progress { .. } => {}
            a => other.extend(r.response(&a).0),
        }
    }
    let line = |path: &str, state| {
        let m = pigeon_i18n::Message::new("file-row")
            .arg("path", path)
            .arg("state", r.state(state));
        r.catalogue().format(&m)
    };
    let body = html! {
        (list(&other))
        @if rows.is_empty() && other.is_empty() { p { (r.id("files-none")) } }
        ul {
            @for f in &rows {
                @let full = std::path::Path::new(&root).join(&f.path);
                li { a href=(format!("/file?path={}", enc(&full.to_string_lossy()))) { (line(&f.path, f.state)) } }
            }
        }
        p { a href="/files" { (r.id("web-back")) } }
    };
    page(&r, "/files", &at.path, &body)
}

/// A file's page: its versions, newest first, each with *Restore this
/// version* but the current one.
pub async fn file_page(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
    Query(at): Query<At>,
) -> Html<String> {
    let r = web.renderer();
    let mut rows = Vec::new();
    let mut other = Vec::new();
    for answer in web
        .ask(Request::History {
            path: at.path.clone(),
        })
        .await
    {
        match answer {
            Response::History { versions } => rows = versions,
            Response::Progress { .. } => {}
            a => other.extend(r.response(&a).0),
        }
    }
    let newest = rows.iter().map(|v| v.number).max();
    let body = html! {
        (list(&other))
        h2 { (r.id("web-versions")) }
        @for v in &rows { (version(&r, &at.path, v, Some(v.number) != newest, &form.0)) }
        p { a href="/files" { (r.id("web-back")) } }
    };
    page(&r, "/files", &at.path, &body)
}

/// One version, and the form that restores it.
fn version(r: &Renderer, path: &str, v: &VersionStatus, restore: bool, form: &str) -> Markup {
    html! {
        article {
            p { (r.version(v)) }
            @if restore {
                form method="post" action="/files/restore" {
                    input type="hidden" name="form" value=(form);
                    input type="hidden" name="path" value=(path);
                    input type="hidden" name="version" value=(v.number);
                    button type="submit" { (r.id("web-restore")) }
                }
            }
        }
    }
}
