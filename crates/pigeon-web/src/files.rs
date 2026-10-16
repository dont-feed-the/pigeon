//! Files (ux.md §6.2): the group's folders, each with its state and a
//! *Keep on this computer* or *Free up space* button (08 §8.5); a
//! folder's page lists its files, and a file's page its versions, each
//! with *Restore this version* (`pigeon restore PATH --version N`), which
//! makes a new version. *Deleted files* lists the files deleted from the
//! group, each with *Restore*, which brings back its last version.

use std::sync::Arc;

use crate::Renderer;
use axum::Extension;
use axum::extract::{Query, State};
use axum::response::Html;
use maud::{Markup, html};
use pigeon_proto::api::{Folder, VersionStatus};
use pigeon_proto::{Request, Response};

use crate::pages::{hidden, list, page};
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
    let mut root_files = Vec::new();
    if !root.is_empty() {
        for answer in web
            .ask(Request::Files {
                folder: root.clone(),
            })
            .await
        {
            match answer {
                Response::Files { files, .. } => {
                    root_files.extend(files.into_iter().filter(|f| !f.path.contains('/')))
                }
                Response::Progress { .. } => {}
                a => other.extend(r.response(&a).0),
            }
        }
    }
    let body = html! {
        @if rows.is_empty() && root_files.is_empty() && other.is_empty() { p { (r.id("files-none")) } }
        (list(&other))
        @if !root_files.is_empty() {
            h2 { (r.id("web-root-files")) }
            ul {
                @for f in &root_files {
                    @let full = std::path::Path::new(&root).join(&f.path);
                    li {
                        a href=(r.url(&format!("/file?path={}", enc(&full.to_string_lossy())))) {
                            (r.catalogue().format(&pigeon_i18n::Message::new("file-row").arg("path", f.path.clone()).arg("state", r.state(f.state))))
                        }
                    }
                }
            }
        }
        @for f in &rows { (folder(&r, &root, f, &form.0)) }
        p { a href=(r.url("/space")) { (r.id("space-title")) } }
        p { a href=(r.url("/deleted")) { (r.id("web-deleted")) } }
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
            p { a href=(r.url(&format!("/folder?path={}", enc(&path.to_string_lossy())))) { (r.folder(f)) } }
            @if r.storage_controls() && f.arriving > 0 {
                p { (r.catalogue().format(&pigeon_i18n::Message::new("web-files-arriving").arg("files", i64::try_from(f.arriving).unwrap_or(i64::MAX)))) }
            }
            @if r.storage_controls() { form method="post" action=(r.url(action)) {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="folder" value=(path.to_string_lossy());
                button type="submit" { (r.id(label)) (hidden(&f.path)) }
            } }
        }
    }
}

/// A path in a query.
pub(crate) fn enc(path: &str) -> String {
    form_urlencoded::byte_serialize(path.as_bytes()).collect()
}

/// The query of a folder's or a file's page.
#[derive(serde::Deserialize)]
pub struct At {
    /// The folder or file, as an absolute path.
    path: String,
}

/// A folder's page: its files, each a link to its page.
pub async fn folder_page(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
    Query(at): Query<At>,
) -> Html<String> {
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
    let prefix = std::path::Path::new(&at.path)
        .strip_prefix(&root)
        .ok()
        .and_then(|p| p.to_str())
        .unwrap_or_default()
        .trim_end_matches('/');
    // A file's name from this folder down; *Back* goes up one folder.
    let shown = |path: &str| -> String {
        match path.strip_prefix(prefix).and_then(|p| p.strip_prefix('/')) {
            Some(rest) if !prefix.is_empty() => rest.to_owned(),
            _ => path.to_owned(),
        }
    };
    let here = std::path::Path::new(&at.path);
    let back = match (prefix.contains('/'), here.parent()) {
        (true, Some(parent)) => format!("/folder?path={}", enc(&parent.to_string_lossy())),
        _ => "/files".to_owned(),
    };
    let name = here
        .file_name()
        .map_or_else(|| at.path.clone(), |n| n.to_string_lossy().into_owned());
    let children: std::collections::BTreeSet<_> = rows
        .iter()
        .filter_map(|file| {
            let relative = if prefix.is_empty() {
                file.path.as_str()
            } else {
                file.path.strip_prefix(prefix)?.strip_prefix('/')?
            };
            let (name, _) = relative.split_once('/')?;
            Some(name)
        })
        .collect();
    let body = html! {
        (list(&other))
        @if rows.is_empty() && other.is_empty() { p { (r.id("files-none")) } }
        @if !children.is_empty() {
            h2 { (r.id("web-subfolders")) }
            ul {
                @for name in &children {
                    @let child = std::path::Path::new(&at.path).join(name);
                    li { a href=(r.url(&format!("/folder?path={}", enc(&child.to_string_lossy())))) { (name) } }
                }
            }
        }
        ul {
            @for f in &rows {
                @let full = std::path::Path::new(&root).join(&f.path);
                li { a href=(r.url(&format!("/file?path={}", enc(&full.to_string_lossy())))) { (line(&shown(&f.path), f.state)) } }
            }
        }
        p { a href=(r.url(&back)) { (r.id("web-back")) } }
    };
    let body = html! {
        p { (at.path) }
        @if r.storage_controls() && !root.is_empty() && !rows.is_empty() {
            @for (action, label) in [("/files/free", "web-free"), ("/files/keep", "web-keep")] {
                form method="post" action=(r.url(action)) {
                    input type="hidden" name="form" value=(form.0);
                    input type="hidden" name="folder" value=(at.path);
                    button type="submit" { (r.id(label)) (hidden(&name)) }
                }
            }
        }
        p { a href=(r.url(&format!("/folder-rule?path={}", enc(&at.path)))) { (r.id("web-folder-rule")) } }
        (body)
    };
    page(&r, "/files", &name, &body)
}

/// A file's page: what to do when this computer's copy differs from the
/// group's (`conflicts.rs`), then its versions, newest first and the
/// current one marked, each with *Open a copy*, and *Restore this
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
    let differs = crate::conflicts::on_file(&web, &r, &at.path, &form.0).await;
    // While this copy differs, the newest version is the group's, not this file.
    let current = r.id(if differs.is_some() {
        "web-version-group-current"
    } else {
        "web-version-current"
    });
    let suggested = crate::suggestions::on_file(&web, &r, &at.path).await;
    let body = html! {
        (list(&other))
        @if let Some(differs) = differs { (differs) }
        (suggested)
        h2 { (r.id("web-versions")) }
        @for v in &rows {
            (version(&r, &at.path, v, (Some(v.number) == newest).then_some(current.as_str()), &form.0))
        }
        p { a href=(r.url("/files")) { (r.id("web-back")) } }
    };
    page(&r, "/files", &at.path, &body)
}

/// One version, marked `current` if it is the one in use, the form that
/// opens a copy of it, and the one that restores it but the current one.
/// The buttons name the version for screen readers.
fn version(
    r: &Renderer,
    path: &str,
    v: &VersionStatus,
    current: Option<&str>,
    form: &str,
) -> Markup {
    let saved = pigeon_proto::time::parse_rfc3339(&v.saved_at)
        .map_or_else(|| v.saved_at.clone(), |t| r.ago(crate::now() - t));
    let which = r.catalogue().format(
        &pigeon_i18n::Message::new("web-version-label")
            .arg("number", v.number.to_string())
            .arg("saved", saved),
    );
    html! {
        article {
            p {
                (r.version(v))
                @if let Some(current) = current { " — " strong { (current) } }
            }
            @if current.is_none() {
                form method="post" action=(r.url("/files/restore")) {
                    input type="hidden" name="form" value=(form);
                    input type="hidden" name="path" value=(path);
                    input type="hidden" name="version" value=(v.number);
                    button type="submit" { (r.id("web-restore")) (hidden(&which)) }
                }
            }
            form method="post" action=(r.url("/files/view")) {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="path" value=(path);
                input type="hidden" name="version" value=(v.number);
                button type="submit" { (r.id("web-view")) (hidden(&which)) }
            }
        }
    }
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
    let body = html! {
        (list(&other))
        @if rows.is_empty() && other.is_empty() { p { (r.id("deleted-none")) } }
        @if !folders.is_empty() {
            p { (r.id("web-restore-folder-help")) }
            @for folder in &folders {
                article {
                    p { (folder) }
                    form method="post" action=(r.url("/files/restore")) {
                        input type="hidden" name="form" value=(form.0);
                        input type="hidden" name="path" value=(std::path::Path::new(&root).join(folder).to_string_lossy());
                        button type="submit" { (r.id("web-restore-folder")) }
                    }
                }
            }
        }
        @for f in &rows {
            article {
                p { (r.deleted(f)) }
                form method="post" action=(r.url("/files/restore")) {
                    input type="hidden" name="form" value=(form.0);
                    input type="hidden" name="path" value=(std::path::Path::new(&root).join(&f.path).to_string_lossy());
                    button type="submit" { (r.id("web-restore-deleted")) }
                }
            }
        }
        p { a href=(r.url("/files")) { (r.id("web-back")) } }
    };
    page(&r, "/files", &r.id("web-deleted"), &body)
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
