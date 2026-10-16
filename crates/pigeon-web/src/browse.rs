//! The Files browser (ux.md §6.2): the group's folder as a table of
//! folders then files, each with an icon by kind, its state, size and last
//! change, sortable by column and filtered by name; a breadcrumb trail; the
//! selected row's details beside it (`details.rs`). Every view is a URL, so
//! links and reloads work
//! without the client, which only spares the page loads.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Extension;
use axum::extract::{Query, State as AxumState};
use axum::response::Html;
use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_proto::api::{FileEntry, State};
use pigeon_proto::{Request, Response};

use crate::format::{folder_state, rank, size, state, when};
use crate::icons::{file_icon, icon};
use crate::pages::{bare, list};
use crate::{FormToken, Renderer, Web};

/// A path in a query.
pub(crate) fn enc(path: &str) -> String {
    form_urlencoded::byte_serialize(path.as_bytes()).collect()
}

/// The query of the browser's pages.
#[derive(serde::Deserialize, Default)]
pub struct Browse {
    /// The folder, as an absolute path; the group's folder when absent.
    #[serde(default)]
    path: Option<String>,
    /// The column to sort by: `name`, `size`, `modified` or `state`.
    #[serde(default)]
    sort: Option<String>,
    /// `desc` for the reverse order.
    #[serde(default)]
    dir: Option<String>,
    /// The row whose details are shown, as an absolute path.
    #[serde(default)]
    selected: Option<String>,
}

/// One row: a folder or a file directly in the folder shown.
#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub name: String,
    pub abs: String,
    pub folder: bool,
    pub state: State,
    pub not_safe: u64,
    pub files: u64,
    pub size: Option<u64>,
    pub modified: Option<String>,
    pub freed: bool,
    pub arriving: u64,
}

/// The columns a table sorts by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sort {
    Name,
    Size,
    Modified,
    State,
}

impl Sort {
    fn parse(s: Option<&str>) -> Sort {
        match s {
            Some("size") => Sort::Size,
            Some("modified") => Sort::Modified,
            Some("state") => Sort::State,
            _ => Sort::Name,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Sort::Name => "name",
            Sort::Size => "size",
            Sort::Modified => "modified",
            Sort::State => "state",
        }
    }
}

/// Folders first, then by `sort`, reversed when `desc`; ties by name.
pub(crate) fn sort_rows(rows: &mut [Row], sort: Sort, desc: bool) {
    rows.sort_by(|a, b| {
        let by_name = a
            .name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.name.cmp(&b.name));
        let by = match sort {
            Sort::Name => by_name,
            Sort::Size => a.size.cmp(&b.size).then(by_name),
            Sort::Modified => a.modified.cmp(&b.modified).then(by_name),
            Sort::State => rank(a.state).cmp(&rank(b.state)).then(by_name),
        };
        b.folder
            .cmp(&a.folder)
            .then(if desc { by.reverse() } else { by })
    });
}

/// The rows directly under `prefix` (relative, `""` for the top) of the
/// files `listed`, each folder summing up the files under it.
pub(crate) fn rows_of(root: &str, prefix: &str, listed: &[FileEntry]) -> Vec<Row> {
    let mut folders: BTreeMap<String, Row> = BTreeMap::new();
    let mut files = Vec::new();
    let join = |name: &str| {
        let rel = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        std::path::Path::new(root)
            .join(rel)
            .to_string_lossy()
            .into_owned()
    };
    for f in listed {
        let relative = if prefix.is_empty() {
            f.path.as_str()
        } else {
            match f
                .path
                .strip_prefix(prefix)
                .and_then(|p| p.strip_prefix('/'))
            {
                Some(p) => p,
                None => continue,
            }
        };
        match relative.split_once('/') {
            Some((name, _)) => {
                let row = folders.entry(name.to_owned()).or_insert_with(|| Row {
                    name: name.to_owned(),
                    abs: join(name),
                    folder: true,
                    state: State::Protected,
                    not_safe: 0,
                    files: 0,
                    size: Some(0),
                    modified: None,
                    freed: false,
                    arriving: 0,
                });
                row.files += 1;
                if f.state != State::Protected {
                    row.not_safe += 1;
                }
                if rank(f.state) < rank(row.state) {
                    row.state = f.state;
                }
                row.size = Some(row.size.unwrap_or(0) + f.size.unwrap_or(0));
                if f.modified > row.modified {
                    row.modified.clone_from(&f.modified);
                }
            }
            None => files.push(Row {
                name: relative.to_owned(),
                abs: join(relative),
                folder: false,
                state: f.state,
                not_safe: u64::from(f.state != State::Protected),
                files: 1,
                size: f.size,
                modified: f.modified.clone(),
                freed: false,
                arriving: 0,
            }),
        }
    }
    folders.into_values().chain(files).collect()
}

/// Where the browser's links lead: this folder, sorted so.
pub(crate) struct View {
    pub path: Option<String>,
    pub sort: Sort,
    pub desc: bool,
}

impl View {
    /// This view's URL, with `sort`, `desc` and `selected`.
    pub(crate) fn href(
        &self,
        r: &Renderer,
        sort: Sort,
        desc: bool,
        selected: Option<&str>,
    ) -> String {
        let mut q = form_urlencoded::Serializer::new(String::new());
        if let Some(path) = &self.path {
            q.append_pair("path", path);
        }
        if sort != Sort::Name {
            q.append_pair("sort", sort.key());
        }
        if desc {
            q.append_pair("dir", "desc");
        }
        if let Some(s) = selected {
            q.append_pair("selected", s);
        }
        let q = q.finish();
        let base = if self.path.is_some() {
            "/folder"
        } else {
            "/files"
        };
        r.url(&if q.is_empty() {
            base.to_owned()
        } else {
            format!("{base}?{q}")
        })
    }

    /// The URL of this view with `selected`'s details.
    pub(crate) fn select(&self, r: &Renderer, selected: Option<&str>) -> String {
        self.href(r, self.sort, self.desc, selected)
    }
}

/// The page of a folder (`/folder?path=`) or of the group's folder.
async fn browse(web: &Web, r: &Renderer, form: &str, q: Browse) -> Html<String> {
    let mut other = Vec::new();
    let mut root = String::new();
    let mut tops = Vec::new();
    for answer in web.ask(Request::Folders).await {
        match answer {
            Response::Folders { root: at, folders } => {
                root = at;
                tops = folders;
            }
            Response::Progress { .. } => {}
            a if q.path.is_none() => other.extend(r.response(&a).0),
            _ => {}
        }
    }
    let folder = q.path.clone().unwrap_or_else(|| root.clone());
    let mut listed = Vec::new();
    if !folder.is_empty() {
        for answer in web
            .ask(Request::Files {
                folder: folder.clone(),
            })
            .await
        {
            match answer {
                Response::Files { root: at, files } => {
                    root = at;
                    listed = files;
                }
                Response::Progress { .. } => {}
                a => other.extend(r.response(&a).0),
            }
        }
    }
    let prefix = std::path::Path::new(&folder)
        .strip_prefix(&root)
        .ok()
        .and_then(|p| p.to_str())
        .unwrap_or_default()
        .trim_matches('/')
        .to_owned();
    let mut rows = rows_of(&root, &prefix, &listed);
    let top = prefix.split('/').next().unwrap_or_default();
    let top_folder = tops.iter().find(|t| t.path == top);
    for row in rows.iter_mut().filter(|row| row.folder) {
        let summary = if prefix.is_empty() {
            tops.iter().find(|t| t.path == row.name)
        } else {
            top_folder
        };
        if let Some(t) = summary {
            row.freed = t.freed;
            if prefix.is_empty() {
                row.arriving = t.arriving;
                row.size = t.size.or(row.size);
            }
        }
    }
    if prefix.is_empty() {
        for t in &tops {
            if !rows.iter().any(|row| row.folder && row.name == t.path) {
                rows.push(Row {
                    name: t.path.clone(),
                    abs: std::path::Path::new(&root)
                        .join(&t.path)
                        .to_string_lossy()
                        .into_owned(),
                    folder: true,
                    state: t.state,
                    not_safe: t.not_safe,
                    files: t.files,
                    size: t.size,
                    modified: None,
                    freed: t.freed,
                    arriving: t.arriving,
                });
            }
        }
    }
    let view = View {
        path: q.path.clone(),
        sort: Sort::parse(q.sort.as_deref()),
        desc: q.dir.as_deref() == Some("desc"),
    };
    sort_rows(&mut rows, view.sort, view.desc);
    let selected = q
        .selected
        .as_deref()
        .and_then(|s| rows.iter().find(|row| row.abs == s));
    let details = match selected {
        Some(row) => Some(crate::details::panel(web, r, &view, row, &root, form).await),
        None => None,
    };
    let title = if prefix.is_empty() {
        r.id("web-nav-files")
    } else {
        prefix.rsplit('/').next().unwrap_or(&prefix).to_owned()
    };
    let mut trail = vec![(r.id("web-nav-files"), r.url("/files"))];
    let mut at = std::path::PathBuf::from(&root);
    for part in prefix.split('/').filter(|p| !p.is_empty()) {
        at.push(part);
        trail.push((
            part.to_owned(),
            r.url(&format!("/folder?path={}", enc(&at.to_string_lossy()))),
        ));
    }
    // Only a top folder's state is known; a nested one offers both actions.
    let top_freed = top_folder
        .filter(|_| !prefix.contains('/'))
        .map(|t| t.freed);
    let freed_here = !prefix.is_empty() && top_freed == Some(true);
    let body = html! {
        header class="page-head" {
            @if trail.len() > 1 { (crate::shell::crumbs(r, &trail)) }
            h1 id="title" { (title) }
            @if prefix.is_empty() && !root.is_empty() {
                p class="lead" { (r.catalogue().format(&Message::new("web-files-lead").arg("folder", root.as_str()))) }
            }
            @if freed_here { p class="note warn" { (r.id("web-folder-freed-note")) } }
        }
        (list(&other))
        div class="toolbar" {
            label class="filter" data-filter hidden {
                (icon("search", ""))
                span class="visually-hidden" { (r.id("web-filter")) }
                input type="search" placeholder=(r.id("web-filter-placeholder")) autocomplete="off";
            }
            span class="spacer" {}
            @if !prefix.is_empty() {
                a class="btn" href=(r.url(&format!("/folder-rule?path={}", enc(&folder)))) { (icon("rule", "")) (r.id("web-folder-rule")) }
                @if r.storage_controls() && !listed.is_empty() {
                    (keep_or_free(r, form, &folder, top_freed, ""))
                }
            }
        }
        div class={ "browser" @if details.is_some() { " has-details" } } {
            div {
                @if rows.is_empty() && other.is_empty() {
                    div class="empty" {
                        (icon("folder", ""))
                        strong { (r.id("files-none")) }
                        @if !root.is_empty() { (r.catalogue().format(&Message::new("web-files-empty-help").arg("folder", root.as_str()))) }
                    }
                } @else if !rows.is_empty() {
                    div class="table-wrap" { (table(r, &view, &rows, q.selected.as_deref(), form, prefix.is_empty())) }
                    p class="empty" data-filter-empty hidden { (r.id("web-filter-none")) }
                }
            }
            @if let Some(d) = details { (d) }
        }
    };
    bare(r, "/files", &title, &body)
}

/// *Keep on this computer* or *Free up space*, or both when it is not
/// known which the folder is; inline when the client runs.
pub(crate) fn keep_or_free(
    r: &Renderer,
    form: &str,
    folder: &str,
    freed: Option<bool>,
    class: &str,
) -> Markup {
    let actions: &[(&str, &str, &str)] = match freed {
        Some(true) => &[("/files/keep", "web-keep", "download")],
        Some(false) => &[("/files/free", "web-free", "folder-off")],
        None => &[
            ("/files/free", "web-free", "folder-off"),
            ("/files/keep", "web-keep", "download"),
        ],
    };
    html! {
        @for (action, label, symbol) in actions {
            form method="post" action=(r.url(action)) data-inline {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="folder" value=(folder);
                button type="submit" class=(class) { (icon(symbol, "")) span class="label" { (r.id(label)) } }
            }
        }
    }
}

/// A column's heading, a link that sorts by it.
fn heading(r: &Renderer, view: &View, sort: Sort, id: &str, class: &str) -> Markup {
    let current = view.sort == sort;
    let desc = current && !view.desc;
    let order = match (current, view.desc) {
        (false, _) => None,
        (true, false) => Some("ascending"),
        (true, true) => Some("descending"),
    };
    html! {
        th scope="col" class=(class) aria-sort=[order] {
            a href=(view.href(r, sort, desc, None)) {
                (r.id(id))
                @if current { (icon(if view.desc { "down" } else { "up" }, "")) }
            }
        }
    }
}

fn table(
    r: &Renderer,
    view: &View,
    rows: &[Row],
    selected: Option<&str>,
    form: &str,
    top: bool,
) -> Markup {
    html! {
        table class="files" data-files {
            caption class="visually-hidden" { (r.id("web-files-caption")) }
            thead {
                tr {
                    th scope="col" class="col-icon" { span class="visually-hidden" { (r.id("web-col-kind")) } }
                    (heading(r, view, Sort::Name, "web-col-name", "col-name"))
                    (heading(r, view, Sort::State, "web-col-state", "col-state"))
                    (heading(r, view, Sort::Size, "web-col-size", "col-size"))
                    (heading(r, view, Sort::Modified, "web-col-modified", "col-when"))
                    th scope="col" class="col-actions" { span class="visually-hidden" { (r.id("web-col-actions")) } }
                }
            }
            tbody {
                @for row in rows { (table_row(r, view, row, selected == Some(row.abs.as_str()), form, top)) }
            }
        }
    }
}

fn table_row(
    r: &Renderer,
    view: &View,
    row: &Row,
    selected: bool,
    form: &str,
    top: bool,
) -> Markup {
    let href = if row.folder {
        r.url(&format!("/folder?path={}", enc(&row.abs)))
    } else {
        r.url(&format!("/file?path={}", enc(&row.abs)))
    };
    let details = view.select(r, Some(&row.abs));
    let count = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
    html! {
        tr data-name=(row.name) data-details-href=(details) class=[selected.then_some("selected")] {
            td class="col-icon" {
                @if row.folder {
                    (icon(if row.freed { "folder-off" } else { "folder" }, if row.freed { "ft ft-folder-off" } else { "ft ft-folder" }))
                } @else { (file_icon(&row.name)) }
            }
            td class="col-name" {
                a class="name" href=(href) { (row.name) }
                @if row.folder {
                    span class="sub" {
                        (r.catalogue().format(&Message::new("web-folder-files").arg("files", count(row.files))))
                        @if row.freed { " · " (r.id("web-not-here")) }
                    }
                    @if r.storage_controls() && row.arriving > 0 {
                        span class="sub" { (r.catalogue().format(&Message::new("web-files-arriving").arg("files", count(row.arriving)))) }
                    }
                }
            }
            td class="col-state" {
                @if row.folder { (folder_state(r, row.state, row.not_safe)) } @else { (state(r, row.state)) }
            }
            td class="num col-size" { @if let Some(s) = row.size { (size(r, s)) } }
            td class="when col-when" { @if let Some(m) = &row.modified { (when(r, m)) } }
            td class="col-actions" {
                div class="row-actions" {
                    @if !row.folder {
                        a class="icon-btn hover" href=(href) title=(r.id("web-versions")) {
                            (icon("history", ""))
                            span class="visually-hidden" { (r.catalogue().format(&Message::new("web-versions-of").arg("name", row.name.as_str()))) }
                        }
                    }
                    a class="icon-btn hover" href=(details) data-details title=(r.id("web-details")) {
                        (icon("info", ""))
                        span class="visually-hidden" { (r.catalogue().format(&Message::new("web-details-of").arg("name", row.name.as_str()))) }
                    }
                    @if row.folder && top && r.storage_controls() {
                        (keep_or_free(r, form, &row.abs, Some(row.freed), "small"))
                    }
                }
            }
        }
    }
}

/// `/files`: the group's folder.
pub async fn files(
    AxumState(web): AxumState<Arc<Web>>,
    Extension(form): Extension<FormToken>,
    Query(mut q): Query<Browse>,
) -> Html<String> {
    q.path = None;
    let r = web.renderer();
    browse(&web, &r, &form.0, q).await
}

/// `/folder?path=`: one folder of the group.
pub async fn folder_page(
    AxumState(web): AxumState<Arc<Web>>,
    Extension(form): Extension<FormToken>,
    Query(q): Query<Browse>,
) -> Html<String> {
    let r = web.renderer();
    browse(&web, &r, &form.0, q).await
}

#[cfg(test)]
mod tests {
    use super::{Sort, rows_of, sort_rows};
    use pigeon_proto::api::{FileEntry, State};

    fn file(path: &str, state: State, size: u64, modified: &str) -> FileEntry {
        FileEntry {
            path: path.into(),
            state,
            size: Some(size),
            modified: Some(modified.into()),
        }
    }

    #[test]
    fn a_folder_sums_up_the_files_under_it() {
        let listed = [
            file(
                "Photos/2019/a.jpg",
                State::Protected,
                10,
                "2026-01-01T00:00:00Z",
            ),
            file(
                "Photos/2019/deep/b.jpg",
                State::Published,
                5,
                "2026-03-01T00:00:00Z",
            ),
            file("Photos/c.jpg", State::Protected, 1, "2026-02-01T00:00:00Z"),
            file("Other/d.txt", State::Local, 1, "2026-02-01T00:00:00Z"),
        ];
        let rows = rows_of("/g", "Photos", &listed);
        assert_eq!(rows.len(), 2);
        let folder = &rows[0];
        assert!(folder.folder);
        assert_eq!(
            (folder.name.as_str(), folder.abs.as_str()),
            ("2019", "/g/Photos/2019")
        );
        assert_eq!(
            (folder.files, folder.not_safe, folder.size),
            (2, 1, Some(15))
        );
        assert_eq!(folder.state, State::Published);
        assert_eq!(folder.modified.as_deref(), Some("2026-03-01T00:00:00Z"));
        assert_eq!((rows[1].name.as_str(), rows[1].folder), ("c.jpg", false));
        assert_eq!(rows_of("/g", "", &listed).len(), 2, "two top folders");
    }

    #[test]
    fn folders_come_first_whatever_the_order() {
        let listed = [
            file("b.txt", State::Protected, 3, "2026-01-02T00:00:00Z"),
            file("A.txt", State::Local, 9, "2026-01-01T00:00:00Z"),
            file("z/x", State::Protected, 1, "2026-01-03T00:00:00Z"),
        ];
        let mut rows = rows_of("/g", "", &listed);
        let names = |rows: &[super::Row]| rows.iter().map(|r| r.name.clone()).collect::<Vec<_>>();
        sort_rows(&mut rows, Sort::Name, false);
        assert_eq!(names(&rows), ["z", "A.txt", "b.txt"]);
        sort_rows(&mut rows, Sort::Size, true);
        assert_eq!(names(&rows), ["z", "A.txt", "b.txt"]);
        sort_rows(&mut rows, Sort::Modified, true);
        assert_eq!(names(&rows), ["z", "b.txt", "A.txt"]);
        sort_rows(&mut rows, Sort::State, false);
        assert_eq!(names(&rows), ["z", "A.txt", "b.txt"]);
    }
}
