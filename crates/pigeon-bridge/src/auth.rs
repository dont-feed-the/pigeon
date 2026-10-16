//! Browser sessions and Nextcloud login flow v2. A browser authenticates a
//! person; each granted app gets a new daemon and a separately revocable random
//! password. User-Agent is a suggested label, never an identity or credential.

use std::sync::Arc;

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Form, Json};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

use crate::{
    App, Error, Result, bad, forbidden, rpc,
    store::{self, Client},
    unavailable,
};

#[derive(Clone)]
pub struct Session {
    pub user: String,
    pub csrf: String,
    pub expires: u64,
}

pub struct Flow {
    pub poll_hash: String,
    pub label: String,
    pub expires: u64,
    pub credentials: Option<(String, String)>,
}

pub async fn session(app: &App, headers: &HeaderMap) -> Option<Session> {
    let cookie = headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == "pigeon-bridge-session")?
        .1;
    let mut sessions = app.sessions.lock().await;
    sessions.retain(|_, s| s.expires > store::now() && app.config.users.contains_key(&s.user));
    sessions.get(cookie).cloned()
}

pub fn csrf(session: &Session, token: &str) -> Result<()> {
    if token.len() == session.csrf.len()
        && bool::from(session.csrf.as_bytes().ct_eq(token.as_bytes()))
    {
        Ok(())
    } else {
        Err(forbidden())
    }
}

pub async fn client(app: &App, headers: &HeaderMap) -> Result<Client> {
    let unauthorized = || {
        Error(
            StatusCode::UNAUTHORIZED,
            "Use this client's app password".into(),
        )
    };
    let encoded = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Basic "))
        .ok_or_else(unauthorized)?;
    if encoded.len() > 4096 {
        return Err(unauthorized());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| unauthorized())?;
    let text = std::str::from_utf8(&bytes).map_err(|_| unauthorized())?;
    let (user, password) = text.split_once(':').ok_or_else(unauthorized)?;
    if !app.config.users.contains_key(user) {
        return Err(unauthorized());
    }
    let hash = store::hash(password.as_bytes());
    app.store
        .lock()
        .await
        .clients
        .values()
        .find(|c| {
            c.user == user
                && !c.revoked
                && bool::from(c.token_hash.as_bytes().ct_eq(hash.as_bytes()))
        })
        .cloned()
        .ok_or_else(unauthorized)
}

#[derive(Deserialize)]
pub struct Login {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub flow: String,
}

pub async fn login(State(app): State<Arc<App>>, Form(form): Form<Login>) -> Result<Response> {
    if form.username.len() > 80 || form.password.len() > 1024 {
        return Err(forbidden());
    }
    {
        let mut attempts = app.login_attempts.lock().await;
        attempts.retain(|_, (at, _)| store::now().saturating_sub(*at) < 60);
        if attempts.len() > 1024 {
            return Err(Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Try signing in later".into(),
            ));
        }
        let entry = attempts
            .entry(form.username.clone())
            .or_insert((store::now(), 0));
        entry.1 += 1;
        if entry.1 > 10 {
            return Err(Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Try signing in in a minute".into(),
            ));
        }
    }
    let hash = app
        .config
        .users
        .get(&form.username)
        .map(|u| u.password_hash.clone());
    let password = form.password;
    let verified = tokio::task::spawn_blocking(move || {
        hash.is_some_and(|h| {
            PasswordHash::new(&h).is_ok_and(|parsed| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            })
        })
    })
    .await
    .map_err(unavailable)?;
    if !verified {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "The username or password is incorrect".into(),
        ));
    }
    let token = store::random().map_err(unavailable)?;
    let session = Session {
        user: form.username,
        csrf: store::random().map_err(unavailable)?,
        expires: store::now() + 12 * 3600,
    };
    let mut sessions = app.sessions.lock().await;
    sessions.retain(|_, s| s.expires > store::now());
    if sessions.len() >= 1024 {
        return Err(unavailable("Too many browser sessions"));
    }
    sessions.insert(token.clone(), session);
    let next = if form.flow.is_empty() {
        "/".into()
    } else {
        if !app.flows.lock().await.contains_key(&form.flow) {
            return Err(bad("The client authorization expired"));
        }
        format!("/login/v2/flow/{}", form.flow)
    };
    let secure = if app.config.public_url.starts_with("https:") {
        "; Secure"
    } else {
        ""
    };
    let mut response = Redirect::to(&next).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        format!(
            "pigeon-bridge-session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=43200{secure}"
        )
        .parse()
        .map_err(unavailable)?,
    );
    Ok(response)
}

#[derive(Deserialize)]
pub struct Token {
    pub token: String,
}

pub async fn logout(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Form(form): Form<Token>,
) -> Result<Response> {
    let s = session(&app, &headers).await.ok_or_else(forbidden)?;
    csrf(&s, &form.token)?;
    app.sessions.lock().await.retain(|_, x| x.csrf != s.csrf);
    let mut response = Redirect::to("/login").into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        axum::http::HeaderValue::from_static(
            "pigeon-bridge-session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        ),
    );
    Ok(response)
}

pub async fn begin(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>> {
    let token = store::random().map_err(unavailable)?;
    let poll = store::random().map_err(unavailable)?;
    let label = headers
        .get(header::USER_AGENT)
        .and_then(|s| s.to_str().ok())
        .unwrap_or("Nextcloud client")
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    let mut flows = app.flows.lock().await;
    flows.retain(|_, f| f.expires > store::now());
    if flows.len() >= 256 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many pending client authorizations".into(),
        ));
    }
    flows.insert(
        token.clone(),
        Flow {
            poll_hash: store::hash(poll.as_bytes()),
            label,
            expires: store::now() + 1200,
            credentials: None,
        },
    );
    Ok(Json(
        json!({"poll":{"token":poll,"endpoint":format!("{}/login/v2/poll",app.config.public_url)},
        "login":format!("{}/login/v2/flow/{token}", app.config.public_url)}),
    ))
}

pub async fn poll(State(app): State<Arc<App>>, Form(form): Form<Token>) -> Result<Json<Value>> {
    let hash = store::hash(form.token.as_bytes());
    let mut flows = app.flows.lock().await;
    flows.retain(|_, f| f.expires > store::now());
    let id = flows
        .iter()
        .find(|(_, f)| bool::from(f.poll_hash.as_bytes().ct_eq(hash.as_bytes())))
        .map(|(id, _)| id.clone())
        .ok_or_else(|| {
            Error(
                StatusCode::NOT_FOUND,
                "Authorization pending or expired".into(),
            )
        })?;
    let credentials = flows.get_mut(&id).and_then(|f| f.credentials.take());
    if let Some((username, password)) = credentials {
        flows.remove(&id);
        Ok(Json(
            json!({"server":app.config.public_url,"loginName":username,"appPassword":password}),
        ))
    } else {
        Err(Error(StatusCode::NOT_FOUND, "Authorization pending".into()))
    }
}

#[derive(Deserialize)]
pub struct Grant {
    pub csrf: String,
    pub name: String,
}

pub async fn grant(
    State(app): State<Arc<App>>,
    Path(token): Path<String>,
    headers: HeaderMap,
    Form(form): Form<Grant>,
) -> Result<Response> {
    let session = session(&app, &headers).await.ok_or_else(forbidden)?;
    csrf(&session, &form.csrf)?;
    let _grant = app.grants.lock().await;
    let name: String = form
        .name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    if name.is_empty() {
        return Err(bad("Give this client a name"));
    }
    {
        let flows = app.flows.lock().await;
        let flow = flows
            .get(&token)
            .filter(|f| f.expires > store::now())
            .ok_or_else(|| bad("Authorization expired"))?;
        if flow.credentials.is_some() {
            return Err(bad("This authorization has already been granted"));
        }
    }
    let count = app
        .store
        .lock()
        .await
        .clients
        .values()
        .filter(|c| c.user == session.user && !c.revoked)
        .count();
    if count >= app.config.max_clients_per_user {
        return Err(bad("This account has reached its client limit"));
    }
    let password = store::random().map_err(unavailable)?;
    let client = app
        .peers
        .provision(&session.user, name, &password)
        .await
        .map_err(unavailable)?;
    {
        let mut store = app.store.lock().await;
        store.clients.insert(client.id.clone(), client.clone());
        if let Err(e) = store.save(&app.config.state_dir) {
            store.clients.remove(&client.id);
            app.peers.stop(&client.id).await;
            return Err(unavailable(e));
        }
    }
    app.resume_peers().await;
    // Provisioning may take time: never issue credentials to an expired poll.
    let mut flows = app.flows.lock().await;
    let flow = flows
        .get_mut(&token)
        .filter(|f| f.expires > store::now())
        .ok_or_else(|| bad("Authorization expired; revoke the unused client in the portal"))?;
    flow.credentials = Some((session.user, password));
    Ok(crate::ui::page("Client connected", maud::html! {
        p { "This client now has its own Pigeon peer. Return to the Nextcloud app to finish setup." }
        a href=(format!("/clients/{}", client.id)) { "View this client" }
    }).into_response())
}

pub fn ocs(data: Value) -> Json<Value> {
    Json(json!({"ocs":{"meta":{"status":"ok","statuscode":200,"message":"OK"},"data":data}}))
}

pub async fn status() -> Json<Value> {
    Json(
        json!({"installed":true,"maintenance":false,"needsDbUpgrade":false,"version":"35.0.0.0",
        "versionstring":"35.0.0","edition":"","productname":"Pigeon Bridge","extendedSupport":false}),
    )
}

pub async fn capabilities() -> Json<Value> {
    ocs(
        json!({"version":{"major":35,"minor":0,"micro":0,"string":"35.0.0","edition":""},
        "capabilities":{"core":{"pollinterval":5,"webdav-root":"remote.php/webdav"},"dav":{"chunking":"1.0"},
        "files":{"bigfilechunking":true,"undelete":false,"versioning":false},"files_sharing":{"api_enabled":false},
        "checksums":{"supportedTypes":["SHA256"],"preferredUploadType":"SHA256"}}}),
    )
}

pub async fn user(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>> {
    let c = client(&app, &headers).await?;
    Ok(ocs(
        json!({"id":c.user,"display-name":c.user,"enabled":true,"quota":{"free":-3,"used":0,"total":-3,"relative":0}}),
    ))
}

pub async fn existing_password(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
) -> Result<Response> {
    client(&app, &headers).await?;
    Ok(StatusCode::FORBIDDEN.into_response())
}

pub async fn revoke(app: &App, client: &Client) -> Result<()> {
    let lock = app.client_lock(&client.id).await;
    let guard = lock.lock().await;
    {
        let mut store = app.store.lock().await;
        let c = store.clients.get_mut(&client.id).ok_or_else(forbidden)?;
        c.revoked = true;
        store.save(&app.config.state_dir).map_err(unavailable)?;
    }
    app.peers.stop(&client.id).await;
    drop(guard);
    // Access is already revoked even if the control peer is temporarily offline.
    let socket = &app
        .config
        .users
        .get(&client.user)
        .ok_or_else(forbidden)?
        .control_socket;
    let name = match rpc::call(socket, pigeon_proto::Request::Machines)
        .await
        .map_err(unavailable)?
    {
        pigeon_proto::Response::Machines { machines, .. } => machines
            .into_iter()
            .find(|m| m.device == client.device)
            .map(|m| m.name),
        _ => {
            return Err(unavailable(
                "Client access revoked; cannot read the control peer's roster",
            ));
        }
    };
    let Some(name) = name else {
        return Ok(());
    };
    let response = rpc::call(socket, pigeon_proto::Request::RemoveDevice { name })
        .await
        .map_err(unavailable)?;
    if let pigeon_proto::Response::Problem(p) = response {
        return Err(unavailable(format!(
            "Client access revoked; Pigeon device removal needs attention: {}",
            p.code
        )));
    }
    Ok(())
}

pub async fn delete_password(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let c = client(&app, &headers).await?;
    revoke(&app, &c).await?;
    Ok(ocs(json!({})))
}
