//! Settings (ux.md §6.5): this computer's settings of `config.toml` (11
//! §11.2), each a choice and a *Save* button that becomes `SetSettings`.
//! A new language shows from the answer on. *Pause* and *Share again* are
//! `pigeon pause` and `pigeon resume` (15 §15.6), only the one that
//! applies; *Leave the group* is
//! `pigeon leave` (`leave.rs`).

use std::sync::Arc;

use crate::Renderer;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Form};
use maud::{Markup, html};
use pigeon_proto::Request;
use pigeon_proto::api::Setting;

use crate::icons::icon;
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
        if let pigeon_proto::Response::Machines { machines, .. } = answer {
            peers = machines
                .into_iter()
                .filter(|m| !m.this_device && !m.recovery)
                .collect();
        }
    }
    let body = html! {
        (list(&other))
        section class="card stack" aria-labelledby="settings-computer" {
            h2 id="settings-computer" { (icon("laptop", "")) " " (r.id("web-settings-computer")) }
            @for s in rows.iter().filter(|s| !s.key.starts_with("resources.")) { (setting(&r, s, &form.0)) }
            p class="lead" { (r.id("status-freshness")) }
        }
        @if rows.iter().any(|s| s.key.starts_with("resources.")) {
            section class="card stack" aria-labelledby="settings-resources" {
                h2 id="settings-resources" { (r.id("web-settings-resources")) }
                p { (r.id("web-settings-resources-help")) }
                @for s in rows.iter().filter(|s| s.key.starts_with("resources.")) { (setting(&r, s, &form.0)) }
            }
        }
        section class="card stack" aria-labelledby="settings-pause" {
            h2 id="settings-pause" { (icon("clock", "")) " " (r.id("web-settings-sharing")) }
            (pausing(&r, &form.0))
        }
        section class="card stack" aria-labelledby="settings-advanced" {
            h2 id="settings-advanced" { (icon("gear", "")) " " (r.id("web-diagnostics")) }
            p class="inline-forms" {
                a class="btn" href=(r.url("/doctor")) { (r.id("web-doctor")) }
                a class="btn" href=(r.url("/space")) { (icon("disk", "")) (r.id("space-title")) }
            }
            form method="post" action=(r.url("/settings/diagnostics")) class="stack" {
                input type="hidden" name="form" value=(&form.0);
                p {
                    label for="diagnostic-peer" { (r.id("web-diagnostic-peer")) }
                    " "
                    select id="diagnostic-peer" name="peer" {
                        @for peer in &peers { option value=(&peer.device) { (&peer.name) } }
                    }
                }
                p {
                    label {
                        input type="checkbox" name="consent" value="true" required;
                        " "
                        (r.id("web-diagnostics-consent"))
                    }
                }
                button type="submit" { (r.id("web-allow-diagnostics")) }
            }
        }
        @if r.mount.is_none() {
            section class="card stack" aria-labelledby="settings-leave" {
                h2 id="settings-leave" { (r.id("web-leave")) }
                p { (r.id("web-leave-help")) }
                (crate::leave::form(&r, &form.0))
            }
        }
    };
    page(&r, "/settings", &r.id("web-nav-settings"), &body)
}

/// Installation and group diagnostics use the same read-only request as CLI.
pub async fn doctor(State(web): State<Arc<Web>>) -> Html<String> {
    let r = web.renderer();
    let answers = web.ask(Request::Doctor { details: false }).await;
    let lines: Vec<String> = answers
        .iter()
        .flat_map(|answer| r.response(answer).0)
        .collect();
    let body = html! {
        div class="card" { (list(&lines)) }
        p { a class="btn" href=(r.url("/settings")) { (r.id("web-back")) } }
    };
    page(&r, "/settings", &r.id("web-doctor"), &body)
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

fn setting_id(key: &str) -> String {
    format!("setting-{}", key.replace(['.', '_'], "-"))
}

/// One setting, and the form that changes it.
fn setting(r: &Renderer, s: &Setting, form: &str) -> Markup {
    let id = setting_id(&s.key);
    html! {
        div {
            form method="post" action=(r.url("/settings")) class="setting-row" data-inline[s.key != "language"] {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="key" value=(s.key);
                label for=(id) { (r.id(&id)) " " }
                @if s.choices.is_empty() {
                    input id=(id) name="value" value=(&s.value) type="text";
                } @else {
                select id=(id) name="value" {
                    @for c in &s.choices {
                        option value=(c) selected[*c == s.value] { (label(r, &s.key, c)) }
                    }
                }
                }
                " "
                button type="submit" { (r.id("web-save")) }
            }
        }
    }
}

/// The pause, with only the action that applies: while sharing, a
/// duration and *Pause*; while paused, the pause and *Share again*.
fn pausing(r: &Renderer, form: &str) -> Markup {
    if let Some(banner) = &r.banner {
        return html! {
            article {
                p { (r.text(&banner.message)) }
                form method="post" action=(r.url("/resume")) {
                    input type="hidden" name="form" value=(form);
                    button type="submit" { (r.id("web-resume")) }
                }
            }
        };
    }
    html! {
        div class="inline-forms" {
            form method="post" action=(r.url("/pause")) class="setting-row" data-inline {
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
    let pause = matches!(request, Request::Pause { .. } | Request::Resume);
    let answers = web.ask(request).await;
    let mut r = web.renderer();
    // The result of *Pause* or *Share again* shows the pause it leaves.
    if pause {
        r.banner = web.banner(form).await;
    }
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
    if f.key == "language" && answers.iter().any(|a| {
        matches!(a, pigeon_proto::Response::Said { message } if message.id == "setting-changed")
    }) {
        let setting = pigeon_i18n::Setting::parse(&f.value).unwrap_or(pigeon_i18n::Setting::Auto);
        let system = pigeon_i18n::system_languages(|key| std::env::var(key).ok());
        web.set_lang(pigeon_i18n::choose(&setting, &system));
    }
    // After the answer, so that a new language shows at once.
    let r = web.renderer();
    let lines = answers.iter().flat_map(|a| {
        if matches!(a, pigeon_proto::Response::Said { message } if message.id == "setting-changed") {
            let message = pigeon_i18n::Message::new("web-setting-saved")
                .arg("key", r.id(&setting_id(&f.key)))
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
