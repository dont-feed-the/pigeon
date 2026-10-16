//! Which folders are on this computer (08 §8.5), as file sync clients let
//! people choose: each folder of the browser says whether it is here, and
//! whose it is, with *Keep on this computer* or *Free up space* and, for a
//! folder this computer chose, *Use the default*; and *Folders on this
//! computer* shows the whole tree with a box per folder, which `POST
//! /folders` turns into the fewest `Keep` and `Evict` requests. A choice of
//! a folder covers the folders in it, as the daemon's rule says.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::Extension;
use axum::extract::{RawForm, State};
use axum::response::{Html, IntoResponse, Response};
use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_proto::api::SyncFolder;
use pigeon_proto::{Request, Response as Answer};

use crate::icons::icon;
use crate::machines::expired;
use crate::pages::{self, page};
use crate::{FormToken, Renderer, Web};

/// The folders of the group and whether each is on this computer.
pub(crate) struct Sync {
    /// The group's folder.
    pub root: String,
    /// Each folder by its path in the group, parents first.
    pub folders: BTreeMap<String, SyncFolder>,
}

impl Sync {
    /// The folder at `rel`, if the daemon listed it.
    pub(crate) fn get(&self, rel: &str) -> Option<&SyncFolder> {
        self.folders.get(rel)
    }

    /// `rel` in the group as an absolute path.
    pub(crate) fn abs(&self, rel: &str) -> String {
        std::path::Path::new(&self.root)
            .join(rel)
            .to_string_lossy()
            .into_owned()
    }
}

/// What the daemon says of the group's folders, or `None` if it cannot.
pub(crate) async fn read(web: &Web) -> Option<Sync> {
    web.ask(Request::Sync { folder: None })
        .await
        .into_iter()
        .find_map(|answer| match answer {
            Answer::Sync { root, folders } => Some(Sync {
                root,
                folders: folders.into_iter().map(|f| (f.path.clone(), f)).collect(),
            }),
            _ => None,
        })
}

/// Where a folder is, in words: on this computer or not, whose, and
/// whether this computer chose so.
pub(crate) fn here(r: &Renderer, f: &SyncFolder) -> String {
    let mut text = match (&f.others, f.kept) {
        (_, true) => r.id("web-kept-here"),
        (Some(owner), false) => r
            .catalogue()
            .format(&Message::new("web-here-others").arg("owner", owner.as_str())),
        (None, false) => r.id("web-not-here"),
    };
    if f.chosen {
        text.push_str(" · ");
        text.push_str(&r.id("web-here-chosen"));
    }
    text
}

/// *Use the default*: forgets this computer's choice of `folder`.
pub(crate) fn forget(r: &Renderer, form: &str, folder: &str, class: &str) -> Markup {
    html! {
        form method="post" action=(r.url("/files/forget")) data-inline {
            input type="hidden" name="form" value=(form);
            input type="hidden" name="folder" value=(folder);
            button type="submit" class=(class) { (icon("restore", "")) span class="label" { (r.id("web-forget")) } }
        }
    }
}

/// The requests that make each folder of `folders` kept as `keep` says,
/// parents first: a folder is chosen only where what covers it so far
/// differs, and choosing it covers every folder in it.
pub(crate) fn plan(folders: &[SyncFolder], keep: &BTreeSet<String>) -> Vec<(String, bool)> {
    let mut now: BTreeMap<&str, bool> = folders.iter().map(|f| (f.path.as_str(), f.kept)).collect();
    let mut sorted: Vec<&SyncFolder> = folders.iter().collect();
    sorted.sort_by_key(|f| (f.path.matches('/').count(), f.path.clone()));
    let mut out = Vec::new();
    for f in sorted {
        let want = keep.contains(&f.path);
        if now.get(f.path.as_str()) == Some(&want) {
            continue;
        }
        let prefix = format!("{}/", f.path);
        for (path, kept) in &mut now {
            if *path == f.path || path.starts_with(&prefix) {
                *kept = want;
            }
        }
        out.push((f.path.clone(), want));
    }
    out
}

/// Whether some folder in `path` is not kept as `path` is.
fn mixed(folders: &BTreeMap<String, SyncFolder>, path: &str, kept: bool) -> bool {
    let prefix = format!("{path}/");
    folders
        .range(prefix.clone()..)
        .take_while(|(p, _)| p.starts_with(&prefix))
        .any(|(_, f)| f.kept != kept)
}

/// The folders directly in `parent` (`""` for the top).
fn children<'a>(folders: &'a BTreeMap<String, SyncFolder>, parent: &str) -> Vec<&'a SyncFolder> {
    folders
        .values()
        .filter(|f| match f.path.rsplit_once('/') {
            Some((p, _)) => p == parent,
            None => parent.is_empty(),
        })
        .collect()
}

fn tree(r: &Renderer, sync: &Sync, parent: &str, depth: usize, next: &mut usize) -> Markup {
    let items = children(&sync.folders, parent);
    html! {
        ul class="tree" data-tree[depth == 0] {
            @for f in items {
                @let id = { *next += 1; format!("folder-{next}") };
                @let name = f.path.rsplit('/').next().unwrap_or(&f.path);
                @let inner = children(&sync.folders, &f.path);
                @let partly = mixed(&sync.folders, &f.path, f.kept);
                li {
                    div class="tree-row" {
                        input type="checkbox" id=(id) name="keep" value=(f.path) checked[f.kept];
                        input type="hidden" name="shown" value=(f.path);
                        label for=(id) {
                            (icon(if f.kept { "folder" } else { "folder-off" }, if f.kept { "ft-folder" } else { "ft-folder-off" }))
                            (name)
                        }
                        span class="sub" {
                            (here(r, f))
                            @if partly { span data-mixed-note { " · " (r.id("web-sync-partly")) } }
                        }
                        @if f.chosen {
                            button type="submit" class="small" formaction=(r.url("/files/forget")) name="folder" value=(sync.abs(&f.path)) {
                                (icon("restore", "")) (r.id("web-forget"))
                            }
                        }
                    }
                    @if !inner.is_empty() {
                        details open[depth == 0 || partly] {
                            summary {
                                (r.catalogue().format(&Message::new("web-sync-inside")
                                    .arg("count", i64::try_from(inner.len()).unwrap_or(i64::MAX))
                                    .arg("name", name)))
                            }
                            (tree(r, sync, &f.path, depth + 1, next))
                        }
                    }
                }
            }
        }
    }
}

/// *Folders on this computer*: every folder of the group with its box.
pub async fn view(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let Some(sync) = read(&web).await else {
        let lines = pages::lines(&web, &r, Request::Sync { folder: None }).await;
        return pages::result(&r, "/files", lines);
    };
    let mut next = 0;
    let body = html! {
        p class="lead" { (r.id("web-sync-lead")) }
        p class="note" { (r.id("web-sync-help")) }
        @if sync.folders.is_empty() {
            div class="empty" { (icon("folder", "")) strong { (r.id("sync-none")) } }
        } @else {
            form method="post" action=(r.url("/folders")) class="card stack" {
                input type="hidden" name="form" value=(form.0);
                (tree(&r, &sync, "", 0, &mut next))
                p { button type="submit" class="primary" { (icon("check", "")) (r.id("web-sync-apply")) } }
            }
        }
    };
    page(&r, "/folders", &r.id("web-sync-title"), &body)
}

/// `POST /folders`: keeps the ticked folders and frees the others shown.
pub async fn apply(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    RawForm(raw): RawForm,
) -> Response {
    let fields: Vec<(String, String)> = form_urlencoded::parse(&raw).into_owned().collect();
    let tokens: Vec<&str> = fields
        .iter()
        .filter(|(k, _)| k == "form")
        .map(|(_, v)| v.as_str())
        .collect();
    if let Some(refused) = expired(&web, &token, if tokens.len() == 1 { tokens[0] } else { "" }) {
        return refused;
    }
    let r = web.renderer();
    let Some(sync) = read(&web).await else {
        let lines = pages::lines(&web, &r, Request::Sync { folder: None }).await;
        return pages::result(&r, "/folders", lines).into_response();
    };
    let values = |name: &str| -> BTreeSet<String> {
        fields
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .collect()
    };
    let (keep, shown) = (values("keep"), values("shown"));
    // Only the folders the form showed are decided; the others keep theirs.
    let folders: Vec<SyncFolder> = sync
        .folders
        .values()
        .filter(|f| shown.contains(&f.path))
        .cloned()
        .collect();
    let changes = plan(&folders, &keep);
    let confirmed = fields
        .iter()
        .find(|(key, _)| key == "confirm")
        .and_then(|(_, value)| value.parse::<u8>().ok())
        .unwrap_or(0);
    if let Some(response) = confirmation(&web, &r, &sync, &changes, &fields, confirmed).await {
        return response;
    }
    let mut lines = Vec::new();
    for (path, kept) in changes {
        let folder = sync.abs(&path);
        let request = if kept {
            Request::Keep { folder }
        } else {
            Request::Evict {
                folder,
                confirmed: confirmed > 1,
            }
        };
        lines.extend(pages::lines(&web, &r, request).await);
    }
    if lines.is_empty() {
        lines.push(r.id("web-sync-unchanged"));
    }
    pages::result(&r, "/folders", lines).into_response()
}

/// Preview every folder being freed before changing any choice. Quiet
/// backups require a second confirmation, as the individual action does.
async fn confirmation(
    web: &Web,
    r: &Renderer,
    sync: &Sync,
    changes: &[(String, bool)],
    fields: &[(String, String)],
    confirmed: u8,
) -> Option<Response> {
    let mut previews = Vec::new();
    for (path, kept) in changes {
        if *kept {
            continue;
        }
        let answers = web
            .ask(Request::FreePreview {
                folder: sync.abs(path),
            })
            .await;
        let Some(preview) = answers.iter().find_map(|answer| match answer {
            Answer::FreePreview(preview) => Some(preview.clone()),
            _ => None,
        }) else {
            let lines = answers
                .iter()
                .flat_map(|answer| r.response(answer).0)
                .collect();
            return Some(pages::result(r, "/folders", lines).into_response());
        };
        previews.push((path, preview));
    }
    let quiet = previews.iter().any(|(_, preview)| preview.quiet);
    if previews.is_empty() || confirmed > 1 || (confirmed == 1 && !quiet) {
        return None;
    }
    let next = if confirmed == 0 { 1 } else { 2 };
    let label = if confirmed == 0 {
        "web-free"
    } else {
        "web-free-anyway"
    };
    let body = html! {
        @for (path, preview) in &previews {
            h2 { (path) }
            (pages::list(&r.free_preview(preview)))
        }
        form method="post" action=(r.url("/folders")) {
            @for (key, value) in fields {
                @if matches!(key.as_str(), "form" | "keep" | "shown") {
                    input type="hidden" name=(key) value=(value);
                }
            }
            input type="hidden" name="confirm" value=(next);
            button type="submit" { (r.id(label)) }
        }
        p { a href=(r.url("/folders")) { (r.id("web-free-cancel")) } }
    };
    Some(page(r, "/folders", &r.id("web-sync-title"), &body).into_response())
}

/// The form of *Use the default*.
#[derive(serde::Deserialize)]
pub struct ForgetForm {
    /// The session's form token.
    form: String,
    /// The folder, as an absolute path.
    folder: String,
}

/// `POST /files/forget`: `pigeon sync FOLDER --default`.
pub async fn forget_choice(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    axum::Form(f): axum::Form<ForgetForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    let lines = pages::lines(&web, &r, Request::ForgetChoice { folder: f.folder }).await;
    pages::result(&r, "/folders", lines).into_response()
}

#[cfg(test)]
mod tests {
    use super::plan;
    use pigeon_proto::api::SyncFolder;
    use std::collections::BTreeSet;

    fn folder(path: &str, kept: bool) -> SyncFolder {
        SyncFolder {
            path: path.into(),
            kept,
            chosen: false,
            others: None,
        }
    }

    fn keep(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn a_choice_of_a_folder_covers_the_folders_in_it() {
        let all = [
            folder("Photos", true),
            folder("Photos/2019", true),
            folder("Photos/2020", true),
            folder("src", true),
            folder("src/robin", false),
        ];
        assert_eq!(
            plan(
                &all,
                &keep(&["Photos", "Photos/2019", "Photos/2020", "src"])
            ),
            [],
            "nothing changes"
        );
        assert_eq!(
            plan(&all, &keep(&["src"])),
            [("Photos".to_owned(), false)],
            "one request frees a whole folder"
        );
        assert_eq!(
            plan(&all, &keep(&["Photos", "Photos/2020", "src", "src/robin"])),
            [
                ("Photos/2019".to_owned(), false),
                ("src/robin".to_owned(), true)
            ]
        );
        assert_eq!(
            plan(&all, &keep(&["Photos/2019"])),
            [
                ("Photos".to_owned(), false),
                ("src".to_owned(), false),
                ("Photos/2019".to_owned(), true)
            ],
            "a folder kept inside a freed one is chosen after it"
        );
    }

    #[test]
    fn freeing_a_folder_then_keeping_it_back_needs_no_request_below_it() {
        let all = [
            folder("a", false),
            folder("a/b", false),
            folder("a/b/c", true),
        ];
        assert_eq!(
            plan(&all, &keep(&["a", "a/b", "a/b/c"])),
            [("a".to_owned(), true)]
        );
    }
}
