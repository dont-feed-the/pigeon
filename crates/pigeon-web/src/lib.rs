//! pigeon's local web interface (ADR 0021, 10 §10.3): pages rendered on
//! the server from the requests of 10 §10.1, in the person's language
//! (ADR 0028), refreshed by server-sent events, that work without
//! JavaScript. It writes nothing itself: every page asks [`Backend`].
//!
//! Who may see them: the server listens on 127.0.0.1 only. `pigeon open`
//! asks `pigeond` for a link holding a single-use token of 256 random bits
//! ([`Web::login_token`]); `/login` exchanges it for a session cookie
//! (`HttpOnly`, `SameSite=Strict`). Every other request needs that cookie,
//! and a `Host`, and an `Origin` if it has one, naming this server: so
//! neither another site (CSRF) nor a name that resolves to 127.0.0.1 (DNS
//! rebinding) reaches the pages.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Query, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use pigeon_i18n::Catalogue;

mod actions;
mod erase;
mod files;
mod folder_rules;
mod health;
mod held;
mod join;
mod leave;
mod machines;
mod pages;
mod recovery;
mod settings;
mod space;
mod view;

/// What a backend answers a request with: progress, then one result.
pub type Answer = Pin<Box<dyn Future<Output = Vec<pigeon_proto::Response>> + Send>>;

/// The loops, which answer the requests of 10 §10.1.
pub trait Backend: Send + Sync + 'static {
    /// Answers `request`.
    fn request(&self, request: pigeon_proto::Request) -> Answer;
}

/// The web interface of one `pigeond`.
pub struct Web {
    backend: Arc<dyn Backend>,
    lang: Mutex<pigeon_i18n::LanguageIdentifier>,
    port: u16,
    tokens: Mutex<HashSet<String>>,
    /// Each session, with its form token.
    sessions: Mutex<HashMap<String, String>>,
}

/// Shows the port and language only: tokens and sessions are secrets.
impl std::fmt::Debug for Web {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Web")
            .field("lang", &self.lang)
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

/// Seconds since the epoch.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The cookie that holds a session.
const COOKIE: &str = "pigeon-session";

/// 256 random bits, hex.
fn random_hex() -> String {
    let mut b = [0u8; 32];
    if getrandom::fill(&mut b).is_err() {
        // No randomness: a token nobody can guess cannot be made, so none
        // is; an empty one never matches.
        return String::new();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Web {
    /// The interface served on 127.0.0.1:`port`, in `lang`.
    pub fn new(
        backend: Arc<dyn Backend>,
        lang: pigeon_i18n::LanguageIdentifier,
        port: u16,
    ) -> Arc<Web> {
        Arc::new(Web {
            backend,
            lang: Mutex::new(lang),
            port,
            tokens: Mutex::default(),
            sessions: Mutex::default(),
        })
    }

    /// A new single-use login token.
    pub fn login_token(&self) -> String {
        let t = random_hex();
        if !t.is_empty() {
            self.tokens
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(t.clone());
        }
        t
    }

    /// The link `pigeon open` opens: the login with a new token.
    pub fn login_url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/login?token={}",
            self.port,
            self.login_token()
        )
    }

    /// Shows the pages in `lang` from now on.
    pub fn set_lang(&self, lang: pigeon_i18n::LanguageIdentifier) {
        *self.lang.lock().unwrap_or_else(|p| p.into_inner()) = lang;
    }

    fn renderer(&self) -> pigeon_i18n::render::Renderer {
        let lang = self.lang.lock().unwrap_or_else(|p| p.into_inner()).clone();
        pigeon_i18n::render::Renderer::new(Catalogue::new(&lang, false), now())
    }

    async fn ask(&self, request: pigeon_proto::Request) -> Vec<pigeon_proto::Response> {
        self.backend.request(request).await
    }

    /// The routes, behind the checks of the module's header.
    pub fn router(self: &Arc<Web>) -> Router {
        Router::new()
            .route("/", get(pages::home))
            .route("/join", get(join::setup).post(join::join))
            .route("/create", post(join::create))
            .route("/machines", get(machines::machines))
            .route("/machines/remove", post(machines::remove))
            .route("/machines/recovery-sheet", post(recovery::sheet))
            .route("/machines/invite", post(machines::invite))
            .route("/machines/cancel-invitation", post(machines::cancel))
            .route("/suggestions", get(pages::suggestions))
            .route("/events", get(pages::events))
            .route("/files", get(files::files))
            .route("/space", get(space::page))
            .route("/erase", get(erase::view).post(erase::submit))
            .route("/folder", get(files::folder_page))
            .route(
                "/folder-rule",
                get(folder_rules::view).post(folder_rules::set),
            )
            .route("/deleted", get(files::deleted_page))
            .route("/file", get(files::file_page))
            .route("/files/restore", post(actions::restore))
            .route("/files/keep", post(actions::keep))
            .route("/files/free", post(actions::free))
            .route("/settings", get(settings::settings).post(settings::save))
            .route("/doctor", get(settings::doctor))
            .route("/settings/diagnostics", post(settings::diagnostics))
            .route("/pause", post(settings::pause))
            .route("/resume", post(settings::resume))
            .route("/held/share", post(held::share))
            .route("/held/bring-back", post(held::bring_back))
            .route("/held/rename", post(held::rename))
            .route("/leave", post(leave::leave))
            .route("/files/view", post(view::view))
            .route("/suggestions/view", post(view::view))
            .route("/suggestions/accept", post(actions::accept))
            .route("/suggestions/decline", post(actions::decline))
            .route_layer(middleware::from_fn_with_state(self.clone(), session))
            .route("/login", get(login))
            .layer(middleware::from_fn_with_state(self.clone(), same_origin))
            .with_state(self.clone())
    }

    fn hosts(&self) -> [String; 2] {
        [
            format!("127.0.0.1:{}", self.port),
            format!("localhost:{}", self.port),
        ]
    }
}

/// Refuses a request whose `Host`, or `Origin` if any, is not this server.
async fn same_origin(State(web): State<Arc<Web>>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let hosts = web.hosts();
    let host_ok = header_str(headers, header::HOST).is_some_and(|h| hosts.contains(&h));
    let origin_ok = header_str(headers, header::ORIGIN)
        .is_none_or(|o| hosts.iter().any(|h| o == format!("http://{h}")));
    if host_ok && origin_ok {
        next.run(request).await
    } else {
        (StatusCode::FORBIDDEN, pages::refused(&web, "web-forbidden")).into_response()
    }
}

/// A session's form token, which every `POST` must carry (10 §10.3).
#[derive(Clone, Debug)]
struct FormToken(String);

/// Refuses a request without a session cookie of this server, and hands
/// the session's form token to the page.
async fn session(State(web): State<Arc<Web>>, mut request: Request, next: Next) -> Response {
    let form = cookie(request.headers()).and_then(|c| {
        web.sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&c)
            .cloned()
    });
    if let Some(form) = form {
        request.extensions_mut().insert(FormToken(form));
        next.run(request).await
    } else {
        (StatusCode::FORBIDDEN, pages::refused(&web, "web-forbidden")).into_response()
    }
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers.get(name)?.to_str().ok().map(str::to_owned)
}

/// The session cookie's value, if the request has one.
fn cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v.to_owned())
}

/// The query of `/login`.
#[derive(serde::Deserialize)]
struct Login {
    token: String,
}

/// `/login?token=…`: a known token, used once, becomes a session.
async fn login(State(web): State<Arc<Web>>, Query(q): Query<Login>) -> Response {
    let used = web
        .tokens
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&q.token);
    if !used || q.token.is_empty() {
        return (
            StatusCode::FORBIDDEN,
            pages::refused(&web, "web-login-used"),
        )
            .into_response();
    }
    let id = random_hex();
    web.sessions
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(id.clone(), random_hex());
    let cookie = format!("{COOKIE}={id}; HttpOnly; SameSite=Strict; Path=/");
    ([(header::SET_COOKIE, cookie)], Redirect::to("/")).into_response()
}

#[cfg(test)]
mod tests;
