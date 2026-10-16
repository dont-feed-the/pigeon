//! The pages (10 §10.3, ux.md §6): each opens with its one-line answer,
//! under the five sections' navigation. Every word comes from the
//! catalogue (ADR 0028). Minimal event handlers update Home's headline
//! and health facts through text nodes; action results remain static.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Extension;
use axum::extract::State;
use axum::response::Html;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::StreamExt;
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
    ("/settings", "web-nav-settings"),
    ("/join", "web-setup"),
];

const STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:46rem;margin:2rem auto;\
padding:0 1rem;line-height:1.5;color:#1b1b1b}nav a{margin-right:1.2rem}\
nav a[aria-current]{font-weight:bold}h1{font-size:1.5rem}li{margin:.3rem 0}\
article{border-top:1px solid #ccc;padding:.5rem 0}form{display:inline-block;margin-right:1rem}\
textarea{max-width:100%;box-sizing:border-box}\
.recovery-instructions{overflow-wrap:anywhere}.recovery-instructions p{margin:.25rem 0}\
.recovery-code{overflow-wrap:anywhere;white-space:pre-wrap;font-size:.7rem}\
.recovery-qr svg{width:100%;max-width:32rem;height:auto}\
@media print{nav,.no-print{display:none}body{margin:0;max-width:none}.recovery-qr svg{max-width:12cm}}";

/// Puts each `headline` event's text in the page's headline.
const SCRIPT: &str = "const s=new EventSource('/events');\
s.addEventListener('headline',e=>{const h=document.getElementById('headline');\
if(h.textContent!==e.data)h.textContent=e.data});\
s.addEventListener('health',e=>{const p=JSON.parse(e.data),h=document.getElementById('health');\
if(!h||h.dataset.membership!==p.membership){location.reload();return}\
for(const [id,value] of Object.entries(p.values)){const n=document.getElementById(id);\
if(n&&n.textContent!==value)n.textContent=value}\
const l=document.getElementById('health-explanations');\
if(l&&JSON.stringify([...l.children].map(n=>n.textContent))!==JSON.stringify(p.explanations)){\
l.replaceChildren(...p.explanations.map(text=>{const n=document.createElement('li');n.textContent=text;return n}))}})";

pub(crate) fn page(r: &Renderer, here: &str, headline: &str, body: &Markup) -> Html<String> {
    document(r, here, headline, body, here == "/")
}

fn document(r: &Renderer, here: &str, headline: &str, body: &Markup, live: bool) -> Html<String> {
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
                        h1 id=(if live { "headline" } else { "title" }) { (headline) }
                        (body)
                    }
                    @if live { script { (maud::PreEscaped(SCRIPT)) } }
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
async fn status(
    web: &Web,
    r: &Renderer,
) -> (
    String,
    Vec<String>,
    bool,
    Option<Box<pigeon_proto::health::HealthReport>>,
) {
    let answers = web.ask(Request::Health).await;
    let health = answers.iter().find_map(|answer| match answer {
        Response::Health(report) => Some(report.clone()),
        _ => None,
    });
    let no_group = answers
        .iter()
        .any(|a| matches!(a, Response::Problem(p) if p.code == "not-in-group"));
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
    (all.pop().unwrap_or_default(), rest, no_group, health)
}

pub(crate) fn list(items: &[String]) -> Markup {
    html! { ul { @for i in items { li { (i.trim()) } } } }
}

/// Home: the headline, what is not safe yet and why, and the deletions
/// pigeon holds (`held.rs`).
pub async fn home(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let (headline, rest, no_group, health) = status(&web, &r).await;
    let held = if no_group {
        html! {}
    } else {
        crate::held::section(&web, &r, &form.0).await
    };
    let body = html! {
        (held)
        @if let Some(health) = &health { (crate::health::section(&r, health)) }
        ul id="health-explanations" { @for line in &rest { li { (line.trim()) } } }
        @if no_group {
            p { (r.id("web-setup-help")) }
            p { a href="/join" { (r.id("web-setup")) } }
        }
    };
    page(&r, "/", &headline, &body)
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
    // An action result must keep its own title. Home's live status would
    // replace a join refusal with "not in a group" after five seconds.
    document(r, here, &title, &body, false)
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
        let (headline, _, _, health) = status(&web, &r).await;
        let mut events: Vec<Result<Event, Infallible>> =
            vec![Ok(Event::default().event("headline").data(headline))];
        if let Some(health) = health {
            events.push(Ok(Event::default()
                .event("health")
                .data(crate::health::event(&r, &health))));
        }
        Some((events, (web, false)))
    })
    .flat_map(stream::iter);
    Sse::new(s).keep_alive(KeepAlive::default())
}
