//! Erasure pages (08 §8.6, 10 §10.3): GET only reads a typed preview.
//! Each write requires the session form token; destructive confirmation
//! additionally carries the daemon's exact reviewed-history token.

use crate::{
    FormToken, Web,
    machines::expired,
    pages::{self, list, page},
};
use axum::{
    Extension, Form,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use maud::html;
use pigeon_proto::{
    Request, Response as Reply,
    erase::{Answer, Operation, Selection},
};
use std::sync::Arc;

/// Exactly one selection, or none for the list.
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct At {
    path: Option<String>,
    suggestion: Option<String>,
    cancel: Option<String>,
}

impl At {
    fn selection(&self) -> Option<Selection> {
        match (&self.path, &self.suggestion, &self.cancel) {
            (Some(path), None, None) => Some(Selection::Path(path.clone())),
            (None, Some(id), None) => Some(Selection::Suggestion(id.clone())),
            _ => None,
        }
    }
    fn operation(&self) -> Option<Operation> {
        if let Some(selection) = self.selection() {
            return Some(Operation::Preview { selection });
        }
        match (&self.path, &self.suggestion, &self.cancel) {
            (None, None, Some(id)) => Some(Operation::PreviewCancel {
                decision: id.clone(),
            }),
            (None, None, None) => Some(Operation::List),
            _ => None,
        }
    }
}

/// List, review a path/suggestion, or review one pending cancellation.
pub async fn view(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
    Query(at): Query<At>,
) -> Response {
    let Some(operation) = at.operation() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let r = web.renderer();
    let answers = web.ask(Request::Erase { operation }).await;
    let lines: Vec<_> = answers.iter().flat_map(|a| r.response(a).0).collect();
    let answer = answers.iter().find_map(|a| match a {
        Reply::Erasure { answer } => Some(answer.as_ref()),
        _ => None,
    });
    let body = html! {
        (list(&lines))
        @if let Some(answer) = answer {
            @match answer {
                Answer::Preview(p) => {
                    @if p.readiness.ready || (p.readiness.can_prepare && p.readiness.preparation_needed) {
                        form method="post" action="/erase" {
                            input type="hidden" name="form" value=(form.0);
                            input type="hidden" name="history" value=(p.token);
                            @match &p.selection {
                                Selection::Path(path) => { input type="hidden" name="path" value=(path); },
                                Selection::Suggestion(id) => { input type="hidden" name="suggestion" value=(id); },
                            }
                            @if p.readiness.ready {
                                button type="submit" name="action" value="confirm" { (r.id(if p.needs_approval { "web-erase-suggest" } else { "web-erase-confirm" })) }
                            } @else {
                                button type="submit" name="action" value="prepare" { (r.id("web-erase-prepare")) }
                            }
                        }
                    }
                },
                Answer::CancelPreview { decision, token } => {
                    form method="post" action="/erase" {
                        input type="hidden" name="form" value=(form.0);
                        input type="hidden" name="history" value=(token);
                        input type="hidden" name="cancel" value=(decision.id);
                        button type="submit" name="action" value="cancel" { (r.id("web-erase-cancel")) }
                    }
                },
                Answer::Decisions { decisions, can_cancel: true } => {
                    @for decision in decisions.iter().filter(|d| !d.completed) {
                        p { a href=(format!("/erase?cancel={}", enc(&decision.id))) { (r.id("web-erase-review-cancel")) " " (decision.id) } }
                    }
                },
                _ => {},
            }
        }
        p { a href="/erase" { (r.id("web-erase-list")) } }
        p { a href="/files" { (r.id("web-back")) } }
    };
    page(&r, "/files", &r.id("web-erase-title"), &body).into_response()
}

fn enc(text: &str) -> String {
    form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// The explicit action, with independent session and history tokens.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submit {
    form: String,
    history: String,
    action: String,
    path: Option<String>,
    suggestion: Option<String>,
    cancel: Option<String>,
}

/// No write is inferred from an omitted or unrecognized action.
pub async fn submit(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Form(f): Form<Submit>,
) -> Response {
    if let Some(refused) = expired(&web, &token, &f.form) {
        return refused;
    }
    let at = At {
        path: f.path,
        suggestion: f.suggestion,
        cancel: f.cancel,
    };
    let operation = match (f.action.as_str(), at.selection(), at.operation()) {
        ("confirm", Some(selection), _) => Operation::Confirm {
            selection,
            token: f.history,
        },
        ("prepare", Some(selection), _) => Operation::Prepare { selection },
        ("cancel", None, Some(Operation::PreviewCancel { decision })) => Operation::Cancel {
            decision,
            token: f.history,
        },
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    let r = web.renderer();
    let back = match at.selection() {
        Some(Selection::Path(path)) => format!("/erase?path={}", enc(&path)),
        Some(Selection::Suggestion(id)) => format!("/erase?suggestion={}", enc(&id)),
        None => "/erase".into(),
    };
    let lines = pages::lines(&web, &r, Request::Erase { operation }).await;
    pages::result(&r, &back, lines).into_response()
}
