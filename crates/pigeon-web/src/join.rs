//! First use on a new computer (spec 09 §9.6, 10 §10.3): join by invitation
//! or create a group. Refounded groups require a second explicit confirmation.
//! Every action uses the same session and form checks as the other pages.

use std::sync::Arc;

use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Form};
use maud::html;
use pigeon_proto::Request;

use crate::machines::expired;
use crate::pages::{self, page};
use crate::{FormToken, Web};

/// The join and create forms on a new computer.
pub async fn setup(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let body = html! {
        p { (r.id("web-setup-help")) }
        h2 { (r.id("web-join")) }
        form method="post" action=(r.url("/join")) {
            input type="hidden" name="form" value=(&token.0);
            p { label for="invitation" { (r.id("web-invitation")) } " " input id="invitation" name="invitation" required autocomplete="off"; }
            p { label for="join-folder" { (r.id("web-local-folder")) } " " input id="join-folder" name="folder"; }
            p { label for="join-name" { (r.id("web-computer-name")) } " " input id="join-name" name="name"; }
            button type="submit" { (r.id("web-join")) }
        }
        h2 { (r.id("web-create")) }
        form method="post" action=(r.url("/create")) {
            input type="hidden" name="form" value=(&token.0);
            p { label for="group-name" { (r.id("web-group-name")) } " " input id="group-name" name="group" required; }
            p { label for="create-folder" { (r.id("web-local-folder")) } " " input id="create-folder" name="folder" required; }
            p { label for="create-name" { (r.id("web-computer-name")) } " " input id="create-name" name="name" required; }
            p { label for="copies" { (r.id("web-backup-copies")) } " " input id="copies" type="number" name="k" value="2" min="1" required; }
            button type="submit" { (r.id("web-create")) }
        }
    };
    page(&r, "/join", &r.id("web-setup"), &body)
}

/// A join request; the folder is absent on a backup machine.
#[derive(serde::Deserialize)]
pub struct JoinForm {
    form: String,
    invitation: String,
    #[serde(default)]
    folder: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    yes: bool,
}

/// Join, asking again before joining a refounded history.
pub async fn join(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<JoinForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    let invitation = f.invitation.trim();
    if !f.yes
        && let Some((group, refounded)) = pigeon_proto::join::refounded(invitation)
    {
        let message = pigeon_i18n::Message::new("web-join-refounded")
            .arg("group", group)
            .arg("name", refounded.by)
            .arg("date", refounded.at.get(..10).unwrap_or(&refounded.at));
        let body = html! {
            p { (r.catalogue().format(&message)) }
            form method="post" action=(r.url("/join")) {
                input type="hidden" name="form" value=(&f.form);
                input type="hidden" name="invitation" value=(invitation);
                input type="hidden" name="folder" value=(&f.folder);
                input type="hidden" name="name" value=(&f.name);
                input type="hidden" name="yes" value="true";
                button type="submit" { (r.id("web-join")) }
            }
            p { a href=(r.url("/join")) { (r.id("web-back")) } }
        };
        return page(&r, "/join", &r.id("web-join"), &body).into_response();
    }
    let request = Request::Join {
        invitation: invitation.to_owned(),
        folder: nonempty(&f.folder),
        name: nonempty(&f.name),
        keep: true,
    };
    pages::result(&r, "/", pages::lines(&web, &r, request).await).into_response()
}

fn nonempty(s: &str) -> Option<String> {
    (!s.trim().is_empty()).then(|| s.trim().to_owned())
}

/// A new group's name, folder and requested safe copies.
#[derive(serde::Deserialize)]
pub struct CreateForm {
    form: String,
    group: String,
    folder: String,
    name: String,
    k: u32,
}

/// Create a group through the daemon's usual validation.
pub async fn create(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<CreateForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    let request = Request::Create {
        name: f.group,
        folder: f.folder,
        k: f.k,
        device_name: f.name,
    };
    pages::result(&r, "/", pages::lines(&web, &r, request).await).into_response()
}
