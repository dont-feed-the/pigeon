//! Files (ux.md §6.2): the group's folders, each with its state and a
//! *Keep on this computer* or *Free up space* button (08 §8.5). Deleted
//! files and a file's page come later.

use std::sync::Arc;

use axum::Extension;
use axum::extract::State;
use axum::response::Html;
use maud::{Markup, html};
use pigeon_i18n::render::Renderer;
use pigeon_proto::api::Folder;
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
            p { (r.folder(f)) }
            form method="post" action=(action) {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="folder" value=(path.to_string_lossy());
                button type="submit" { (r.id(label)) }
            }
        }
    }
}
