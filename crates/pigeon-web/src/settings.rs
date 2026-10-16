//! Settings (ux.md §6.5): this computer's settings of `config.toml` (11
//! §11.2), each a choice and a *Save* button that becomes `SetSettings`.
//! A new language shows from the answer on. *Pause* and *Share again* are
//! `pigeon pause` and `pigeon resume` (15 §15.6); *Leave the group* is
//! `pigeon leave` (`leave.rs`).

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Form};
use maud::{Markup, html};
use pigeon_i18n::render::Renderer;
use pigeon_proto::Request;
use pigeon_proto::api::Setting;

use crate::pages::{list, page};
use crate::{FormToken, Web, pages};

/// The Settings page.
pub async fn settings(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let mut rows = Vec::new();
    let mut other = Vec::new();
    for answer in web.ask(Request::GetSettings { key: None }).await {
        match answer {
            pigeon_proto::Response::Settings { settings } => rows = settings,
            pigeon_proto::Response::Progress { .. } => {}
            a => other.extend(r.response(&a).0),
        }
    }
    let mut peers = Vec::new();
    for answer in web.ask(Request::Machines).await {
        if let pigeon_proto::Response::Machines { machines } = answer {
            peers = machines
                .into_iter()
                .filter(|m| !m.this_device && !m.recovery)
                .collect();
        }
    }
    let body = html! {
        (list(&other))
        @for s in &rows { (setting(&r, s, &form.0)) }
        p { (r.id("status-freshness")) }
        (pausing(&r, &form.0))
        (crate::leave::form(&r, &form.0))
        h2 { (r.id("web-diagnostics")) }
        p { a href="/doctor" { (r.id("web-doctor")) } }
        p { (r.id("web-diagnostics-consent")) }
        form method="post" action="/settings/diagnostics" {
            input type="hidden" name="form" value=(&form.0);
            label for="diagnostic-peer" { (r.id("web-diagnostic-peer")) }
            select id="diagnostic-peer" name="peer" {
                @for peer in &peers { option value=(&peer.device) { (&peer.name) } }
            }
            label {
                input type="checkbox" name="consent" value="true" required;
                (r.id("web-diagnostics-consent"))
            }
            button type="submit" { (r.id("web-allow-diagnostics")) }
        }
    };
    page(&r, "/settings", &r.id("web-nav-settings"), &body)
}

/// Installation and group diagnostics use the same read-only request as CLI.
pub async fn doctor(State(web): State<Arc<Web>>) -> Html<String> {
    let r = web.renderer();
    let answers = web.ask(Request::Doctor { details: false }).await;
    pages::result(
        &r,
        "/settings",
        answers
            .iter()
            .flat_map(|answer| r.response(answer).0)
            .collect(),
    )
}

/// What a value is called: a language by its own name, the others from
/// the catalogue.
fn label(r: &Renderer, key: &str, value: &str) -> String {
    match (key, value) {
        (_, "auto") => r.id("setting-auto"),
        ("telemetry-device", "off") => r.id("setting-off"),
        ("telemetry-device", name) => name.to_owned(),
        ("language", lang) => r.id(&format!("language-{lang}")),
        (_, v) => r.id(&format!("setting-{v}")),
    }
}

/// One setting, and the form that changes it.
fn setting(r: &Renderer, s: &Setting, form: &str) -> Markup {
    let id = format!("setting-{}", s.key);
    html! {
        article {
            form method="post" action="/settings" {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="key" value=(s.key);
                label for=(id) { (r.id(&id)) " " }
                select id=(id) name="value" {
                    @for c in &s.choices {
                        option value=(c) selected[*c == s.value] { (label(r, &s.key, c)) }
                    }
                }
                " "
                button type="submit" { (r.id("web-save")) }
            }
        }
    }
}

/// The pause: a duration and *Pause*, and *Share again*.
fn pausing(r: &Renderer, form: &str) -> Markup {
    html! {
        article {
            form method="post" action="/pause" {
                input type="hidden" name="form" value=(form);
                label for="pause-for" { (r.id("web-pause")) " " }
                select id="pause-for" name="duration" {
                    option value="1h" { (r.id("pause-hour")) }
                    option value="1d" { (r.id("pause-day")) }
                    option value="" { (r.id("pause-forever")) }
                }
                " "
                button type="submit" { (r.id("web-pause")) }
            }
            form method="post" action="/resume" {
                input type="hidden" name="form" value=(form);
                button type="submit" { (r.id("web-resume")) }
            }
        }
    }
}

/// The form of *Pause*.
#[derive(serde::Deserialize)]
pub struct PauseForm {
    /// The session's form token.
    form: String,
    /// A duration, or empty until *Share again*.
    #[serde(default)]
    duration: String,
}

/// The form of *Share again*.
#[derive(serde::Deserialize)]
pub struct ResumeForm {
    /// The session's form token.
    form: String,
}

/// *Pause*: `pigeon pause [DURATION]`.
pub async fn pause(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<PauseForm>,
) -> Response {
    let duration = (!f.duration.is_empty()).then_some(f.duration);
    answer(&web, &token, &f.form, Request::Pause { duration }).await
}

/// *Share again*: `pigeon resume`.
pub async fn resume(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<ResumeForm>,
) -> Response {
    answer(&web, &token, &f.form, Request::Resume).await
}

/// Sends `request` if the form token holds, and shows the answer.
async fn answer(web: &Arc<Web>, token: &FormToken, form: &str, request: Request) -> Response {
    if token.0.is_empty() || form != token.0 {
        return (
            StatusCode::FORBIDDEN,
            pages::refused(web, "web-form-expired"),
        )
            .into_response();
    }
    let answers = web.ask(request).await;
    let r = web.renderer();
    let lines = answers.iter().flat_map(|a| r.response(a).0).collect();
    pages::result(&r, "/settings", lines).into_response()
}

/// The form of one setting.
#[derive(serde::Deserialize)]
pub struct SettingForm {
    /// The session's form token.
    form: String,
    /// The setting's key.
    key: String,
    /// Its new value.
    value: String,
}

/// *Save*: `pigeon settings KEY VALUE`.
pub async fn save(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<SettingForm>,
) -> Response {
    if token.0.is_empty() || f.form != token.0 {
        return (
            StatusCode::FORBIDDEN,
            pages::refused(&web, "web-form-expired"),
        )
            .into_response();
    }
    let answers = web
        .ask(Request::SetSettings {
            key: f.key.clone(),
            value: f.value.clone(),
        })
        .await;
    // After the answer, so that a new language shows at once.
    let r = web.renderer();
    let lines = answers.iter().flat_map(|a| {
        if matches!(a, pigeon_proto::Response::Said { message } if message.id == "setting-changed") {
            let message = pigeon_i18n::Message::new("web-setting-saved")
                .arg("key", r.id(&format!("setting-{}", f.key)))
                .arg("value", label(&r, &f.key, &f.value));
            vec![r.catalogue().format(&message)]
        } else {
            r.response(a).0
        }
    }).collect();
    pages::result(&r, "/settings", lines).into_response()
}

/// An explicit local permission to disclose one group diagnostic part.
#[derive(serde::Deserialize)]
pub struct DiagnosticConsent {
    form: String,
    peer: String,
    #[serde(default)]
    consent: bool,
}

/// No checkbox, stale form or forged token can grant diagnostic access.
pub async fn diagnostics(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<DiagnosticConsent>,
) -> Response {
    if !f.consent {
        return StatusCode::BAD_REQUEST.into_response();
    }
    answer(&web, &token, &f.form, Request::AllowDebug { peer: f.peer }).await
}
