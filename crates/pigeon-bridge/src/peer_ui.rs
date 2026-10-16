//! The regular peer UI behind the bridge's browser session. A mount selects
//! one client; each RPC repeats ownership and revocation checks, including
//! polls on an already open event stream. No daemon web cookie is forwarded.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use pigeon_proto::{Message, Problem, Request as Rpc, Response as Answer};
use tower::ServiceExt;

use crate::{App, Result, auth, forbidden, rpc, ui, unavailable};

struct Backend {
    app: Arc<App>,
    headers: HeaderMap,
    client: Option<String>,
}

impl Backend {
    async fn call(&self, request: Rpc) -> Result<Answer> {
        if let Some(id) = &self.client {
            let lock = self.app.client_lock(id).await;
            let _lock = lock.lock().await;
            let session = auth::session(&self.app, &self.headers)
                .await
                .ok_or_else(forbidden)?;
            let client = ui::owned(&self.app, &session.user, id).await?;
            if client.revoked {
                return Err(forbidden());
            }
            let socket = self.app.peers.checked(&client).await.map_err(unavailable)?;
            self.app
                .lifecycle_trace
                .route(&client, &session.user, &socket);
            rpc::call(&socket, request).await.map_err(unavailable)
        } else {
            let session = auth::session(&self.app, &self.headers)
                .await
                .ok_or_else(forbidden)?;
            let user = self
                .app
                .config
                .users
                .get(&session.user)
                .ok_or_else(forbidden)?;
            self.app
                .lifecycle_trace
                .control(&session.user, &user.control_socket);
            rpc::call(&user.control_socket, request)
                .await
                .map_err(unavailable)
        }
    }
}

impl pigeon_web::Backend for Backend {
    fn request(&self, request: Rpc) -> pigeon_web::Answer {
        let backend = Self {
            app: self.app.clone(),
            headers: self.headers.clone(),
            client: self.client.clone(),
        };
        Box::pin(async move {
            vec![backend.call(request).await.unwrap_or_else(|error| {
                let (code, id) = if error.0 == axum::http::StatusCode::FORBIDDEN {
                    ("web-forbidden", "web-forbidden")
                } else {
                    ("internal-error", "problem-internal-error")
                };
                Answer::Problem(Problem {
                    code: code.into(),
                    message: Message {
                        id: id.into(),
                        args: BTreeMap::new(),
                    },
                    hint: None,
                    details: Some(error.1),
                })
            })]
        })
    }
}

pub async fn client(
    State(app): State<Arc<App>>,
    Path(params): Path<BTreeMap<String, String>>,
    request: Request,
) -> Result<Response> {
    let session = auth::session(&app, request.headers())
        .await
        .ok_or_else(forbidden)?;
    let id = params.get("id").ok_or_else(forbidden)?;
    let client = ui::owned(&app, &session.user, id).await?;
    if client.revoked {
        return Err(forbidden());
    }
    serve(
        app,
        request,
        Some(id.clone()),
        pigeon_web::Mount {
            path: format!("/clients/{id}/peer"),
            label: format!("{} · Manage client and conflicts", client.name),
            back: format!("/clients/{id}"),
            storage_controls: false,
        },
        session.csrf,
    )
    .await
}

/// The account's personal control peer uses the same review pages too.
pub async fn account(State(app): State<Arc<App>>, request: Request) -> Result<Response> {
    let Some(session) = auth::session(&app, request.headers()).await else {
        return Ok(Redirect::to("/login").into_response());
    };
    serve(
        app,
        request,
        None,
        pigeon_web::Mount {
            path: "/peer".into(),
            label: format!("{} · Clients", session.user),
            back: "/".into(),
            storage_controls: true,
        },
        session.csrf,
    )
    .await
}

async fn serve(
    app: Arc<App>,
    mut request: Request,
    client: Option<String>,
    mount: pigeon_web::Mount,
    csrf: String,
) -> Result<Response> {
    let path = request
        .uri()
        .path()
        .strip_prefix(&mount.path)
        .ok_or_else(forbidden)?;
    if path.is_empty() {
        return Ok(Redirect::to(&format!("{}/", mount.path)).into_response());
    }
    let uri = match request.uri().query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_owned(),
    };
    *request.uri_mut() = uri.parse().map_err(unavailable)?;
    let backend = Arc::new(Backend {
        app,
        headers: request.headers().clone(),
        client,
    });
    let language = match backend
        .call(Rpc::GetSettings {
            key: Some("language".into()),
        })
        .await?
    {
        Answer::Settings { settings } => settings
            .into_iter()
            .find(|s| s.key == "language")
            .and_then(|s| pigeon_i18n::Setting::parse(&s.value)),
        _ => None,
    }
    .unwrap_or(pigeon_i18n::Setting::Auto);
    let system = pigeon_i18n::system_languages(|key| std::env::var(key).ok());
    let web = pigeon_web::Web::embedded(backend, pigeon_i18n::choose(&language, &system), mount);
    // This router is called only after the bridge's origin middleware and
    // browser authentication; its POST handlers check the same session token.
    match web.embedded_router(csrf).oneshot(request).await {
        Ok(response) => Ok(response),
        Err(error) => match error {},
    }
}
