//! The frame of every page (ux.md §6): a sidebar with the sections, the
//! live status and Settings, and a content panel that opens with the
//! page's one-line answer. The client script and stylesheet are the
//! embedded assets; every page works without the script.

use maud::{DOCTYPE, Markup, html};

use crate::Renderer;
use crate::assets::{ICON, SCRIPT, STYLE};
use crate::icons::{icon, sprite};

/// The sections, as (path, message id, icon, under the one before).
const SECTIONS: &[(&str, &str, &str, bool)] = &[
    ("/", "web-nav-home", "home", false),
    ("/files", "web-nav-files", "folder", false),
    ("/deleted", "web-deleted", "trash", true),
    ("/erase", "web-erase-list", "trash", true),
    ("/suggestions", "web-nav-suggestions", "suggest", false),
    ("/machines", "web-nav-machines", "people", false),
];

/// The links at the foot of the sidebar, as (path, message id, icon).
const FOOT: &[(&str, &str, &str)] = &[
    ("/space", "space-title", "disk"),
    ("/settings", "web-nav-settings", "gear"),
    ("/join", "web-setup", "plus"),
];

/// How a page's title is shown.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Head {
    /// As an `h1` opening the content.
    Title,
    /// Not at all: the content has its own `h1`.
    Own,
}

/// A link of the sidebar, current if it is `here`.
fn link(r: &Renderer, here: &str, path: &str, id: &str, symbol: &str) -> Markup {
    html! {
        a href=(r.url(path)) aria-current=[(path == here).then_some("page")] {
            (icon(symbol, "")) span { (r.id(id)) }
        }
    }
}

/// The whole document: the sidebar with `here` current, and `body`.
pub(crate) fn document(r: &Renderer, here: &str, title: &str, body: &Markup, head: Head) -> String {
    let lang = r.catalogue().lang().to_string();
    let app = r.id("web-title");
    html! {
        (DOCTYPE)
        html lang=(lang) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="light dark";
                title { @if title != app { (title) " – " } (app) }
                link rel="icon" href=(r.url(&format!("/assets/{}", ICON.name))) type="image/svg+xml";
                link rel="stylesheet" href=(r.url(&format!("/assets/{}", STYLE.name)));
                script type="module" src=(r.url(&format!("/assets/{}", SCRIPT.name))) {}
            }
            body data-events=(r.url("/events")) {
                (sprite())
                a class="skip" href="#main" { (r.id("web-skip")) }
                div class="shell" {
                    (sidebar(r, here))
                    main id="main" class="content" tabindex="-1" {
                        @if head == Head::Title {
                            header class="page-head" { h1 id="title" { (title) } }
                        }
                        (crate::pages::pause_banner(r, title, here == "/"))
                        (body)
                    }
                }
                div id="toasts" class="toasts" role="status" aria-live="polite" {}
            }
        }
    }
    .into_string()
}

fn sidebar(r: &Renderer, here: &str) -> Markup {
    html! {
        div class="sidebar" {
            a class="brand" href=(r.url("/")) {
                img src=(r.url(&format!("/assets/{}", ICON.name))) alt="";
                span { (r.id("web-title")) }
            }
            @if let Some(mount) = &r.mount {
                p class="mount no-print" {
                    a href=(&mount.back) { (icon("back", "")) " " (&mount.label) }
                }
                @if !mount.storage_controls { p class="mount" { (r.id("web-managed-client")) } }
            }
            nav aria-label=(r.id("web-sections")) {
                ul class="nav" {
                    @for (path, id, symbol, sub) in SECTIONS {
                        li class=[sub.then_some("sub")] { (link(r, here, path, id, symbol)) }
                        @if *path == "/files" && r.storage_controls() {
                            li class="sub" { (link(r, here, "/folders", "web-sync-title", "laptop")) }
                        }
                    }
                }
            }
            div class="sidebar-foot" {
                @if here != "/" {
                    a id="live-status" class="status-pill" href=(r.url("/")) hidden {
                        span class="dot" {}
                        span {
                            span class="visually-hidden" { (r.id("web-status")) " " }
                            span id="live-headline" {}
                        }
                    }
                }
                ul class="nav" {
                    @for (path, id, symbol) in FOOT {
                        @if r.mount.is_none() || *path != "/join" {
                            li { (link(r, here, path, id, symbol)) }
                        }
                    }
                }
            }
        }
    }
}

/// A breadcrumb trail: each (label, link) but the last, which is current.
pub(crate) fn crumbs(r: &Renderer, trail: &[(String, String)]) -> Markup {
    html! {
        nav class="crumbs" aria-label=(r.id("web-breadcrumb")) {
            ol {
                @for (i, (label, href)) in trail.iter().enumerate() {
                    li {
                        @if i + 1 == trail.len() {
                            a href=(href) aria-current="page" {
                                @if i == 0 { (icon("folder", "")) } (label)
                            }
                        } @else {
                            a href=(href) { @if i == 0 { (icon("folder", "")) } (label) }
                        }
                    }
                }
            }
        }
    }
}

/// A page with nothing but `id`'s sentence: why a request was refused.
pub(crate) fn refused(r: &Renderer, id: &str) -> String {
    html! {
        (DOCTYPE)
        html lang=(r.catalogue().lang().to_string()) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (r.id("web-title")) }
                link rel="stylesheet" href=(r.url(&format!("/assets/{}", STYLE.name)));
            }
            body { main id="main" class="content" { h1 { (r.id("web-title")) } p { (r.id(id)) } } }
        }
    }
    .into_string()
}
