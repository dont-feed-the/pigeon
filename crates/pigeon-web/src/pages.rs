//! The pages (10 §10.3, ux.md §6): each opens with its one-line answer,
//! in the frame of `shell.rs`. Every word comes from the catalogue (ADR
//! 0028). Home's headline card and the sidebar's status follow the server's
//! events through the client script; action results stay static.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use crate::Renderer;
use crate::icons::icon;
use crate::shell::{Head, document};
use axum::Extension;
use axum::extract::State;
use axum::response::Html;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::StreamExt;
use futures_util::stream::{self, Stream};
use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_proto::api::{Level, Suggestion};
use pigeon_proto::{Request, Response};

use crate::{FormToken, Web};

/// A page of section `here` that opens with `title` as its heading.
pub(crate) fn page(r: &Renderer, here: &str, title: &str, body: &Markup) -> Html<String> {
    Html(document(r, here, title, body, Head::Title))
}

/// A page whose `body` has its own heading; `title` names it in the tab.
pub(crate) fn bare(r: &Renderer, here: &str, title: &str, body: &Markup) -> Html<String> {
    Html(document(r, here, title, body, Head::Own))
}

/// A page that says only why the request was refused.
pub fn refused(web: &Web, id: &str) -> Html<String> {
    Html(crate::shell::refused(&web.renderer(), id))
}

/// The lines of the answers to `request`: a problem's, or the result's.
pub async fn lines(web: &Web, r: &Renderer, request: Request) -> Vec<String> {
    let mut out = Vec::new();
    for answer in web.ask(request).await {
        if !matches!(answer, Response::Progress { .. }) {
            out.extend(r.response(&answer).0);
        }
    }
    out
}

/// What `pigeon status` says, for Home and the live status.
struct Status {
    headline: String,
    rest: Vec<String>,
    level: Option<Level>,
    no_group: bool,
    health: Option<Box<pigeon_proto::health::HealthReport>>,
    denied: bool,
}

/// The headline and the lines under it of `pigeon status`.
async fn status(web: &Web, r: &Renderer) -> Status {
    let answers = web.ask(Request::Health).await;
    let health = answers.iter().find_map(|answer| match answer {
        Response::Health(report) => Some(report.clone()),
        _ => None,
    });
    let level = answers.iter().find_map(|answer| match answer {
        Response::Health(report) => Some(report.status.level),
        Response::Status(s) => Some(s.level),
        _ => None,
    });
    let problem = |code: &str| {
        answers
            .iter()
            .any(|a| matches!(a, Response::Problem(p) if p.code == code))
    };
    let mut all: Vec<String> = answers
        .iter()
        .filter(|a| !matches!(a, Response::Progress { .. }))
        .flat_map(|a| r.response(a).0)
        .collect();
    let rest = if all.is_empty() {
        Vec::new()
    } else {
        all.split_off(1)
    };
    Status {
        headline: all.pop().unwrap_or_default(),
        rest,
        level,
        no_group: problem("not-in-group"),
        health,
        denied: problem("web-forbidden"),
    }
}

/// A level as the client and the stylesheet name it.
fn level_name(level: Level) -> &'static str {
    match level {
        Level::NeedsYou => "needs-you",
        Level::NotSafe => "not-safe",
        Level::NotChecked => "not-checked",
        Level::Working => "working",
        Level::Safe => "safe",
    }
}

/// The icon of a headline's level.
fn level_icon(level: Option<Level>) -> &'static str {
    match level {
        Some(Level::Safe) => "safe",
        Some(Level::Working) => "working",
        Some(Level::NotSafe) => "clock",
        Some(Level::NeedsYou) => "alert",
        Some(Level::NotChecked) | None => "question",
    }
}

pub(crate) fn list(items: &[String]) -> Markup {
    html! { ul { @for i in items { li { (i.trim()) } } } }
}

/// Home: the headline, what is not safe yet and why, the deletions pigeon
/// holds (`held.rs`) and the group's health.
pub async fn home(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let s = status(&web, &r).await;
    let held = if s.no_group {
        html! {}
    } else {
        crate::held::section(&web, &r, &form.0).await
    };
    let class = s.level.map_or("headline-card".to_owned(), |l| {
        format!("headline-card lv-{}", level_name(l))
    });
    let body = html! {
        section id="headline-card" class=(class) aria-labelledby="headline" {
            (icon(level_icon(s.level), ""))
            div {
                h1 id="headline" aria-live="polite" { (s.headline) }
                ul id="health-explanations" class="plain-list" { @for line in &s.rest { li { (line.trim()) } } }
                noscript { p class="lead" { (r.id("web-noscript")) } }
            }
        }
        @if !s.no_group {
            nav class="shortcuts" aria-label=(r.id("web-shortcuts")) {
                @for (path, id, symbol) in [("/files", "web-nav-files", "folder"), ("/deleted", "web-deleted", "trash"), ("/suggestions", "web-nav-suggestions", "suggest"), ("/machines", "web-nav-machines", "people")] {
                    a href=(r.url(path)) { (icon(symbol, "")) span { (r.id(id)) } }
                }
            }
        }
        (held)
        @if let Some(health) = &s.health { (crate::health::section(&r, health)) }
        @if s.no_group && r.mount.is_none() {
            div class="card stack" {
                p { (r.id("web-setup-help")) }
                p { a class="btn primary" href=(r.url("/join")) { (icon("plus", "")) (r.id("web-setup")) } }
            }
        }
    };
    bare(&r, "/", &s.headline, &body)
}

/// Suggestions: those waiting for a decision, with *Accept* and *Decline*
/// on those this person decides (ux.md §6.3).
pub async fn suggestions(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let mut rows = Vec::new();
    let mut other = Vec::new();
    for answer in web.ask(Request::Proposals).await {
        match answer {
            Response::Proposals { proposals } => rows = proposals,
            Response::Progress { .. } => {}
            a => other.extend(r.response(&a).0),
        }
    }
    let body = html! {
        @if rows.is_empty() && other.is_empty() {
            div class="empty" {
                (icon("suggest", ""))
                strong { (r.id("suggestions-none")) }
                (r.id("web-suggestions-empty-help"))
            }
        }
        (list(&other))
        div class="stack" { @for s in &rows { (suggestion(&r, s, &form.0)) } }
    };
    page(&r, "/suggestions", &r.id("web-nav-suggestions"), &body)
}

/// The first letter of a name, for its avatar.
pub(crate) fn initial(name: &str) -> String {
    name.chars().next().map(String::from).unwrap_or_default()
}

/// One suggestion, and the forms that decide it.
fn suggestion(r: &Renderer, s: &Suggestion, form: &str) -> Markup {
    let saved = pigeon_proto::time::parse_rfc3339(&s.saved_at)
        .map_or_else(|| s.saved_at.clone(), |t| r.ago(crate::now() - t));
    let id = if s.mine {
        "web-suggestion-yours"
    } else if s.decide {
        "web-suggestion-decide"
    } else {
        "web-suggestion-other"
    };
    let msg = Message::new(id)
        .arg("author", s.author.clone())
        .arg("paths", s.paths.join(", "))
        .arg("saved", saved);
    let decides = s.decide && (!s.mine || s.force);
    html! {
        article class="card" {
            div class="person" {
                span class="avatar" aria-hidden="true" { (initial(&s.author)) }
                div class="grow" {
                    p { (r.catalogue().format(&msg)) }
                    p { a href=(r.url(&format!("/erase?suggestion={}", form_urlencoded::byte_serialize(s.id.as_bytes()).collect::<String>()))) { (r.id("web-erase-suggestion")) } }
                    div class="suggestion-paths" {
                        @for path in &s.paths {
                            form method="post" action=(r.url("/suggestions/view")) {
                                input type="hidden" name="form" value=(form);
                                input type="hidden" name="proposal" value=(s.id);
                                input type="hidden" name="path" value=(path);
                                button type="submit" class="small" {
                                    (crate::icons::file_icon(path)) (r.id("web-view-suggestion")) " — " (path)
                                }
                            }
                        }
                    }
                    @if decides {
                        div class="inline-forms" {
                            form method="post" action=(r.url("/suggestions/accept")) class="inline-forms" {
                                input type="hidden" name="form" value=(form);
                                input type="hidden" name="id" value=(s.id);
                                @if s.force {
                                    label { (r.id("web-override-reason")) " " input name="reason" required; }
                                }
                                button type="submit" class="primary" { (icon("check", "")) (r.id("web-accept")) }
                            }
                            form method="post" action=(r.url("/suggestions/decline")) class="inline-forms" {
                                input type="hidden" name="form" value=(form);
                                input type="hidden" name="id" value=(s.id);
                                label { (r.id("web-decline-note")) " " input type="text" name="note"; }
                                button type="submit" { (r.id("web-decline")) }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The result of an action: its first line as the title, the rest under
/// it, and the way back to `here`. A problem's result says so, for the
/// client's toast.
pub fn result(r: &Renderer, here: &str, mut lines: Vec<String>) -> Html<String> {
    let code = r
        .catalogue()
        .format(&Message::new("problem-code").arg("code", "\u{0}"));
    let code = code.split('\u{0}').next().unwrap_or_default().to_owned();
    let problem = !code.is_empty() && lines.iter().any(|l| l.starts_with(&code));
    let rest = if lines.is_empty() {
        Vec::new()
    } else {
        lines.split_off(1)
    };
    let title = lines.pop().unwrap_or_default();
    let body = html! {
        div class="card stack" data-result {
            @if problem { span data-problem hidden {} }
            (list(&rest))
            p { a class="btn" href=(r.url(here)) { (icon("back", "")) (r.id("web-back")) } }
        }
    };
    // An action result keeps its own title; only Home's headline is live.
    page(r, here, &title, &body)
}

/// Any other path: a page that says there is none, with the navigation.
pub async fn not_found(State(web): State<Arc<Web>>) -> (axum::http::StatusCode, Html<String>) {
    let r = web.renderer();
    let body = html! { p { a href=(r.url("/")) { (r.id("web-nav-home")) } } };
    (
        axum::http::StatusCode::NOT_FOUND,
        page(&r, "", &r.id("web-not-found"), &body),
    )
}

/// The headline and its level every 5 seconds, as `headline` and `level`
/// events, and Home's health facts as `health`.
pub async fn events(
    State(web): State<Arc<Web>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let s = stream::unfold((web, true), |(web, first)| async move {
        if !first {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        let r = web.renderer();
        let s = status(&web, &r).await;
        if s.denied {
            return None;
        }
        let mut events: Vec<Result<Event, Infallible>> =
            vec![Ok(Event::default().event("headline").data(s.headline))];
        if let Some(level) = s.level {
            events.push(Ok(Event::default().event("level").data(level_name(level))));
        }
        if let Some(health) = s.health {
            events.push(Ok(Event::default()
                .event("health")
                .data(crate::health::event(&r, &health))));
        }
        Some((events, (web, false)))
    })
    .flat_map(stream::iter);
    Sse::new(s).keep_alive(KeepAlive::default())
}
