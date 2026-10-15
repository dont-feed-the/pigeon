//! The pages (10 §10.3, ux.md §6): each opens with its one-line answer,
//! under the five sections' navigation. Every word comes from the
//! catalogue (ADR 0028); the pages hold no script but the one line that
//! puts the headline the events bring in place.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Extension;
use axum::extract::State;
use axum::response::Html;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{self, Stream};
use maud::{DOCTYPE, Markup, html};
use pigeon_i18n::Message;
use pigeon_i18n::render::Renderer;
use pigeon_proto::api::Suggestion;
use pigeon_proto::{Request, Response};

use crate::{FormToken, Web};

/// The sections, as (path, message id); Settings comes later.
const SECTIONS: &[(&str, &str)] = &[
    ("/", "web-nav-home"),
    ("/files", "web-nav-files"),
    ("/suggestions", "web-nav-suggestions"),
    ("/machines", "web-nav-machines"),
];

const STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:46rem;margin:2rem auto;\
padding:0 1rem;line-height:1.5;color:#1b1b1b}nav a{margin-right:1.2rem}\
nav a[aria-current]{font-weight:bold}h1{font-size:1.5rem}li{margin:.3rem 0}\
article{border-top:1px solid #ccc;padding:.5rem 0}form{display:inline-block;margin-right:1rem}";

/// Puts each `headline` event's text in the page's headline.
const SCRIPT: &str = "new EventSource('/events').addEventListener('headline',\
e=>{document.getElementById('headline').textContent=e.data})";

pub(crate) fn page(r: &Renderer, here: &str, headline: &str, body: &Markup) -> Html<String> {
    let lang = r.catalogue().lang().to_string();
    Html(
        html! {
            (DOCTYPE)
            html lang=(lang) {
                head {
                    meta charset="utf-8";
                    meta name="viewport" content="width=device-width, initial-scale=1";
                    title { (r.id("web-title")) }
                    style { (STYLE) }
                }
                body {
                    nav aria-label=(r.id("web-title")) {
                        @for (path, id) in SECTIONS {
                            @if *path == here {
                                a href=(path) aria-current="page" { (r.id(id)) }
                            } @else {
                                a href=(path) { (r.id(id)) }
                            }
                        }
                    }
                    main {
                        h1 id=(if here == "/" { "headline" } else { "title" }) { (headline) }
                        (body)
                    }
                    @if here == "/" { script { (maud::PreEscaped(SCRIPT)) } }
                }
            }
        }
        .into_string(),
    )
}

/// A page that says only why the request was refused.
pub fn refused(web: &Web, id: &str) -> Html<String> {
    let r = web.renderer();
    Html(
        html! {
            (DOCTYPE)
            html lang=(r.catalogue().lang().to_string()) {
                head { meta charset="utf-8"; title { (r.id("web-title")) } style { (STYLE) } }
                body { main { p { (r.id(id)) } } }
            }
        }
        .into_string(),
    )
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

/// The headline and the lines under it of `pigeon status`.
async fn status(web: &Web, r: &Renderer) -> (String, Vec<String>) {
    let mut all = lines(
        web,
        r,
        Request::Status {
            path: None,
            not_shared: false,
        },
    )
    .await;
    let rest = if all.is_empty() {
        Vec::new()
    } else {
        all.split_off(1)
    };
    (all.pop().unwrap_or_default(), rest)
}

pub(crate) fn list(items: &[String]) -> Markup {
    html! { ul { @for i in items { li { (i.trim()) } } } }
}

/// Home: the headline, and what is not safe yet and why.
pub async fn home(State(web): State<Arc<Web>>) -> Html<String> {
    let r = web.renderer();
    let (headline, rest) = status(&web, &r).await;
    page(&r, "/", &headline, &list(&rest))
}

/// People & machines: the group's computers and backup machines.
pub async fn machines(State(web): State<Arc<Web>>) -> Html<String> {
    let r = web.renderer();
    let rows = lines(&web, &r, Request::Machines).await;
    page(&r, "/machines", &r.id("web-nav-machines"), &list(&rows))
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
        @if rows.is_empty() && other.is_empty() { p { (r.id("suggestions-none")) } }
        (list(&other))
        @for s in &rows { (suggestion(&r, s, &form.0)) }
    };
    page(&r, "/suggestions", &r.id("web-nav-suggestions"), &body)
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
    let decides = s.decide && !s.mine;
    html! {
        article {
            p { (r.catalogue().format(&msg)) }
            @if decides && !s.force {
                form method="post" action="/suggestions/accept" {
                    input type="hidden" name="form" value=(form);
                    input type="hidden" name="id" value=(s.id);
                    button type="submit" { (r.id("web-accept")) }
                }
            }
            @if decides {
                form method="post" action="/suggestions/decline" {
                    input type="hidden" name="form" value=(form);
                    input type="hidden" name="id" value=(s.id);
                    label { (r.id("web-decline-note")) " " input type="text" name="note"; }
                    " "
                    button type="submit" { (r.id("web-decline")) }
                }
            }
        }
    }
}

/// The result of an action: its first line as the title, the rest under
/// it, and the way back to `here`.
pub fn result(r: &Renderer, here: &str, mut lines: Vec<String>) -> Html<String> {
    let rest = if lines.is_empty() {
        Vec::new()
    } else {
        lines.split_off(1)
    };
    let title = lines.pop().unwrap_or_default();
    let body = html! {
        (list(&rest))
        p { a href=(here) { (r.id("web-back")) } }
    };
    page(r, here, &title, &body)
}

/// The headline every 5 seconds, as `headline` events.
pub async fn events(
    State(web): State<Arc<Web>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let s = stream::unfold((web, true), |(web, first)| async move {
        if !first {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        let r = web.renderer();
        let (headline, _) = status(&web, &r).await;
        Some((
            Ok(Event::default().event("headline").data(headline)),
            (web, false),
        ))
    });
    Sse::new(s).keep_alive(KeepAlive::default())
}
