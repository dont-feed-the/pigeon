//! The pages (10 §10.3, ux.md §6): each opens with its one-line answer,
//! under the five sections' navigation. Every word comes from the
//! catalogue (ADR 0028); the pages hold no script but the one line that
//! puts the headline the events bring in place.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::Html;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{self, Stream};
use maud::{DOCTYPE, Markup, html};
use pigeon_i18n::render::Renderer;
use pigeon_proto::{Request, Response};

use crate::Web;

/// The sections, as (path, message id); Files and Settings come later.
const SECTIONS: &[(&str, &str)] = &[
    ("/", "web-nav-home"),
    ("/suggestions", "web-nav-suggestions"),
    ("/machines", "web-nav-machines"),
];

const STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:46rem;margin:2rem auto;\
padding:0 1rem;line-height:1.5;color:#1b1b1b}nav a{margin-right:1.2rem}\
nav a[aria-current]{font-weight:bold}h1{font-size:1.5rem}li{margin:.3rem 0}";

/// Puts each `headline` event's text in the page's headline.
const SCRIPT: &str = "new EventSource('/events').addEventListener('headline',\
e=>{document.getElementById('headline').textContent=e.data})";

fn page(r: &Renderer, here: &str, headline: &str, body: &Markup) -> Html<String> {
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
async fn lines(web: &Web, r: &Renderer, request: Request) -> Vec<String> {
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

fn list(items: &[String]) -> Markup {
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

/// Suggestions: those waiting for a decision.
pub async fn suggestions(State(web): State<Arc<Web>>) -> Html<String> {
    let r = web.renderer();
    let rows = lines(&web, &r, Request::Proposals).await;
    page(
        &r,
        "/suggestions",
        &r.id("web-nav-suggestions"),
        &list(&rows),
    )
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
