//! The browser portal: person-level sign-in, client identities, Pigeon status,
//! suggestions and conflict resolutions. Every decision is sent through that
//! user's peer; the daemon, not the browser, checks the person's authority.

use std::sync::Arc;

use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{
    Form,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use maud::{DOCTYPE, Markup, html};
use pigeon_proto::{Request, Response as Answer};
use serde::Deserialize;

use crate::{App, Result, auth, bad, forbidden, index, rpc, store::Client, unavailable};

pub fn page(title: &str, body: Markup) -> Html<String> {
    Html(html! {
        (DOCTYPE)
        html lang="en" {
            head { meta charset="utf-8"; meta name="viewport" content="width=device-width,initial-scale=1";
                title { (title) " · Pigeon Bridge" }
                style { "body{font:16px/1.55 system-ui,sans-serif;max-width:64rem;margin:2rem auto;padding:0 1rem;color:#172e30;background:#f7faf9}nav{display:flex;gap:1.5rem;border-bottom:1px solid #cedbd7;padding-bottom:1rem}a{color:#176254}h1{font-size:2rem}article{background:white;border:1px solid #d5e0dc;border-radius:.6rem;padding:1rem;margin:1rem 0}label{display:block;margin:.6rem 0}input{padding:.5rem;max-width:95%}button{padding:.5rem 1rem;background:#176254;color:white;border:0;border-radius:.3rem;cursor:pointer}form{margin:.7rem 0}code{overflow-wrap:anywhere}small{color:#47605d}.error{color:#963b2f}table{border-collapse:collapse;width:100%}td,th{text-align:left;padding:.5rem;border-bottom:1px solid #d5e0dc}" }
            }
            body { nav { strong { "Pigeon Bridge" } a href="/" { "Clients" } a href="/suggestions" { "Suggestions" } }
                main { h1 { (title) } (body) }
            }
        }
    }.into_string())
}

#[derive(Default, Deserialize)]
pub struct LoginQuery {
    #[serde(default)]
    flow: String,
}

pub async fn login(Query(query): Query<LoginQuery>) -> Html<String> {
    page(
        "Sign in",
        html! {
            p { "Sign in to manage your Pigeon clients and review suggestions." }
            form method="post" action="/login" {
                input type="hidden" name="flow" value=(query.flow);
                label { "Username " input name="username" autocomplete="username" required; }
                label { "Password " input type="password" name="password" autocomplete="current-password" required; }
                button { "Sign in" }
            }
        },
    )
}

pub async fn home(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Response> {
    let Some(session) = auth::session(&app, &headers).await else {
        return Ok(Redirect::to("/login").into_response());
    };
    let clients: Vec<_> = app
        .store
        .lock()
        .await
        .clients
        .values()
        .filter(|c| c.user == session.user)
        .cloned()
        .collect();
    Ok(page("Your clients", html! {
        p { "Signed in as " strong { (&session.user) } "." }
        p { "In your Nextcloud app, add an account using " code { (&app.config.public_url) } ". Authorize each installation separately to give it its own Pigeon peer." }
        p { "A client’s sync indicator means its bridge peer has the file. Pigeon’s protection status also checks backup machines." }
        @if clients.is_empty() { p { "No clients connected yet." } }
        @for c in &clients {
            article {
                h2 { a href=(format!("/clients/{}", c.id)) { (&c.name) } }
                p { @if c.revoked { "Access revoked" } @else { "Connected · separate Pigeon peer" } }
                small { "Peer " code { (&c.device) } }
            }
        }
        form method="post" action="/logout" {
            input type="hidden" name="token" value=(&session.csrf); button { "Sign out" }
        }
    }).into_response())
}

pub async fn grant(
    State(app): State<Arc<App>>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Result<Response> {
    let Some(session) = auth::session(&app, &headers).await else {
        return Ok(Redirect::to(&format!("/login?flow={}", url_encode(&token))).into_response());
    };
    let flows = app.flows.lock().await;
    let flow = flows
        .get(&token)
        .filter(|f| f.expires > crate::store::now())
        .ok_or_else(|| bad("This authorization has expired; try again from the client"))?;
    if flow.credentials.is_some() {
        return Ok(page(
            "Client authorized",
            html! { p { "Return to your Nextcloud app." } },
        )
        .into_response());
    }
    Ok(page("Connect a client", html! {
        p { "This creates a separate Pigeon peer belonging to " strong { (&session.user) } ". The bridge keeps this peer’s key and files." }
        form method="post" {
            input type="hidden" name="csrf" value=(&session.csrf);
            label { "Client name " input name="name" value=(&flow.label) maxlength="80" required; }
            button { "Connect this client" }
        }
    }).into_response())
}

pub async fn owned(app: &App, user: &str, id: &str) -> Result<Client> {
    app.store
        .lock()
        .await
        .clients
        .get(id)
        .filter(|c| c.user == user)
        .cloned()
        .ok_or_else(forbidden)
}

fn renderer() -> pigeon_i18n::render::Renderer {
    let lang = pigeon_i18n::choose(&pigeon_i18n::Setting::Auto, &[]);
    pigeon_i18n::render::Renderer::new(
        pigeon_i18n::Catalogue::new(&lang, false),
        crate::store::now() as i64,
    )
}

pub async fn suggestions(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Response> {
    let Some(session) = auth::session(&app, &headers).await else {
        return Ok(Redirect::to("/login").into_response());
    };
    let socket = &app
        .config
        .users
        .get(&session.user)
        .ok_or_else(forbidden)?
        .control_socket;
    let answer = rpc::call(socket, Request::Proposals)
        .await
        .map_err(unavailable)?;
    let proposals = match answer {
        Answer::Proposals { proposals } => proposals,
        other => return Ok(result(other, "/suggestions")),
    };
    Ok(page("Suggestions", html! {
        p { "These suggestions come from the group. Pigeon checks which ones your person may decide." }
        @if proposals.is_empty() { p { "No suggestions waiting." } }
        @for s in proposals {
            article {
                h2 { (&s.author) " · " (&s.device) }
                p { "Saved " (&s.saved_at) }
                ul { @for path in &s.paths { li { (path) } } }
                @if s.mine { p { "Submitted by one of your peers." } }
                @if s.decide {
                    form method="post" action="/decide" {
                        input type="hidden" name="csrf" value=(&session.csrf);
                        input type="hidden" name="proposal" value=(&s.id);
                        @if s.force { label { "Override reason " input name="reason" required; } }
                        button name="decision" value="accept" { @if s.force { "Accept with admin override" } @else { "Accept" } }
                    }
                    form method="post" action="/decide" {
                        input type="hidden" name="csrf" value=(&session.csrf);
                        input type="hidden" name="proposal" value=(&s.id);
                        label { "Note (optional) " input name="note"; }
                        button name="decision" value="decline" { "Decline" }
                    }
                } @else { p { "Waiting for the folder’s owner or stewards." } }
            }
        }
    }).into_response())
}

#[derive(Deserialize)]
pub struct Decision {
    csrf: String,
    proposal: String,
    decision: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    reason: String,
}

pub async fn decide(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Form(form): Form<Decision>,
) -> Result<Response> {
    let session = auth::session(&app, &headers).await.ok_or_else(forbidden)?;
    auth::csrf(&session, &form.csrf)?;
    let request = match form.decision.as_str() {
        "accept" => Request::Accept {
            proposal: form.proposal,
            reason: (!form.reason.trim().is_empty()).then_some(form.reason),
        },
        "decline" => Request::Reject {
            proposal: form.proposal,
            note: (!form.note.trim().is_empty()).then_some(form.note),
        },
        _ => return Err(bad("Choose accept or decline")),
    };
    let socket = &app
        .config
        .users
        .get(&session.user)
        .ok_or_else(forbidden)?
        .control_socket;
    Ok(result(
        rpc::call(socket, request).await.map_err(unavailable)?,
        "/suggestions",
    ))
}

fn result(answer: Answer, back: &str) -> Response {
    let (lines, error) = renderer().response(&answer);
    page(
        if error { "Needs attention" } else { "Done" },
        html! {
            @for line in lines { p { (line) } } a href=(back) { "Back" }
        },
    )
    .into_response()
}

pub async fn client(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response> {
    let Some(session) = auth::session(&app, &headers).await else {
        return Ok(Redirect::to("/login").into_response());
    };
    let c = owned(&app, &session.user, &id).await?;
    if c.revoked {
        return Ok(page(&c.name, html! { p { "This client’s access has been revoked. Its retained peer data remains on the bridge." } }).into_response());
    }
    let lock = app.client_lock(&id).await;
    let _lock = lock.lock().await;
    let socket = app.peers.checked(&c).await.map_err(unavailable)?;
    let status = rpc::call(
        &socket,
        Request::Status {
            path: None,
            not_shared: false,
        },
    )
    .await
    .map_err(unavailable)?;
    let lines = renderer().response(&status).0;
    let conflicts = match rpc::call(&socket, Request::Conflicts)
        .await
        .map_err(unavailable)?
    {
        Answer::Conflicts { conflicts } => conflicts,
        other => return Ok(result(other, "/")),
    };
    let index = index::load(&app, &c).await?;
    Ok(page(&c.name, html! {
        p { "Pigeon peer " code { (&c.device) } }
        @for line in lines { p { (line) } }
        @for conflict in conflicts {
            article {
                h2 { (&conflict.path) }
                p { (&conflict.kind) " · " (&conflict.state) }
                @if conflict.state == "waiting" { p { "Your saved version is visible to this client while the group reviews the suggestion." } }
                form method="post" action=(format!("/clients/{id}/resolve")) {
                    input type="hidden" name="csrf" value=(&session.csrf);
                    input type="hidden" name="path" value=(&conflict.path);
                    button name="choice" value="take-group" { "Take the group’s version" }
                }
                form method="post" action=(format!("/clients/{id}/resolve")) {
                    input type="hidden" name="csrf" value=(&session.csrf);
                    input type="hidden" name="path" value=(&conflict.path);
                    label { "Keep my version at " input name="destination" required; }
                    button name="choice" value="keep-copy" { "Keep mine as a copy" }
                }
                form method="post" action=(format!("/clients/{id}/resolve")) {
                    input type="hidden" name="csrf" value=(&session.csrf);
                    input type="hidden" name="path" value=(&conflict.path);
                    button name="choice" value="suggest-again" { "Suggest again" }
                }
            }
        }
        h2 { "Files" }
        table { tr { th { "File" } th { "Size" } th { "State" } }
            @for node in index.nodes.values().filter(|n| !n.directory) {
                tr { td { a href=(format!("/clients/{id}/file?path={}",url_encode(&node.path))) { (&node.path) } }
                    td { (node.size) " bytes" } td { @if node.diverging { "Your saved version" } @else { "Group version" } } }
            }
        }
        form method="post" action=(format!("/clients/{id}/revoke")) {
            input type="hidden" name="token" value=(&session.csrf); button { "Revoke this client" }
        }
    }).into_response())
}

#[derive(Deserialize)]
pub struct FileQuery {
    path: String,
}

pub async fn file(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
) -> Result<Response> {
    let session = auth::session(&app, &headers).await.ok_or_else(forbidden)?;
    let c = owned(&app, &session.user, &id).await?;
    if c.revoked {
        return Err(forbidden());
    }
    let lock = app.client_lock(&id).await;
    let _lock = lock.lock().await;
    let index = index::load(&app, &c).await?;
    let node = index
        .nodes
        .get(&query.path)
        .filter(|n| !n.directory)
        .ok_or_else(|| bad("File not found"))?;
    crate::dav::download(&app, &c, node, &headers, false).await
}

#[derive(Deserialize)]
pub struct Resolution {
    csrf: String,
    path: String,
    choice: String,
    #[serde(default)]
    destination: String,
}

pub async fn resolve(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Form(form): Form<Resolution>,
) -> Result<Response> {
    let session = auth::session(&app, &headers).await.ok_or_else(forbidden)?;
    auth::csrf(&session, &form.csrf)?;
    let c = owned(&app, &session.user, &id).await?;
    if c.revoked || !index::path_valid(&form.path) {
        return Err(forbidden());
    }
    let lock = app.client_lock(&id).await;
    let _lock = lock.lock().await;
    let socket = app.peers.checked(&c).await.map_err(unavailable)?;
    let root = match index::reply(
        rpc::bridge(&socket, pigeon_proto::bridge::Operation::Info)
            .await
            .map_err(unavailable)?,
    )? {
        pigeon_proto::bridge::Reply::Info { root, .. } => std::path::PathBuf::from(root),
        _ => return Err(unavailable("No client folder")),
    };
    let choice = match form.choice.as_str() {
        "take-group" => pigeon_proto::api::Choice::TakeGroup,
        "suggest-again" => pigeon_proto::api::Choice::SuggestAgain,
        "keep-copy" if index::path_valid(&form.destination) => {
            pigeon_proto::api::Choice::KeepCopy {
                dest: root.join(form.destination).display().to_string(),
            }
        }
        _ => return Err(bad("Choose a resolution and a relative destination")),
    };
    let answer = rpc::call(
        &socket,
        Request::Resolve {
            path: root.join(form.path).display().to_string(),
            choice,
        },
    )
    .await
    .map_err(unavailable)?;
    Ok(result(answer, &format!("/clients/{id}")))
}

pub async fn revoke(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Form(form): Form<auth::Token>,
) -> Result<Response> {
    let session = auth::session(&app, &headers).await.ok_or_else(forbidden)?;
    auth::csrf(&session, &form.token)?;
    let c = owned(&app, &session.user, &id).await?;
    auth::revoke(&app, &c).await?;
    Ok(Redirect::to("/").into_response())
}

fn url_encode(s: &str) -> String {
    form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
