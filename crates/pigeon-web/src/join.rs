//! First use on a new computer (spec 09 §9.6, 10 §10.3): join by the group's
//! invitation or create a group. Joining a refounded group, or as a person
//! the group has, asks a second, explicit confirmation.
//! Every action uses the same session and form checks as the other pages.
//! A refusal shows the forms again, under it, with what was typed.

use std::sync::Arc;

use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Form};
use maud::{Markup, html};
use pigeon_proto::{Request, Response as Answer};

use crate::machines::expired;
use crate::pages::{self, page};
use crate::{FormToken, Renderer, Web};

/// The join and create forms on a new computer.
pub async fn setup(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let body = forms(&r, &token.0, &JoinForm::default(), &CreateForm::default());
    page(&r, "/join", &r.id("web-setup"), &body)
}

/// Both forms, holding `join`'s and `create`'s values.
fn forms(r: &Renderer, token: &str, join: &JoinForm, create: &CreateForm) -> Markup {
    let k = if create.k.is_empty() { "2" } else { &create.k };
    html! {
        p { (r.id("web-setup-help")) }
        h2 { (r.id("web-join")) }
        form method="post" action=(r.url("/join")) {
            input type="hidden" name="form" value=(token);
            p { label for="invitation" { (r.id("web-invitation")) } " " input id="invitation" name="invitation" required autocomplete="off" value=(&join.invitation); }
            p { label for="join-person" { (r.id("web-your-name")) } " " input id="join-person" name="person" value=(&join.person); }
            p { label for="join-folder" { (r.id("web-local-folder")) } " " input id="join-folder" name="folder" value=(&join.folder); }
            p { label for="join-name" { (r.id("web-computer-name")) } " " input id="join-name" name="name" value=(&join.name); }
            button type="submit" { (r.id("web-join")) }
        }
        h2 { (r.id("web-create")) }
        form method="post" action=(r.url("/create")) {
            input type="hidden" name="form" value=(token);
            p { label for="group-name" { (r.id("web-group-name")) } " " input id="group-name" name="group" required value=(&create.group); }
            p { label for="create-folder" { (r.id("web-local-folder")) } " " input id="create-folder" name="folder" required value=(&create.folder); }
            p { label for="create-name" { (r.id("web-computer-name")) } " " input id="create-name" name="name" required value=(&create.name); }
            p { label for="create-person" { (r.id("web-your-name")) } " " input id="create-person" name="person" autocomplete="name" value=(&create.person); }
            p { label for="copies" { (r.id("web-backup-copies")) } " " input id="copies" type="number" name="k" value=(k) min="1" required; }
            button type="submit" { (r.id("web-create")) }
        }
    }
}

/// The answer to a join or create: its result, or, for a problem, the
/// problem over the forms again with what was typed, so nothing is lost.
async fn outcome(
    web: &Web,
    token: &str,
    answers: Vec<Answer>,
    join: &JoinForm,
    create: &CreateForm,
) -> Response {
    let r = web.renderer();
    let lines: Vec<String> = answers
        .iter()
        .filter(|a| !matches!(a, Answer::Progress { .. }))
        .flat_map(|a| r.response(a).0)
        .collect();
    if !answers.iter().any(|a| matches!(a, Answer::Problem(_))) {
        return pages::result(&r, "/", lines).into_response();
    }
    let (title, rest) = lines
        .split_first()
        .map_or((String::new(), &[][..]), |(t, rest)| (t.clone(), rest));
    let body = html! {
        (pages::list(rest))
        (forms(&r, token, join, create))
    };
    page(&r, "/join", &title, &body).into_response()
}

/// `~` or `~/…` as the person's home folder, which the CLI's shell would
/// have expanded; anything else as typed, trimmed.
fn folder(typed: &str) -> String {
    let typed = typed.trim();
    let home = std::env::var("HOME").unwrap_or_default();
    match typed.strip_prefix('~') {
        Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
            format!("{}{rest}", home.trim_end_matches('/'))
        }
        _ => typed.to_owned(),
    }
}

/// A join request; the folder is absent on a backup machine.
#[derive(Default, serde::Deserialize)]
pub struct JoinForm {
    form: String,
    invitation: String,
    #[serde(default)]
    folder: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    person: String,
    #[serde(default)]
    existing: bool,
    #[serde(default)]
    yes: bool,
}

/// Join, asking again before joining a refounded history, and before
/// joining as a person the group has.
pub async fn join(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<JoinForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    let invitation = pigeon_proto::join::invitation(&f.invitation);
    let invitation = invitation.as_str();
    if !f.yes
        && let Some((group, refounded)) = pigeon_proto::join::refounded(invitation)
    {
        let message = pigeon_i18n::Message::new("web-join-refounded")
            .arg("group", group)
            .arg("name", refounded.by)
            .arg(
                "date",
                pigeon_proto::time::parse_rfc3339(&refounded.at).map_or_else(
                    || refounded.at.get(..10).unwrap_or(&refounded.at).to_owned(),
                    |at| r.date(at),
                ),
            );
        let body = confirm(&r, &message, &f, invitation, true, f.existing);
        return page(&r, "/join", &r.id("web-join"), &body).into_response();
    }
    let request = Request::Join {
        invitation: invitation.to_owned(),
        folder: nonempty(&folder(&f.folder)),
        name: nonempty(&f.name),
        keep: true,
        person: nonempty(&f.person),
        existing: f.existing,
        custodian: false,
    };
    let answers = web.ask(request).await;
    let exists = answers.iter().find_map(|a| match a {
        Answer::Problem(p) if p.code == "person-exists" => p.message.args.get("person").cloned(),
        _ => None,
    });
    if let Some(person) = exists {
        let group = pigeon_proto::join::decode(invitation).map(|i| i.name);
        let message = pigeon_i18n::Message::new("web-join-existing")
            .arg(
                "person",
                match person {
                    pigeon_proto::Arg::Text(t) => t,
                    pigeon_proto::Arg::Number(n) => n.to_string(),
                },
            )
            .arg("group", group.unwrap_or_default());
        let body = confirm(&r, &message, &f, invitation, f.yes, true);
        return page(&r, "/join", &r.id("web-join"), &body).into_response();
    }
    outcome(&web, &token.0, answers, &f, &CreateForm::default()).await
}

/// The question `message`, and the form that joins with what was typed,
/// once refounding is accepted (`yes`) and joining a person of the group
/// is (`existing`).
fn confirm(
    r: &Renderer,
    message: &pigeon_i18n::Message,
    f: &JoinForm,
    invitation: &str,
    yes: bool,
    existing: bool,
) -> Markup {
    html! {
        p { (r.catalogue().format(message)) }
        form method="post" action=(r.url("/join")) {
            input type="hidden" name="form" value=(&f.form);
            input type="hidden" name="invitation" value=(invitation);
            input type="hidden" name="folder" value=(&f.folder);
            input type="hidden" name="name" value=(&f.name);
            input type="hidden" name="person" value=(&f.person);
            @if yes { input type="hidden" name="yes" value="true"; }
            @if existing { input type="hidden" name="existing" value="true"; }
            button type="submit" { (r.id("web-join")) }
        }
        p { a href=(r.url("/join")) { (r.id("web-back")) } }
    }
}

fn nonempty(s: &str) -> Option<String> {
    (!s.trim().is_empty()).then(|| s.trim().to_owned())
}

/// A new group's name, folder and requested safe copies, as typed: the
/// daemon refuses what is wrong in words, not the form's parser.
#[derive(Default, serde::Deserialize)]
pub struct CreateForm {
    form: String,
    #[serde(default)]
    group: String,
    #[serde(default)]
    folder: String,
    #[serde(default)]
    name: String,
    /// The creator's name as the others see it; the daemon chooses one
    /// when it is empty.
    #[serde(default)]
    person: String,
    #[serde(default)]
    k: String,
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
    let request = Request::Create {
        name: f.group.trim().to_owned(),
        folder: folder(&f.folder),
        k: f.k.trim().parse().unwrap_or(0),
        device_name: f.name.trim().to_owned(),
        person: nonempty(&f.person),
    };
    let answers = web.ask(request).await;
    outcome(&web, &token.0, answers, &JoinForm::default(), &f).await
}
