//! *Leave the group* on Settings (09 §9.7, ux.md §6.5): asked again first,
//! then `pigeon leave`; the files stay in the folder.

use std::sync::Arc;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Form};
use maud::{Markup, html};
use pigeon_i18n::render::Renderer;
use pigeon_proto::Request;

use crate::machines::expired;
use crate::pages::{self, page};
use crate::{FormToken, Web};

/// The form of *Leave the group*.
pub fn form(r: &Renderer, form: &str) -> Markup {
    html! {
        article {
            form method="post" action="/leave" {
                input type="hidden" name="form" value=(form);
                button type="submit" { (r.id("web-leave")) }
            }
        }
    }
}

/// The form of *Leave the group*; `yes` once the person confirmed.
#[derive(serde::Deserialize)]
pub struct LeaveForm {
    /// The session's form token.
    form: String,
    /// Asked again and confirmed.
    #[serde(default)]
    yes: bool,
}

/// *Leave the group*: asks again, then `pigeon leave`.
pub async fn leave(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<LeaveForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    if !f.yes {
        let body = html! {
            form method="post" action="/leave" {
                input type="hidden" name="form" value=(f.form);
                input type="hidden" name="yes" value="true";
                button type="submit" { (r.id("web-leave")) }
            }
            p { a href="/settings" { (r.id("web-back")) } }
        };
        return page(&r, "/settings", &r.id("web-leave-confirm"), &body).into_response();
    }
    let lines = pages::lines(&web, &r, Request::Leave).await;
    pages::result(&r, "/settings", lines).into_response()
}
