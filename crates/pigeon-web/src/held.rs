//! Held changes on Home (07 §7.4, ux.md §6.1): each batch pigeon holds,
//! with *Delete for everyone*, asked again first, and *Bring them back*,
//! which are `pigeon held ID --share` and `--bring-back`; a held name has
//! *Rename to …* instead, `pigeon held ID --rename` (05 §5.4).

use std::sync::Arc;

use crate::Renderer;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Form};
use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_proto::Request;
use pigeon_proto::api::{HeldChoice, HeldEntry};

use crate::machines::expired;
use crate::pages::{self, page};
use crate::{FormToken, Web};

/// The held batches, each with its two forms; nothing when none is held.
pub async fn section(web: &Web, r: &Renderer, form: &str) -> Markup {
    let mut held = Vec::new();
    for answer in web.ask(Request::Held).await {
        if let pigeon_proto::Response::Held { held: h } = answer {
            held = h;
        }
    }
    html! {
        @if !held.is_empty() {
            h2 { (r.id("web-held")) }
            div class="stack" { @for h in &held { (batch(r, h, form)) } }
        }
    }
}

fn batch(r: &Renderer, h: &HeldEntry, form: &str) -> Markup {
    if h.reason.id.starts_with("held-name") {
        return html! {
            article class="card stack" {
                p { (r.held(h)) }
                @if let Some(s) = &h.suggested {
                    form method="post" action=(r.url("/held/rename")) {
                        input type="hidden" name="form" value=(form);
                        input type="hidden" name="batch" value=(h.id);
                        button type="submit" {
                            (r.catalogue().format(&Message::new("web-held-rename").arg("suggested", s.as_str())))
                        }
                    }
                }
            }
        };
    }
    html! {
        article class="card stack" {
            p { (r.held(h)) }
            div class="inline-forms" {
            form method="post" action=(r.url("/held/share")) {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="batch" value=(h.id);
                button type="submit" { (r.id("web-held-share")) }
            }
            form method="post" action=(r.url("/held/bring-back")) {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="batch" value=(h.id);
                button type="submit" class="primary" { (r.id("web-held-bring-back")) }
            }
            }
        }
    }
}

/// The form of a held batch; `yes` once the person confirmed.
#[derive(serde::Deserialize)]
pub struct HeldForm {
    /// The session's form token.
    form: String,
    /// The batch's ID.
    batch: String,
    /// Asked again and confirmed.
    #[serde(default)]
    yes: bool,
}

/// *Delete for everyone*: asks again, then `pigeon held ID --share`.
pub async fn share(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<HeldForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    if !f.yes {
        let question = r
            .catalogue()
            .format(&Message::new("web-held-share-confirm").arg("batch", f.batch.as_str()));
        let body = html! {
            div class="card inline-forms" {
                form method="post" action=(r.url("/held/share")) {
                    input type="hidden" name="form" value=(f.form);
                    input type="hidden" name="batch" value=(f.batch);
                    input type="hidden" name="yes" value="true";
                    button type="submit" class="danger" { (r.id("web-held-share")) }
                }
                a class="btn" href=(r.url("/")) { (r.id("web-back")) }
            }
        };
        return page(&r, "/", &question, &body).into_response();
    }
    release(&web, f.batch, HeldChoice::Share).await
}

/// *Bring them back*: `pigeon held ID --bring-back`.
pub async fn bring_back(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<HeldForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    release(&web, f.batch, HeldChoice::BringBack).await
}

/// *Rename to …*: `pigeon held ID --rename`.
pub async fn rename(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<HeldForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    release(&web, f.batch, HeldChoice::Rename).await
}

async fn release(web: &Web, batch: String, choice: HeldChoice) -> Response {
    let r = web.renderer();
    let lines = pages::lines(web, &r, Request::ReleaseHeld { batch, choice }).await;
    pages::result(&r, "/", lines).into_response()
}
