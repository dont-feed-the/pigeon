//! A file's page and Deleted files (ux.md §6.2). A file's page shows its
//! versions as a timeline, newest first, each with *Open a copy* and, but
//! the current one, *Restore this version* (`pigeon restore PATH --version
//! N`), which makes a new version. *Deleted files* lists the files deleted
//! from the group, each with *Restore*, which brings back its last version,
//! and the wholly deleted folders, each restored at once.

use std::sync::Arc;

use axum::Extension;
use axum::extract::{Query, State};
use axum::response::Html;
use maud::html;
use pigeon_proto::{Request, Response};

use crate::browse::enc;
use crate::format::when;
use crate::icons::{file_icon, icon};
use crate::pages::{bare, list};
use crate::{FormToken, Renderer, Web};

/// The query of a file's page.
#[derive(serde::Deserialize)]
pub struct At {
    /// The file, as an absolute path.
    path: String,
}

/// A file's page: its versions, newest first.
pub async fn file_page(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
    Query(at): Query<At>,
) -> Html<String> {
    let r = web.renderer();
    let differs = crate::conflicts::on_file(&web, &r, &at.path, &form.0).await;
    let mut rows = Vec::new();
    let mut other = Vec::new();
    let mut root = String::new();
    for answer in web.ask(Request::Folders).await {
        if let Response::Folders { root: at, .. } = answer {
            root = at;
        }
    }
    for answer in web
        .ask(Request::History {
            path: at.path.clone(),
        })
        .await
    {
        match answer {
            Response::History { versions } => rows = versions,
            Response::Progress { .. } => {}
            // The block below explains a differing copy and how to settle it.
            Response::Said { message } if differs.is_some() && message.id == "why-differs" => {}
            a => other.extend(r.response(&a).0),
        }
    }
    let newest = rows.iter().map(|v| v.number).max();
    let here = current_state(&web, &r, &at.path).await;
    let suggested = crate::suggestions::on_file(&web, &r, &at.path).await;
    let current = r.id(if differs.is_some() {
        "web-version-group-current"
    } else {
        "web-version-current"
    });
    let path = std::path::Path::new(&at.path);
    let name = path
        .file_name()
        .map_or_else(|| at.path.clone(), |n| n.to_string_lossy().into_owned());
    let mut trail = vec![(r.id("web-nav-files"), r.url("/files"))];
    if let Ok(rel) = path.strip_prefix(&root)
        && !root.is_empty()
    {
        let mut here = std::path::PathBuf::from(&root);
        for part in rel.iter() {
            here.push(part);
            let href = if here == path {
                r.url(&format!("/file?path={}", enc(&at.path)))
            } else {
                r.url(&format!("/folder?path={}", enc(&here.to_string_lossy())))
            };
            trail.push((part.to_string_lossy().into_owned(), href));
        }
    } else {
        trail.push((
            name.clone(),
            r.url(&format!("/file?path={}", enc(&at.path))),
        ));
    }
    let body = html! {
        header class="page-head" {
            (crate::shell::crumbs(&r, &trail))
            div class="details-head" {
                (file_icon(&name))
                h1 id="title" { (name) }
            }
            p class="lead" { (at.path) }
            @if let Some(v) = rows.iter().find(|v| Some(v.number) == newest) {
                div { (crate::format::state(&r, v.state)) }
            }
        }
        (list(&other))
        @if let Some(state) = &here { p { (state) } }
        @if let Some(differs) = differs { (differs) }
        (suggested)
        div class="card" {
            h2 { (icon("history", "")) " " (r.id("web-versions")) }
            ol class="timeline" {
                @for v in &rows { (crate::details::version(&r, &at.path, v, (Some(v.number) == newest).then_some(current.as_str()), &form.0)) }
            }
        }
        p { a class="btn" href=(r.url("/files")) { (icon("back", "")) (r.id("web-back")) } }
    };
    bare(&r, "/files", &name, &body)
}

/// The state of the file at `path` now, and whether it is on this
/// computer: "Safe, not on this computer" once its space is freed.
async fn current_state(web: &Web, r: &Renderer, path: &str) -> Option<String> {
    let answers = web
        .ask(Request::Files {
            folder: path.to_owned(),
        })
        .await;
    answers.into_iter().find_map(|answer| match answer {
        Response::Files { root, files } => files
            .iter()
            .find(|f| std::path::Path::new(&root).join(&f.path) == std::path::Path::new(path))
            .map(|f| r.file_state(f)),
        _ => None,
    })
}

/// Deleted files: each with *Restore*, its last version.
pub async fn deleted_page(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let mut root = String::new();
    let mut rows = Vec::new();
    let mut other = Vec::new();
    for answer in web.ask(Request::Deleted { path: None }).await {
        match answer {
            Response::Deleted { root: here, files } => {
                root = here;
                rows = files;
            }
            Response::Progress { .. } => {}
            a => other.extend(r.response(&a).0),
        }
    }
    let current = if root.is_empty() {
        None
    } else {
        web.ask(Request::Files {
            folder: root.clone(),
        })
        .await
        .into_iter()
        .find_map(|answer| match answer {
            Response::Files { files, .. } => {
                Some(files.into_iter().map(|f| f.path).collect::<Vec<_>>())
            }
            _ => None,
        })
    };
    let folders = current.as_ref().map_or_else(Vec::new, |current| {
        deleted_folders(rows.iter().map(|f| f.path.as_str()), current)
    });
    let restore = |path: &str, label: &str| {
        html! {
            form method="post" action=(r.url("/files/restore")) data-inline {
                input type="hidden" name="form" value=(form.0);
                input type="hidden" name="path" value=(std::path::Path::new(&root).join(path).to_string_lossy());
                button type="submit" class="small" { (icon("restore", "")) (r.id(label)) }
            }
        }
    };
    let body = html! {
        header class="page-head" {
            h1 id="title" { (r.id("web-deleted")) }
            p class="lead" { (r.id("web-deleted-lead")) }
        }
        (list(&other))
        @if rows.is_empty() && other.is_empty() {
            div class="empty" { (icon("trash", "")) strong { (r.id("deleted-none")) } }
        }
        @if !folders.is_empty() {
            div class="card" {
                p { (r.id("web-restore-folder-help")) }
                ul class="plain-list" {
                    @for folder in &folders {
                        li class="machine" {
                            (icon("folder", "ft ft-folder"))
                            span class="grow" { (folder) }
                            (restore(folder, "web-restore-folder"))
                        }
                    }
                }
            }
        }
        @if !rows.is_empty() {
            table class="files" data-files {
                caption class="visually-hidden" { (r.id("web-deleted")) }
                thead {
                    tr {
                        th scope="col" class="col-icon" { span class="visually-hidden" { (r.id("web-col-kind")) } }
                        th scope="col" { (r.id("web-col-name")) }
                        th scope="col" class="col-when" { (r.id("web-col-deleted")) }
                        th scope="col" class="col-actions" { span class="visually-hidden" { (r.id("web-col-actions")) } }
                    }
                }
                tbody {
                    @for f in &rows {
                        tr data-name=(f.path) {
                            td class="col-icon" { (file_icon(&f.path)) }
                            td class="col-name" { (f.path) }
                            td class="when col-when" { (when(&r, &f.deleted_at)) }
                            td class="col-actions" { (restore(&f.path, "web-restore-deleted")) }
                        }
                    }
                }
            }
        }
    };
    bare(&r, "/deleted", &r.id("web-deleted"), &body)
}

/// Offer the outermost wholly deleted folders, never a partly current
/// ancestor whose latest snapshot would omit the deleted subtree.
fn deleted_folders<'a>(deleted: impl Iterator<Item = &'a str>, current: &[String]) -> Vec<String> {
    let mut candidates = std::collections::BTreeSet::new();
    for path in deleted {
        for (index, _) in path.match_indices('/') {
            let folder = &path[..index];
            let prefix = format!("{folder}/");
            if !current
                .iter()
                .any(|p| p == folder || p.starts_with(&prefix))
            {
                candidates.insert(folder.to_owned());
            }
        }
    }
    candidates
        .iter()
        .filter(|folder| {
            !candidates
                .iter()
                .any(|parent| *folder != parent && folder.starts_with(&format!("{parent}/")))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod restore_tests {
    #[test]
    fn only_wholly_deleted_subtrees_get_one_restore_action() {
        let deleted = [
            "Photos/Gone/a",
            "Photos/Gone/deep/b",
            "Photos-old/gone",
            "whole/a",
            "whole/deep/b",
        ];
        let current = vec!["Photos/current.jpg".into(), "Photos-old/current".into()];
        assert_eq!(
            super::deleted_folders(deleted.into_iter(), &current),
            ["Photos/Gone", "whole"]
        );
    }
}
