//! Files whose copy here differs from the group's version (07 §7.6,
//! ux.md §8.8), from `pigeon conflicts`: Home lists them under the
//! headline, each a link to its page; the file's page says so and offers
//! the choices of `pigeon resolve PATH`, each saying where both versions
//! go: *Take the group's version*, *Keep mine as a copy* and, once the
//! suggestion is decided, *Suggest again*. A bridge's managed client,
//! without a folder of its own, resolves on the bridge's page instead
//! (10 §10.3), so its mounted pages show none of this.

use std::path::Path;
use std::sync::Arc;

use crate::Renderer;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Form};
use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_proto::Request;
use pigeon_proto::api::{Choice, Divergence};

use crate::files::enc;
use crate::machines::expired;
use crate::{FormToken, Web, pages};

/// The diverging paths, and the group's folder on this computer when any
/// path diverges and the folder could be read.
async fn diverging(web: &Web, r: &Renderer) -> (Option<String>, Vec<Divergence>) {
    let mut rows = Vec::new();
    if !r.storage_controls() {
        return (None, rows);
    }
    for answer in web.ask(Request::Conflicts).await {
        if let pigeon_proto::Response::Conflicts { conflicts } = answer {
            rows = conflicts;
        }
    }
    if rows.is_empty() {
        return (None, rows);
    }
    let root = web
        .ask(Request::Folders)
        .await
        .into_iter()
        .find_map(|answer| match answer {
            pigeon_proto::Response::Folders { root, .. } => Some(root),
            _ => None,
        })
        .filter(|root| !root.is_empty());
    (root, rows)
}

/// The page of a file, by its absolute path.
fn file_url(path: &str) -> String {
    format!("/file?path={}", enc(path))
}

/// Whether the person's version waits for a decision rather than for them.
fn waiting(d: &Divergence) -> bool {
    d.state == "waiting"
}

/// Home's list of the files that differ; nothing when none does.
pub async fn section(web: &Web, r: &Renderer) -> Markup {
    let (root, rows) = diverging(web, r).await;
    html! {
        @if !rows.is_empty() {
            h2 { (r.id("web-conflicts")) }
            ul {
                @for d in &rows {
                    @let id = if waiting(d) { "web-conflict-row-waiting" } else { "web-conflict-row" };
                    @let line = r.catalogue().format(&Message::new(id).arg("path", d.path.clone()));
                    li {
                        @if let Some(root) = &root {
                            a href=(r.url(&file_url(&Path::new(root).join(&d.path).to_string_lossy()))) { (line) }
                        } @else {
                            (line)
                        }
                    }
                }
            }
        }
    }
}

/// The default name of *Keep mine as a copy*: `recipes (my copy).txt`.
fn copy_name(r: &Renderer, path: &str) -> String {
    let p = Path::new(path);
    let stem = p
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let ext = p
        .extension()
        .map_or_else(String::new, |e| format!(".{}", e.to_string_lossy()));
    format!("{stem} ({}){ext}", r.id("web-resolve-copy-suffix"))
}

/// What the file's page says and offers when this computer's copy of
/// `path` (absolute) differs from the group's; `None` when it does not.
pub async fn on_file(web: &Web, r: &Renderer, path: &str, form: &str) -> Option<Markup> {
    let (root, rows) = diverging(web, r).await;
    let root = root?;
    let d = rows
        .iter()
        .find(|d| Path::new(&root).join(&d.path) == Path::new(path))?;
    let name = Path::new(path)
        .file_name()
        .map_or_else(|| d.path.clone(), |n| n.to_string_lossy().into_owned());
    let explain = if waiting(d) {
        "web-conflict-waiting-explain"
    } else {
        "web-conflict-explain"
    };
    let mut choices = vec![
        (
            "take-group",
            "web-resolve-take-group",
            if waiting(d) {
                "web-resolve-take-group-waiting-help"
            } else {
                "web-resolve-take-group-help"
            },
        ),
        (
            "keep-copy",
            "web-resolve-keep-copy",
            "web-resolve-keep-copy-help",
        ),
    ];
    if !waiting(d) {
        choices.push((
            "suggest-again",
            "web-resolve-suggest-again",
            "web-resolve-suggest-again-help",
        ));
    }
    Some(html! {
        h2 { (r.id("web-conflict-heading")) }
        p { (r.catalogue().format(&Message::new(explain).arg("name", name))) }
        @for (choice, label, help) in choices {
            article {
                p id=(format!("{choice}-help")) { (r.id(help)) }
                form method="post" action=(r.url("/files/resolve")) {
                    input type="hidden" name="form" value=(form);
                    input type="hidden" name="path" value=(path);
                    input type="hidden" name="choice" value=(choice);
                    @if choice == "keep-copy" {
                        label for="copy-name" { (r.id("web-resolve-copy-name")) }
                        " "
                        input id="copy-name" name="dest" value=(copy_name(r, path)) required;
                        " "
                    }
                    button type="submit" aria-describedby=(format!("{choice}-help")) { (r.id(label)) }
                }
            }
        }
    })
}

/// What the person chose on a file's page.
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Pick {
    /// *Take the group's version*.
    TakeGroup,
    /// *Keep mine as a copy*.
    KeepCopy,
    /// *Suggest again*.
    SuggestAgain,
}

/// The form of a choice.
#[derive(serde::Deserialize)]
pub struct ResolveForm {
    /// The session's form token.
    form: String,
    /// The file, as an absolute path.
    path: String,
    /// What the person chose.
    choice: Pick,
    /// The copy's name, from the file's folder, for *Keep mine as a copy*.
    #[serde(default)]
    dest: String,
}

/// A choice: `pigeon resolve PATH --take-group | --keep-copy DEST |
/// --suggest-again`; the result links back to the file's page.
pub async fn resolve(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<ResolveForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    let choice = match f.choice {
        Pick::TakeGroup => Choice::TakeGroup,
        Pick::SuggestAgain => Choice::SuggestAgain,
        Pick::KeepCopy => {
            let name = match f.dest.trim() {
                "" => copy_name(&r, &f.path),
                name => name.to_owned(),
            };
            let folder = Path::new(&f.path).parent().unwrap_or(Path::new("/"));
            Choice::KeepCopy {
                dest: folder.join(name).to_string_lossy().into_owned(),
            }
        }
    };
    let back = file_url(&f.path);
    let request = Request::Resolve {
        path: f.path,
        choice,
    };
    let lines = pages::lines(&web, &r, request).await;
    pages::result(&r, &back, lines).into_response()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_copy_keeps_its_extension_so_it_still_opens() {
        let r = crate::Renderer::new(
            pigeon_i18n::Catalogue::new(&"en".parse().unwrap(), false),
            0,
            None,
        );
        assert_eq!(
            super::copy_name(&r, "/g/Recettes/recipes.txt"),
            "recipes (my copy).txt"
        );
        assert_eq!(super::copy_name(&r, "/g/notes"), "notes (my copy)");
    }
}
