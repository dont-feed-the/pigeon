//! People & machines (ux.md §6.4): the group's computers and backup
//! machines, each but this one with *Remove* (`pigeon remove-computer`,
//! asked again before it is done), and the line that adds someone to the
//! group (`pigeon invite`), with *Copy*. People show as cards, machines
//! with an icon of their kind.

use std::sync::Arc;

use crate::Renderer;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Form};
use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_proto::Request;
use pigeon_proto::api::Machine;

use crate::icons::icon;
use crate::pages::{list, page};
use crate::{FormToken, Web, pages};

/// The People & machines page.
pub async fn machines(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let mut rows = Vec::new();
    let mut other = Vec::new();
    for answer in web.ask(Request::Machines).await {
        match answer {
            pigeon_proto::Response::Machines { machines } => rows = machines,
            pigeon_proto::Response::Progress { .. } => {}
            a => other.extend(r.response(&a).0),
        }
    }
    // The people first, each with their machines (ux.md §6.4).
    let entries: Vec<pigeon_proto::api::PersonEntry> = web
        .ask(Request::People)
        .await
        .iter()
        .flat_map(|a| match a {
            pigeon_proto::Response::People { people } => people.clone(),
            _ => Vec::new(),
        })
        .collect();
    let can_print = entries.iter().any(|p| p.this_person && p.admin);
    let invitation = web
        .ask(Request::Invite)
        .await
        .into_iter()
        .find_map(|a| match a {
            pigeon_proto::Response::Said { message } if message.id == "invitation" => Some(message),
            _ => None,
        });
    let f = &form.0;
    let body = html! {
        (list(&other))
        @if !entries.is_empty() {
            h2 { (r.id("web-people")) }
            div class="cards" {
                @for p in &entries {
                    article class="card person" {
                        span class="avatar" aria-hidden="true" { (crate::pages::initial(&p.name)) }
                        div class="grow" {
                            p { (r.person(p)) }
                        }
                    }
                }
            }
        }
        @if can_print {
            form method="post" action=(r.url("/machines/recovery-sheet")) class="toolbar" {
                input type="hidden" name="form" value=(f);
                button type="submit" { (icon("key", "")) (r.id("web-print-recovery")) }
            }
        }
        @if !rows.is_empty() {
            h2 { (r.id("web-machines")) }
            ul class="plain-list card" { @for m in &rows { (machine(&r, m, f)) } }
        }
        @if let Some(message) = &invitation { (invite(&r, message)) }
    };
    page(&r, "/machines", &r.id("web-nav-machines"), &body)
}

/// One machine, and the form that removes it.
fn machine(r: &Renderer, m: &Machine, form: &str) -> Markup {
    let symbol = if m.recovery {
        "key"
    } else if m.custodian {
        "server"
    } else {
        "laptop"
    };
    html! {
        li class="machine" {
            (icon(symbol, ""))
            span class="grow" { (r.machine(m)) }
            @if !m.this_device {
                form method="post" action=(r.url("/machines/remove")) {
                    input type="hidden" name="form" value=(form);
                    input type="hidden" name="name" value=(m.name);
                    button type="submit" class="small danger" { (r.id(if m.recovery { "web-revoke-sheet" } else { "web-remove" })) }
                }
            }
        }
    }
}

/// The line that adds someone to the group, of the `invitation` message,
/// with what it does and a *Copy* button, which the client script shows.
fn invite(r: &Renderer, message: &pigeon_proto::Message) -> Markup {
    let arg = |key: &str| match message.args.get(key) {
        Some(pigeon_proto::Arg::Text(t)) => t.clone(),
        _ => String::new(),
    };
    let help = Message::new("web-invite-help").arg("group", arg("group"));
    html! {
        section class="card stack" aria-labelledby="invite-title" {
            h2 id="invite-title" { (icon("plus", "")) " " (r.id("web-invite")) }
            p { (r.catalogue().format(&help)) }
            p { label for="invitation-line" { (r.id("web-invitation-code")) } }
            textarea id="invitation-line" rows="4" cols="60" readonly { (arg("command")) }
            p { button type="button" class="primary" data-copy="invitation-line" data-copied=(r.id("web-copied")) hidden { (r.id("web-copy")) } }
        }
    }
}

/// The form that removes a machine; `yes` once the person confirmed.
#[derive(serde::Deserialize)]
pub struct RemoveForm {
    /// The session's form token.
    form: String,
    /// The machine's name in the roster.
    name: String,
    /// Asked again and confirmed.
    #[serde(default)]
    yes: bool,
}

/// *Remove*: asks again, then `pigeon remove-computer NAME`.
pub async fn remove(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<RemoveForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    if !f.yes {
        let recovery = web.ask(Request::Machines).await.iter().any(|answer| {
            matches!(answer, pigeon_proto::Response::Machines { machines }
                if machines.iter().any(|m| m.name == f.name && m.recovery))
        });
        let question = r.catalogue().format(
            &Message::new(if recovery {
                "web-revoke-confirm"
            } else {
                "web-remove-confirm"
            })
            .arg("name", f.name.as_str()),
        );
        let body = html! {
            div class="card inline-forms" {
                form method="post" action=(r.url("/machines/remove")) {
                    input type="hidden" name="form" value=(f.form);
                    input type="hidden" name="name" value=(f.name);
                    input type="hidden" name="yes" value="true";
                    button type="submit" class="danger" { (r.id(if recovery { "web-revoke-sheet" } else { "web-remove" })) }
                }
                a class="btn" href=(r.url("/machines")) { (r.id("web-back")) }
            }
        };
        return page(&r, "/machines", &question, &body).into_response();
    }
    let lines = pages::lines(&web, &r, Request::RemoveDevice { name: f.name }).await;
    pages::result(&r, "/machines", lines).into_response()
}

/// The refusal of a form without the session's token.
pub(crate) fn expired(web: &Web, token: &FormToken, form: &str) -> Option<Response> {
    (token.0.is_empty() || form != token.0).then(|| {
        (
            StatusCode::FORBIDDEN,
            pages::refused(web, "web-form-expired"),
        )
            .into_response()
    })
}
