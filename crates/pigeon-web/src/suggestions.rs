//! Suggestions (ux.md §6.3, 10 §10.3, ADR 0076): those for this person to
//! decide, theirs, and the others, each listing its files with what
//! changes in each and a *Review* button that downloads the suggested
//! copy (`view.rs`). On one this person decides, every file has a
//! checkbox, all ticked: *Accept selected* accepts the ticked files and
//! declines the others, *Decline selected* the reverse, with a note for
//! the author (`DecideFiles`). With several to decide, *Accept all* and
//! *Decline all* decide every file of each. The author sees their
//! suggestions decided lately: which files were accepted, which declined,
//! and the note. A file's page says the same of that file
//! ([`on_file`]). Everything works without JavaScript: the forms are
//! plain, the review buttons belong to forms of their own (`form=`), and
//! the first button of a decision is disabled, so Enter in the note
//! decides nothing.

use std::sync::Arc;

use axum::Extension;
use axum::extract::{RawForm, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_proto::api::{FileChange, Outcome, SuggestedFile, Suggestion};
use pigeon_proto::{Request, Response as Answer};

use crate::pages::{list, page};
use crate::{FormToken, Renderer, Web, pages};

/// The pending suggestions, this person's decided lately, the group's
/// folder, and the lines of any problem.
struct Listing {
    pending: Vec<Suggestion>,
    decided: Vec<Outcome>,
    root: String,
    other: Vec<String>,
}

async fn listing(web: &Web, r: &Renderer) -> Listing {
    let mut out = Listing {
        pending: Vec::new(),
        decided: Vec::new(),
        root: String::new(),
        other: Vec::new(),
    };
    for answer in web.ask(Request::Proposals).await {
        match answer {
            Answer::Proposals {
                proposals,
                decided,
                root,
            } => {
                out.pending = proposals;
                out.decided = decided;
                out.root = root;
            }
            Answer::Progress { .. } => {}
            a => out.other.extend(r.response(&a).0),
        }
    }
    out
}

/// Whether this person decides `s` here: theirs only with an override.
fn decides(s: &Suggestion) -> bool {
    s.decide && (!s.mine || s.force)
}

/// The Suggestions page.
pub async fn page_of(
    State(web): State<Arc<Web>>,
    Extension(form): Extension<FormToken>,
) -> Html<String> {
    let r = web.renderer();
    let l = listing(&web, &r).await;
    let mine: Vec<&Suggestion> = l.pending.iter().filter(|s| s.mine && !decides(s)).collect();
    let decide: Vec<&Suggestion> = l.pending.iter().filter(|s| decides(s)).collect();
    let others: Vec<&Suggestion> = l
        .pending
        .iter()
        .filter(|s| !s.mine && !decides(s))
        .collect();
    let bulk: Vec<&&Suggestion> = decide.iter().filter(|s| !s.force).collect();
    let body = html! {
        @if l.pending.is_empty() && l.other.is_empty() { p { (r.id("suggestions-none")) } }
        (list(&l.other))
        @if bulk.len() > 1 {
            section {
                p { (r.catalogue().format(&Message::new("web-decide-all").arg("count", count(bulk.len())))) }
                form method="post" action=(r.url("/suggestions/all")) {
                    input type="hidden" name="form" value=(form.0);
                    @for s in &bulk { input type="hidden" name="id" value=(s.id); }
                    button type="submit" disabled hidden {}
                    p { label { (r.id("web-decline-note")) " " input type="text" name="note"; } }
                    button type="submit" name="decision" value="accept" { (r.id("web-accept-all")) }
                    " "
                    button type="submit" name="decision" value="decline" { (r.id("web-decline-all")) }
                }
            }
        }
        @if !decide.is_empty() {
            h2 { (r.id("web-suggestions-for-you")) }
            @for (i, s) in decide.iter().enumerate() { (deciding(&r, s, i, &form.0)) }
        }
        @if !mine.is_empty() {
            h2 { (r.id("web-suggestions-yours")) }
            @for s in &mine { (shown(&r, s, &form.0)) }
        }
        @if !others.is_empty() {
            h2 { (r.id("web-suggestions-others")) }
            @for s in &others { (shown(&r, s, &form.0)) }
        }
        @if !l.decided.is_empty() {
            h2 { (r.id("web-suggestions-decided")) }
            @for o in &l.decided { article { (list(&r.outcome(o))) } }
        }
    };
    page(&r, "/suggestions", &r.id("web-nav-suggestions"), &body)
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// The headline of `s`: who suggests what, when.
fn headline(r: &Renderer, s: &Suggestion) -> String {
    let saved = pigeon_proto::time::parse_rfc3339(&s.saved_at)
        .map_or_else(|| s.saved_at.clone(), |t| r.ago(crate::now() - t));
    let id = if s.mine {
        "web-suggestion-yours"
    } else if s.decide {
        "web-suggestion-decide"
    } else {
        "web-suggestion-other"
    };
    let msg = Message::new(id)
        .arg("paths", s.paths.join(", "))
        .arg("saved", saved);
    let msg = if s.mine {
        msg
    } else {
        msg.arg("author", s.author.clone())
    };
    r.catalogue().format(&msg)
}

/// The files of `s`, one per path when the daemon says nothing of them.
fn files(s: &Suggestion) -> Vec<SuggestedFile> {
    if !s.files.is_empty() {
        return s.files.clone();
    }
    s.paths
        .iter()
        .map(|p| SuggestedFile {
            path: p.clone(),
            change: FileChange::Changed,
            from: None,
            before: None,
            after: None,
            lines: None,
        })
        .collect()
}

/// What a file's change is, in words; just its name when nothing is known.
fn described(r: &Renderer, f: &SuggestedFile) -> String {
    if f.before.is_none() && f.after.is_none() {
        f.path.clone()
    } else {
        r.suggested_file(f)
    }
}

/// The form that downloads the suggested copy of `f`, owned by the button
/// `review` places wherever it is.
fn review_form(r: &Renderer, s: &Suggestion, f: &SuggestedFile, id: &str, form: &str) -> Markup {
    html! {
        form id=(id) method="post" action=(r.url("/suggestions/view")) {
            input type="hidden" name="form" value=(form);
            input type="hidden" name="proposal" value=(s.id);
            input type="hidden" name="path" value=(f.path);
        }
    }
}

fn review(r: &Renderer, f: &SuggestedFile, id: &str) -> Markup {
    let label = r
        .catalogue()
        .format(&Message::new("web-review-file").arg("path", f.path.clone()));
    html! {
        @if f.change != FileChange::Deleted {
            " " button type="submit" form=(id) formnovalidate { (label) }
        }
    }
}

/// A suggestion this person decides: its files, each ticked, and the
/// buttons that decide them.
fn deciding(r: &Renderer, s: &Suggestion, i: usize, form: &str) -> Markup {
    let files = files(s);
    let ids: Vec<String> = (0..files.len())
        .map(|j| format!("review-{i}-{j}"))
        .collect();
    let note = r
        .catalogue()
        .format(&Message::new("web-note-for").arg("author", s.author.clone()));
    // Which suggestion the buttons decide, for screen readers.
    let which = r.catalogue().format(
        &Message::new("web-suggestion-label")
            .arg("author", s.author.clone())
            .arg("paths", s.paths.join(", ")),
    );
    html! {
        article {
            p { (headline(r, s)) }
            @for (f, id) in files.iter().zip(&ids) { (review_form(r, s, f, id, form)) }
            form method="post" action=(r.url("/suggestions/decide")) {
                input type="hidden" name="form" value=(form);
                input type="hidden" name="id" value=(s.id);
                button type="submit" disabled hidden {}
                fieldset {
                    legend { (r.id("web-suggestion-files")) }
                    @if files.len() > 1 { p { (r.id("web-suggestion-choose")) } }
                    ul {
                        @for (f, id) in files.iter().zip(&ids) {
                            li {
                                label { input type="checkbox" name="path" value=(f.path) checked; " " (described(r, f)) }
                                (review(r, f, id))
                            }
                        }
                    }
                }
                @if s.force {
                    p { label { (r.id("web-override-reason")) " " input name="reason" required; } }
                }
                p { label { (note) " " input type="text" name="note"; } }
                button type="submit" name="decision" value="accept" { (r.id("web-accept-selected")) (crate::pages::hidden(&which)) }
                " "
                button type="submit" name="decision" value="decline" formnovalidate { (r.id("web-decline-selected")) (crate::pages::hidden(&which)) }
            }
        }
    }
}

/// A suggestion someone else decides, or this person's own: its files,
/// each with *Review*.
fn shown(r: &Renderer, s: &Suggestion, form: &str) -> Markup {
    let files = files(s);
    html! {
        article {
            p { (headline(r, s)) }
            ul {
                @for (j, f) in files.iter().enumerate() {
                    @let id = format!("review-{}-{j}", &s.id[..s.id.len().min(12)]);
                    li { (review_form(r, s, f, &id, form)) (described(r, f)) (review(r, f, &id)) }
                }
            }
        }
    }
}

/// The fields of a form, each name with every value it has.
fn fields(raw: &[u8]) -> Vec<(String, String)> {
    form_urlencoded::parse(raw).into_owned().collect()
}

fn field<'a>(fields: &'a [(String, String)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn values(fields: &[(String, String)], name: &str) -> Vec<String> {
    fields
        .iter()
        .filter(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .collect()
}

fn text(fields: &[(String, String)], name: &str) -> Option<String> {
    field(fields, name)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

fn expired(web: &Web) -> Response {
    (
        StatusCode::FORBIDDEN,
        pages::refused(web, "web-form-expired"),
    )
        .into_response()
}

/// *Accept selected* or *Decline selected*: `pigeon accept ID FILE…` or
/// `pigeon decline ID FILE…`, then what was decided, in words.
pub async fn decide(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    RawForm(raw): RawForm,
) -> Response {
    let f = fields(&raw);
    if token.0.is_empty() || field(&f, "form") != Some(token.0.as_str()) {
        return expired(&web);
    }
    let (Some(id), Some(decision)) = (field(&f, "id"), field(&f, "decision")) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let accept = match decision {
        "accept" => true,
        "decline" => false,
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    let request = Request::DecideFiles {
        proposal: id.to_owned(),
        paths: values(&f, "path"),
        accept,
        note: text(&f, "note"),
        reason: text(&f, "reason"),
    };
    let r = web.renderer();
    let lines = pages::lines(&web, &r, request).await;
    pages::result(&r, "/suggestions", lines).into_response()
}

/// *Accept all* or *Decline all*: every file of each suggestion the page
/// listed that this person still decides without an override.
pub async fn all(
    State(web): State<Arc<Web>>,
    Extension(token): Extension<FormToken>,
    RawForm(raw): RawForm,
) -> Response {
    let f = fields(&raw);
    if token.0.is_empty() || field(&f, "form") != Some(token.0.as_str()) {
        return expired(&web);
    }
    let accept = match field(&f, "decision") {
        Some("accept") => true,
        Some("decline") => false,
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    let chosen = values(&f, "id");
    let r = web.renderer();
    let l = listing(&web, &r).await;
    let mut lines = l.other;
    let mut done = 0;
    for s in l.pending.iter().filter(|s| chosen.contains(&s.id)) {
        if !decides(s) {
            continue;
        }
        if s.force {
            let m = Message::new("web-decide-all-skipped").arg("author", s.author.clone());
            lines.push(r.catalogue().format(&m));
            continue;
        }
        let request = Request::DecideFiles {
            proposal: s.id.clone(),
            paths: s.paths.clone(),
            accept,
            note: text(&f, "note").filter(|_| !accept),
            reason: None,
        };
        let answers = web.ask(request).await;
        if answers
            .iter()
            .any(|a| matches!(a, Answer::Said { .. } | Answer::Done))
        {
            done += 1;
        }
        lines.extend(
            answers
                .iter()
                .filter(|a| !matches!(a, Answer::Progress { .. }))
                .flat_map(|a| r.response(a).0),
        );
    }
    let title = r
        .catalogue()
        .format(&Message::new("web-decided-all").arg("count", count(done)));
    let mut all = vec![title];
    all.extend(lines);
    pages::result(&r, "/suggestions", all).into_response()
}

/// On a file's page at `path`: this person's suggestion of it that waits,
/// and how their suggestions of it were decided lately.
pub async fn on_file(web: &Web, r: &Renderer, path: &str) -> Markup {
    let l = listing(web, r).await;
    let Some(rel) = std::path::Path::new(path)
        .strip_prefix(&l.root)
        .ok()
        .and_then(|p| p.to_str())
        .filter(|_| !l.root.is_empty())
    else {
        return html! {};
    };
    let waiting: Vec<&Suggestion> = l
        .pending
        .iter()
        .filter(|s| s.mine && s.paths.iter().any(|p| p == rel))
        .collect();
    let decided: Vec<&Outcome> = l
        .decided
        .iter()
        .filter(|o| o.accepted.iter().chain(&o.declined).any(|p| p == rel))
        .collect();
    html! {
        @for s in &waiting {
            @let saved = pigeon_proto::time::parse_rfc3339(&s.saved_at)
                .map_or_else(|| s.saved_at.clone(), |t| r.ago(crate::now() - t));
            p { (r.catalogue().format(&Message::new("web-file-suggestion-waiting").arg("saved", saved))) }
        }
        @for o in &decided { (list(&r.outcome(o))) }
    }
}
