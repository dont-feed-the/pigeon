//! A Nextcloud file gateway and authenticated user portal. Each app credential
//! routes to exactly one isolated Pigeon peer, never to a shared service key.
//! The portal uses the user's existing control peer for group decisions.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use tokio::sync::Mutex;

mod auth;
mod catalogue;
pub mod config;
mod dav;
mod dav_xml;
mod index;
mod peers;
mod rpc;
mod store;
mod ui;
mod uploads;

/// Shared bridge state; daemon identities and credentials persist on disk.
pub struct App {
    config: config::Config,
    peers: peers::Peers,
    store: Mutex<store::Store>,
    sessions: Mutex<BTreeMap<String, auth::Session>>,
    flows: Mutex<BTreeMap<String, auth::Flow>>,
    client_locks: Mutex<BTreeMap<String, Arc<Mutex<()>>>>,
    grants: Mutex<()>,
    login_attempts: Mutex<BTreeMap<String, (u64, u32)>>,
    _state_lock: std::fs::File,
}

#[derive(Debug)]
struct Error(StatusCode, String);
type Result<T> = std::result::Result<T, Error>;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let mut response = (self.0, self.1).into_response();
        if self.0 == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                axum::http::HeaderValue::from_static("Basic realm=\"Pigeon Bridge\""),
            );
        }
        response
    }
}

fn unavailable(message: impl ToString) -> Error {
    Error(StatusCode::SERVICE_UNAVAILABLE, message.to_string())
}
fn bad(message: impl ToString) -> Error {
    Error(StatusCode::BAD_REQUEST, message.to_string())
}
fn forbidden() -> Error {
    Error(StatusCode::FORBIDDEN, "Access denied".into())
}

impl App {
    /// Opens private state without issuing invitations or starting client peers.
    pub async fn new(config: config::Config) -> std::result::Result<Arc<Self>, String> {
        config.validate()?;
        let store = store::Store::load(&config.state_dir)?;
        use std::os::unix::fs::OpenOptionsExt;
        let state_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(config.state_dir.join("bridge.lock"))
            .map_err(|e| e.to_string())?;
        state_lock
            .try_lock()
            .map_err(|_| "Another bridge is using this state_dir".to_owned())?;
        for sub in ["run", "peers", "staging", "uploads"] {
            std::fs::create_dir_all(config.state_dir.join(sub)).map_err(|e| e.to_string())?;
        }
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        let content = config.content_dir();
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&content)
            .map_err(|e| e.to_string())?;
        let meta = content.symlink_metadata().map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
            return Err("content_dir must be a private directory (0700)".into());
        }
        if meta.dev()
            != config
                .state_dir
                .join("peers")
                .metadata()
                .map_err(|e| e.to_string())?
                .dev()
        {
            return Err("content_dir and client peers must be on the same filesystem".into());
        }
        Ok(Arc::new(Self {
            peers: peers::Peers::new(config.clone()),
            config,
            store: Mutex::new(store),
            sessions: Mutex::default(),
            flows: Mutex::default(),
            client_locks: Mutex::default(),
            grants: Mutex::new(()),
            login_attempts: Mutex::default(),
            _state_lock: state_lock,
        }))
    }

    async fn client_lock(&self, id: &str) -> Arc<Mutex<()>> {
        self.client_locks
            .lock()
            .await
            .entry(id.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Stops daemon processes started by this bridge instance.
    pub async fn shutdown(&self) {
        self.peers.shutdown().await;
    }

    /// Restores active peer processes and shares same-group direct addresses.
    /// Returns the number of unavailable peers without changing their identities.
    pub async fn resume_peers(&self) -> usize {
        let clients = self
            .store
            .lock()
            .await
            .clients
            .values()
            .filter(|c| !c.revoked && self.config.users.contains_key(&c.user))
            .cloned()
            .collect();
        self.peers.mesh(clients).await
    }
}

/// All Nextcloud API and authenticated portal routes, with origin checks.
pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/", get(ui::home))
        .route("/login", get(ui::login).post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/suggestions", get(ui::suggestions))
        .route("/decide", post(ui::decide))
        .route("/clients/{id}", get(ui::client))
        .route("/clients/{id}/file", get(ui::file))
        .route("/clients/{id}/resolve", post(ui::resolve))
        .route("/clients/{id}/revoke", post(ui::revoke))
        .route("/index.php/login/v2", post(auth::begin))
        .route("/login/v2/poll", post(auth::poll))
        .route("/login/v2/flow/{token}", get(ui::grant).post(auth::grant))
        .route("/status.php", get(auth::status))
        .route("/ocs/v1.php/cloud/capabilities", get(auth::capabilities))
        .route("/ocs/v2.php/cloud/capabilities", get(auth::capabilities))
        .route("/ocs/v1.php/cloud/user", get(auth::user))
        .route("/ocs/v2.php/cloud/user", get(auth::user))
        .route(
            "/ocs/v2.php/core/apppassword",
            axum::routing::delete(auth::delete_password),
        )
        .route(
            "/ocs/v2.php/core/getapppassword",
            get(auth::existing_password),
        )
        .route("/remote.php/dav", any(dav::handle))
        .route("/remote.php/dav/", any(dav::handle))
        .route("/remote.php/dav/{*path}", any(dav::handle))
        .route("/remote.php/webdav", any(dav::handle))
        .route("/remote.php/webdav/", any(dav::handle))
        .route("/remote.php/webdav/{*path}", any(dav::handle))
        .layer(middleware::from_fn_with_state(app.clone(), origin))
        .with_state(app)
}

async fn origin(State(app): State<Arc<App>>, request: Request, next: Next) -> Response {
    let expected = app
        .config
        .public_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok());
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok());
    if host != Some(expected) || origin.is_some_and(|o| o != app.config.public_url) {
        return forbidden().into_response();
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        // `no-referrer` turns a browser form navigation's Origin into `null`,
        // which would fail our origin guard even for our own forms.
        axum::http::HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    headers.insert(header::CONTENT_SECURITY_POLICY, axum::http::HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'"));
    response
}

#[cfg(test)]
mod tests;
