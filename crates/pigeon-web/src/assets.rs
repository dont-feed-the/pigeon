//! The web client's files (ADR 0077): built by `build.rs` from `client/`
//! and embedded in the binary, served under content-hashed names that may
//! be cached for a year, as nothing is ever fetched from elsewhere. And the
//! headers of every page: a Content-Security-Policy that allows only this
//! server's own scripts, styles, images and connections, and no inline
//! script.

use axum::extract::{Path, Request};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// One embedded file.
pub(crate) struct Asset {
    /// Its content-hashed name, under `/assets/`.
    pub name: &'static str,
    /// Its bytes.
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// The policy of every page served by `pigeond` itself; the bridge sets
/// its own, as strict, around the pages it mounts.
pub(crate) const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
img-src 'self'; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'";

/// `GET /assets/NAME`: an embedded file, immutable under its name. A host
/// that mounts the pages under a route of its own (the bridge's
/// `/clients/{id}/peer/{*path}`) adds its parameters, so `name` is looked up
/// among them rather than taken as the only one.
pub(crate) async fn serve(Path(params): Path<Vec<(String, String)>>) -> Response {
    let name = params
        .iter()
        .rev()
        .find(|(key, _)| key == "name")
        .map_or("", |(_, value)| value.as_str());
    let (asset, kind) = match name {
        n if n == SCRIPT.name => (&SCRIPT, "text/javascript; charset=utf-8"),
        n if n == STYLE.name => (&STYLE, "text/css; charset=utf-8"),
        n if n == ICON.name => (&ICON, "image/svg+xml"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, kind),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        asset.bytes,
    )
        .into_response()
}

/// Adds the policy, `nosniff` and a same-origin referrer to a response.
pub(crate) async fn headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    response
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test fixtures")]
mod tests {
    use super::*;
    use axum::{Router, body::Body, routing::get};
    use tower::ServiceExt;

    #[tokio::test]
    async fn assets_keep_working_under_a_client_prefix() {
        let app = Router::new().nest(
            "/clients/{client}/peer",
            Router::new().route("/assets/{name}", get(serve)),
        );
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/clients/selected/peer/assets/{}", SCRIPT.name))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .as_ref(),
            SCRIPT.bytes
        );
        let missing = app
            .oneshot(
                Request::builder()
                    .uri("/clients/selected/peer/assets/missing.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }
}
