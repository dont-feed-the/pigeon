//! Folder sharing controls (04 §4.3, 10 §10.3). A checked session form
//! selects people by identity; the daemon repeats authority checks. Raw
//! form parsing supports repeated steward selections without JavaScript.
use crate::{
    FormToken, Web,
    machines::expired,
    pages::{self, page},
};
use axum::{
    Extension,
    extract::{Query, RawForm, State},
    response::{Html, IntoResponse, Response},
};
use maud::html;
use pigeon_proto::{Request, policy};
use std::sync::Arc;

/// The local folder whose sharing rule is being configured.
#[derive(serde::Deserialize)]
pub struct At {
    path: String,
}

/// Read the resolved rule and show an admin's change form.
pub async fn view(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    Query(at): Query<At>,
) -> Html<String> {
    let r = web.renderer();
    let answers = web
        .ask(Request::FolderRules {
            folder: Some(at.path.clone()),
        })
        .await;
    let Some(report) = answers.iter().find_map(|r| match r {
        pigeon_proto::Response::FolderRules(report) => Some(report),
        _ => None,
    }) else {
        return pages::result(
            &r,
            "/files",
            answers.iter().flat_map(|a| r.response(a).0).collect(),
        );
    };
    let body = html! {
        p { (r.id("web-folder-rule-help")) }
        @for rule in &report.rules {
            p { (r.folder_rule(rule, &report.people)) }
            @if report.can_edit && !rule.path.is_empty() {
                form method="post" action="/folder-rule" {
                    input type="hidden" name="form" value=(&token.0);
                    input type="hidden" name="path" value=(std::path::Path::new(&report.root).join(&rule.path).to_string_lossy());
                    p { label { input type="radio" name="kind" value="owned" checked[rule.direct && rule.owner.is_some()]; (r.id("web-folder-rule-owner")) } }
                    p { label for="policy-owner" { (r.id("web-folder-rule-owner")) } " "
                        select id="policy-owner" name="owner" {
                            @for person in &report.people {
                                option value=(&person.id) selected[rule.owner.as_ref().map_or(person.this_person, |id| id == &person.id)] { (&person.name) }
                            }
                        }
                    }
                    p { label { input type="radio" name="kind" value="add-only" checked[rule.direct && rule.owner.is_none()]; (r.id("web-folder-rule-add-only")) } }
                    fieldset {
                        legend { (r.id("web-folder-rule-stewards")) }
                        p { (r.id("web-folder-rule-default-admins")) }
                        @for person in &report.people {
                            p { label { input type="checkbox" name="steward" value=(&person.id) checked[rule.stewards.contains(&person.id)]; (&person.name) } }
                        }
                    }
                    p { label { input type="radio" name="kind" value="inherit" checked[!rule.direct]; (r.id("web-folder-rule-inherit")) } }
                    button type="submit" { (r.id("web-save")) }
                }
            } @else { p { (r.id("web-folder-rule-admin")) } }
        }
        p { a href="/files" { (r.id("web-back")) } }
    };
    page(&r, "/files", &r.id("web-folder-rule"), &body)
}

/// Validate the same CSRF token as every other write, then send one change.
pub async fn set(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    RawForm(raw): RawForm,
) -> Response {
    let fields: Vec<_> = form_urlencoded::parse(&raw).into_owned().collect();
    let one = |name: &str| {
        let values: Vec<_> = fields
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .collect();
        (values.len() == 1).then(|| values[0])
    };
    if let Some(refused) = expired(&web, &token, one("form").unwrap_or_default()) {
        return refused;
    }
    let Some(path) = one("path") else {
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    };
    let rule = match one("kind") {
        Some("owned") => match one("owner") {
            Some(person) => policy::Change::Owned {
                person: person.into(),
            },
            None => return axum::http::StatusCode::BAD_REQUEST.into_response(),
        },
        Some("add-only") => policy::Change::AddOnly {
            stewards: fields
                .iter()
                .filter(|(key, _)| key == "steward")
                .map(|(_, value)| value.clone())
                .collect(),
        },
        Some("inherit") => policy::Change::Inherit,
        _ => return axum::http::StatusCode::BAD_REQUEST.into_response(),
    };
    let r = web.renderer();
    pages::result(
        &r,
        "/files",
        pages::lines(
            &web,
            &r,
            Request::SetFolderRule {
                folder: path.into(),
                rule,
            },
        )
        .await,
    )
    .into_response()
}
