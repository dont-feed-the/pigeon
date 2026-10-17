//! The details panel of the Files browser (ux.md §3.8, progressive
//! disclosure): for a file, its state, size, last change, sharing rule and
//! latest versions, each to open or restore; for a folder, its files'
//! state, size, whether it is on this computer, and its sharing rule, with
//! *Keep* or *Free up space*. It is part of the page whose URL selects it.

use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_proto::api::VersionStatus;
use pigeon_proto::{Request, Response, policy};

use crate::Renderer;
use crate::Web;
use crate::browse::{Row, View, enc, keep_or_free};
use crate::format::{folder_state, size, state, when};
use crate::icons::{file_icon, icon};

/// How many versions the panel lists; the file's page lists them all.
const VERSIONS: usize = 5;

/// Who may change what a rule covers, in words, without its folder.
pub(crate) fn rule_summary(r: &Renderer, rule: &policy::Rule, people: &[policy::Person]) -> String {
    let name = |id: &str| {
        people
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| id.to_owned(), |p| p.name.clone())
    };
    let message = match &rule.owner {
        Some(owner) => Message::new("web-rule-owned").arg("owner", name(owner)),
        None if rule.per_person => Message::new("web-rule-per-person"),
        None => {
            let stewards: Vec<String> = if rule.stewards.is_empty() {
                people
                    .iter()
                    .filter(|p| p.admin)
                    .map(|p| p.name.clone())
                    .collect()
            } else {
                rule.stewards.iter().map(|id| name(id)).collect()
            };
            Message::new("web-rule-add-only").arg("stewards", stewards.join(", "))
        }
    };
    r.catalogue().format(&message)
}

/// The sharing rule of `folder` in words, if the daemon says.
async fn rule(web: &Web, r: &Renderer, folder: &str) -> Option<String> {
    web.ask(Request::FolderRules {
        folder: Some(folder.to_owned()),
    })
    .await
    .into_iter()
    .find_map(|a| match a {
        Response::FolderRules(report) => report
            .rules
            .first()
            .map(|rule| rule_summary(r, rule, &report.people)),
        _ => None,
    })
}

/// The versions of `file`, newest first, or what went wrong.
async fn versions(web: &Web, r: &Renderer, file: &str) -> (Vec<VersionStatus>, Vec<String>) {
    let mut versions = Vec::new();
    let mut other = Vec::new();
    for answer in web
        .ask(Request::History {
            path: file.to_owned(),
        })
        .await
    {
        match answer {
            Response::History { versions: v } => versions = v,
            Response::Progress { .. } => {}
            a => other.extend(r.response(&a).0),
        }
    }
    (versions, other)
}

/// One version, with *Open a copy* and, but for the newest, *Restore*.
pub(crate) fn version(
    r: &Renderer,
    path: &str,
    v: &VersionStatus,
    current: Option<&str>,
    form: &str,
) -> Markup {
    let saved = pigeon_proto::time::parse_rfc3339(&v.saved_at)
        .map_or_else(|| v.saved_at.clone(), |t| r.ago(crate::now() - t));
    let which = r.catalogue().format(
        &Message::new("web-version-label")
            .arg("number", v.number.to_string())
            .arg("saved", saved),
    );
    html! {
        li {
            strong { (r.catalogue().format(&Message::new("web-version-number").arg("number", v.number.to_string()))) }
            " " (state(r, v.state))
            span class="when" {
                (r.id("web-saved")) " " (when(r, &v.saved_at))
                @if let Some(current) = current { " · " strong { (current) } }
            }
            @if let Some(reason) = &v.reason { span class="when" { (r.text(reason)) } }
            div class="inline-forms" {
                form method="post" action=(r.url("/files/view")) {
                    input type="hidden" name="form" value=(form);
                    input type="hidden" name="path" value=(path);
                    input type="hidden" name="version" value=(v.number);
                    button type="submit" class="small" { (icon("download", "")) (r.id("web-view")) (crate::pages::hidden(&which)) }
                }
                @if current.is_none() {
                    form method="post" action=(r.url("/files/restore")) data-inline {
                        input type="hidden" name="form" value=(form);
                        input type="hidden" name="path" value=(path);
                        input type="hidden" name="version" value=(v.number);
                        button type="submit" class="small" { (icon("restore", "")) (r.id("web-restore")) (crate::pages::hidden(&which)) }
                    }
                }
            }
        }
    }
}

/// The panel of `row`, shown beside the table of `view`.
pub(crate) async fn panel(
    web: &Web,
    r: &Renderer,
    view: &View,
    row: &Row,
    root: &str,
    form: &str,
) -> Markup {
    let folder = if row.folder {
        row.abs.clone()
    } else {
        std::path::Path::new(&row.abs)
            .parent()
            .map_or_else(|| root.to_owned(), |p| p.to_string_lossy().into_owned())
    };
    let rule = rule(web, r, &folder).await;
    let (history, problems) = if row.folder {
        (Vec::new(), Vec::new())
    } else {
        versions(web, r, &row.abs).await
    };
    let newest = history.iter().map(|v| v.number).max();
    let differs = if row.folder {
        None
    } else {
        crate::conflicts::on_file(web, r, &row.abs, form).await
    };
    let suggested = if row.folder {
        html! {}
    } else {
        crate::suggestions::on_file(web, r, &row.abs).await
    };
    let current = r.id(if differs.is_some() {
        "web-version-group-current"
    } else {
        "web-version-current"
    });
    let count = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
    let open = if row.folder {
        r.url(&format!("/folder?path={}", enc(&row.abs)))
    } else {
        r.url(&format!("/file?path={}", enc(&row.abs)))
    };
    let close = r
        .catalogue()
        .format(&Message::new("web-details-close").arg("name", row.name.as_str()));
    html! {
        aside id="details" class="details" aria-labelledby="details-title" {
            div class="details-head" {
                @if row.folder { (icon("folder", "ft ft-folder")) } @else { (file_icon(&row.name)) }
                h2 id="details-title" tabindex="-1" { (row.name) }
                a class="icon-btn" href=(view.select(r, None)) data-details title=(close) {
                    (icon("close", "")) span class="visually-hidden" { (close) }
                }
            }
            div {
                @if row.folder { (folder_state(r, row.state, row.not_safe)) } @else { (crate::format::file_state(r, row.state, row.freed)) }
            }
            dl {
                @if row.folder {
                    dt { (r.id("web-detail-files")) }
                    dd { (r.catalogue().format(&Message::new("web-folder-files").arg("files", count(row.files)))) }
                }
                @if let Some(s) = row.size { dt { (r.id("web-col-size")) } dd { (size(r, s)) } }
                @if let Some(m) = &row.modified { dt { (r.id("web-col-modified")) } dd { (when(r, m)) } }
                @if row.folder && r.storage_controls() {
                    dt { (r.id("web-detail-here")) }
                    dd {
                        @if let Some(f) = &row.sync { (crate::sync::here(r, f)) }
                        @else { (r.id(if row.freed { "web-not-here" } else { "web-kept-here" })) }
                    }
                }
                @if let Some(rule) = &rule { dt { (r.id("web-detail-rule")) } dd { (rule) } }
            }
            div class="inline-forms" {
                a class="btn small" href=(open) {
                    (icon(if row.folder { "folder" } else { "history" }, ""))
                    (r.id(if row.folder { "web-open-folder" } else { "web-all-versions" }))
                }
                a class="btn small" href=(r.url(&format!("/folder-rule?path={}", enc(&folder)))) {
                    (icon("rule", "")) (r.id("web-folder-rule"))
                }
                @if row.folder && r.storage_controls() && (row.sync.is_some() || std::path::Path::new(&row.abs).parent() == Some(std::path::Path::new(root))) {
                    (keep_or_free(r, form, &row.abs, Some(row.freed), "small"))
                    @if row.sync.as_ref().is_some_and(|f| f.chosen) { (crate::sync::forget(r, form, &row.abs, "small")) }
                }
            }
            @if !row.folder {
                @if let Some(differs) = differs { (differs) }
                (suggested)
                h3 { (r.id("web-versions")) }
                @if !problems.is_empty() { (crate::pages::list(&problems)) }
                ol class="timeline" {
                    @for v in history.iter().take(VERSIONS) {
                        (version(r, &row.abs, v, (Some(v.number) == newest).then_some(current.as_str()), form))
                    }
                }
                @if history.len() > VERSIONS { p { a href=(open) { (r.id("web-all-versions")) } } }
            }
        }
    }
}
