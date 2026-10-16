//! People & machines (ux.md §6.4): the group's computers and backup
//! machines, each but this one with *Remove* (`pigeon remove-computer`,
//! asked again before it is done), and the forms that invite a person or
//! a backup machine (`pigeon invite`), whose answer is the invitation,
//! and the pending invitations, each with *Cancel* (`pigeon invite
//! --cancel ID`).

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Form};
use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_i18n::render::Renderer;
use pigeon_proto::Request;
use pigeon_proto::api::{InvitationEntry, Machine};

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
    let people: Vec<String> = web
        .ask(Request::People)
        .await
        .iter()
        .flat_map(|a| match a {
            pigeon_proto::Response::People { people } => {
                people.iter().map(|p| r.person(p)).collect()
            }
            _ => Vec::new(),
        })
        .collect();
    let mut invitations = Vec::new();
    for answer in web.ask(Request::Invitations).await {
        if let pigeon_proto::Response::Invitations { invitations: i } = answer {
            invitations = i;
        }
    }
    let f = &form.0;
    let body = html! {
        (list(&other))
        (list(&people))
        @for m in &rows { (machine(&r, m, f)) }
        h2 { (r.id("web-invite")) }
        form method="post" action="/machines/invite" {
            input type="hidden" name="form" value=(f);
            label for="person" { (r.id("web-invite-person")) " " }
            input id="person" name="person" required;
            " "
            label { input type="checkbox" name="admin" value="true"; " " (r.id("web-invite-admin")) }
            " "
            button type="submit" { (r.id("web-invite")) }
        }
        form method="post" action="/machines/invite" {
            input type="hidden" name="form" value=(f);
            input type="hidden" name="backup" value="true";
            button type="submit" { (r.id("web-invite-backup")) }
        }
        @if !invitations.is_empty() {
            h2 { (r.id("web-invitations")) }
            @for i in &invitations { (invitation(&r, i, f)) }
        }
    };
    page(&r, "/machines", &r.id("web-nav-machines"), &body)
}

/// One machine, and the form that removes it.
fn machine(r: &Renderer, m: &Machine, form: &str) -> Markup {
    html! {
        article {
            p { (r.machine(m)) }
            @if !m.this_device {
                form method="post" action="/machines/remove" {
                    input type="hidden" name="form" value=(form);
                    input type="hidden" name="name" value=(m.name);
                    button type="submit" { (r.id(if m.recovery { "web-revoke-sheet" } else { "web-remove" })) }
                }
            }
        }
    }
}

/// One pending invitation, and the form that cancels it.
fn invitation(r: &Renderer, i: &InvitationEntry, form: &str) -> Markup {
    html! {
        article {
            p { (r.invitation(i)) }
            form method="post" action="/machines/cancel-invitation" {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="invitation" value=(i.id);
                button type="submit" { (r.id("web-cancel-invitation")) }
            }
        }
    }
}

/// The form that cancels a pending invitation.
#[derive(serde::Deserialize)]
pub struct CancelForm {
    /// The session's form token.
    form: String,
    /// The invitation's ID.
    invitation: String,
}

/// *Cancel*: `pigeon invite --cancel ID`.
pub async fn cancel(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<CancelForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    let request = Request::CancelInvitation {
        invitation: f.invitation,
    };
    let lines = pages::lines(&web, &r, request).await;
    pages::result(&r, "/machines", lines).into_response()
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
            form method="post" action="/machines/remove" {
                input type="hidden" name="form" value=(f.form);
                input type="hidden" name="name" value=(f.name);
                input type="hidden" name="yes" value="true";
                button type="submit" { (r.id(if recovery { "web-revoke-sheet" } else { "web-remove" })) }
            }
            p { a href="/machines" { (r.id("web-back")) } }
        };
        return page(&r, "/machines", &question, &body).into_response();
    }
    let lines = pages::lines(&web, &r, Request::RemoveDevice { name: f.name }).await;
    pages::result(&r, "/machines", lines).into_response()
}

/// The form that invites a person, or a backup machine with `backup`.
#[derive(serde::Deserialize)]
pub struct InviteForm {
    /// The session's form token.
    form: String,
    /// The person's name.
    #[serde(default)]
    person: String,
    /// As an admin.
    #[serde(default)]
    admin: bool,
    /// A backup machine.
    #[serde(default)]
    backup: bool,
}

/// *Invite*: `pigeon invite [--admin] PERSON` or `--backup-machine`.
pub async fn invite(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<InviteForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let request = Request::Invite {
        person: if f.backup {
            String::new()
        } else {
            f.person.trim().to_owned()
        },
        admin: f.admin && !f.backup,
        custodian: f.backup,
        name: None,
    };
    let r = web.renderer();
    let answers = web.ask(request).await;
    let code = answers.iter().find_map(|a| match a {
        pigeon_proto::Response::Said { message } if message.id == "invitation" => {
            match message.args.get("code") {
                Some(pigeon_proto::Arg::Text(code)) => Some(code),
                _ => None,
            }
        }
        _ => None,
    });
    if let Some(code) = code {
        let lines: Vec<String> = answers.iter().flat_map(|a| r.response(a).0).collect();
        let body = html! {
            p { (r.id("web-invite-handoff")) }
            p { label for="invitation-code" { (r.id("web-invitation-code")) } }
            textarea id="invitation-code" rows="6" cols="60" readonly { (code) }
            details { summary { (r.id("web-command-line")) } (list(&lines)) }
            p { a href="/machines" { (r.id("web-back")) } }
        };
        return page(&r, "/machines", &r.id("web-invitation-ready"), &body).into_response();
    }
    let lines = answers.iter().flat_map(|a| r.response(a).0).collect();
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
