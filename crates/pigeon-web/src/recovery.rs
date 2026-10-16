//! Printable recovery sheets (09 §9.9). Only an authenticated local POST
//! creates one. The secret never goes in a URL or a cacheable response.

use std::sync::Arc;

use axum::{
    Extension, Form,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use maud::{PreEscaped, html};
use pigeon_proto::{Arg, Request, Response as Answer};

use crate::{FormToken, Web, machines::expired, pages};

/// The current session's form token.
#[derive(serde::Deserialize)]
pub struct SheetForm {
    form: String,
}

/// Prepare a paper key, confirmed by a backup machine before showing it.
pub async fn sheet(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<SheetForm>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let r = web.renderer();
    let answers = web.ask(Request::RecoverySheet).await;
    let sheet = answers.iter().find_map(|answer| match answer {
        Answer::Said { message } if message.id == "recovery-sheet-text" => {
            match message.args.get("sheet") {
                Some(Arg::Text(text)) => Some(text),
                _ => None,
            }
        }
        _ => None,
    });
    let result = if let Some(sheet) = sheet {
        let text = pigeon_i18n::recovery::text(r.catalogue(), sheet);
        let code = pigeon_i18n::recovery::code(sheet);
        let instructions = code
            .and_then(|code| text.rsplit_once(code))
            .map_or(text.as_str(), |(before, _)| before);
        let qr = code.and_then(pigeon_i18n::recovery::svg);
        let body = html! {
            p class="no-print" { (r.id("web-recovery-print-help")) }
            div class="recovery-instructions" {
                @for line in instructions.lines().skip(1) { p { (line) } }
            }
            @if let Some(code) = code { pre class="recovery-code" { (code) } }
            @if let Some(qr) = qr {
                div class="recovery-qr" role="img" aria-label=(r.id("recovery-title")) { (PreEscaped(qr)) }
            } @else { p { (r.id("recovery-qr-too-large")) } }
            p class="no-print" { a href=(r.url("/machines")) { (r.id("web-back")) } }
        };
        pages::page(&r, "/machines", &r.id("recovery-title"), &body)
    } else {
        pages::result(
            &r,
            "/machines",
            answers.iter().flat_map(|a| r.response(a).0).collect(),
        )
    };
    ([(header::CACHE_CONTROL, "no-store")], result).into_response()
}
