//! The bridge's isolation and protocol boundaries without external daemons.
//! `scripts/check-nextcloud-bridge` additionally exercises real peer processes,
//! signed changes, suggestions, restarts, chunk uploads, and portal actions.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::BTreeMap;
use std::sync::Arc;

use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use pigeon_proto::bridge::File;
use serde_json::Value;
use tower::ServiceExt;

use crate::{
    App, auth,
    config::{Config, User},
    dav, dav_xml, index, router,
    store::{self, Client},
};

#[path = "tests/peer_ui.rs"]
mod peer_ui;

fn config(root: &std::path::Path) -> Config {
    let salt = SaltString::encode_b64(b"a-test-salt-only").unwrap();
    let password_hash = Argon2::default()
        .hash_password(b"portal-password", &salt)
        .unwrap()
        .to_string();
    Config {
        listen: "127.0.0.1:8787".parse().unwrap(),
        public_url: "http://127.0.0.1:8787".into(),
        state_dir: root.join("state"),
        content_dir: None,
        pigeond: "/not-started-in-unit-tests/pigeond".into(),
        max_upload_bytes: 1024,
        max_clients_per_user: 4,
        peer_network: "off".into(),
        users: ["alice", "bob"]
            .into_iter()
            .map(|u| {
                (
                    u.to_owned(),
                    User {
                        password_hash: password_hash.clone(),
                        control_socket: root.join(format!("{u}.sock")),
                        storage_socket: None,
                    },
                )
            })
            .collect(),
    }
}

pub(crate) fn client(id: &str, user: &str, password: &str) -> Client {
    Client {
        id: id.repeat(32),
        user: user.into(),
        name: format!("{user}'s client"),
        token_hash: store::hash(password.as_bytes()),
        device: id.repeat(64),
        annex_uuid: format!("annex-{id}"),
        revoked: false,
        metadata: BTreeMap::new(),
    }
}

pub(crate) async fn app() -> (tempfile::TempDir, Arc<App>) {
    let temp = tempfile::tempdir().unwrap();
    let app = App::new(config(temp.path())).await.unwrap();
    {
        let mut store = app.store.lock().await;
        for c in [client("a", "alice", "app-a"), client("b", "bob", "app-b")] {
            store.clients.insert(c.id.clone(), c);
        }
        store.save(&app.config.state_dir).unwrap();
    }
    (temp, app)
}

async fn request(
    app: Arc<App>,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("Host", "127.0.0.1:8787");
    for (k, v) in headers {
        request = request.header(*k, *v);
    }
    router(app)
        .oneshot(request.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap()
}

async fn json(response: axum::response::Response) -> Value {
    serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn app_passwords_route_to_one_peer_and_never_authenticate_another_person() {
    let (_tmp, app) = app().await;
    let mut headers = axum::http::HeaderMap::new();
    for (credentials, expected) in [
        ("alice:app-a", Some("a".repeat(32))),
        ("bob:app-a", None),
        ("alice:portal-password", None),
    ] {
        headers.insert(
            "Authorization",
            format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(credentials)
            )
            .parse()
            .unwrap(),
        );
        assert_eq!(
            auth::client(&app, &headers).await.ok().map(|c| c.id),
            expected
        );
    }
    headers.insert(
        "Authorization",
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("alice:app-a")
        )
        .parse()
        .unwrap(),
    );
    app.store
        .lock()
        .await
        .clients
        .get_mut(&"a".repeat(32))
        .unwrap()
        .revoked = true;
    assert!(auth::client(&app, &headers).await.is_err());
}

#[tokio::test]
async fn host_origin_csrf_and_peer_ownership_are_checked_before_daemon_access() {
    let (_tmp, app) = app().await;
    let response = router(app.clone())
        .oneshot(
            Request::builder()
                .uri("/status.php")
                .header("Host", "attacker.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        request(
            app.clone(),
            "GET",
            "/status.php",
            &[("Origin", "https://attacker.example")],
            ""
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    app.sessions.lock().await.insert(
        "cookie".into(),
        auth::Session {
            user: "alice".into(),
            csrf: "csrf".into(),
            expires: store::now() + 100,
        },
    );
    let cookie = [("Cookie", "pigeon-bridge-session=cookie")];
    assert_eq!(
        request(
            app.clone(),
            "GET",
            &format!("/clients/{}", "b".repeat(32)),
            &cookie,
            ""
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            app,
            "POST",
            "/decide",
            &[
                ("Cookie", "pigeon-bridge-session=cookie"),
                ("Content-Type", "application/x-www-form-urlencoded")
            ],
            "csrf=wrong&decision=accept&proposal=123"
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn login_poll_is_independent_secret_and_delivers_credentials_only_once() {
    let (_tmp, app) = app().await;
    let a = json(request(app.clone(), "POST", "/index.php/login/v2", &[], "").await).await;
    let b = json(request(app.clone(), "POST", "/index.php/login/v2", &[], "").await).await;
    assert_ne!(a["poll"]["token"], b["poll"]["token"]);
    let token = a["login"].as_str().unwrap().rsplit('/').next().unwrap();
    let headers = [("Content-Type", "application/x-www-form-urlencoded")];
    let body = format!("token={}", a["poll"]["token"].as_str().unwrap());
    assert_eq!(
        request(app.clone(), "POST", "/login/v2/poll", &headers, &body)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            app.clone(),
            "POST",
            &format!("/login/v2/flow/{token}"),
            &headers,
            "csrf=no&name=forged"
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    app.flows.lock().await.get_mut(token).unwrap().credentials =
        Some(("alice".into(), "random-app-secret".into()));
    let reply = json(request(app.clone(), "POST", "/login/v2/poll", &headers, &body).await).await;
    assert_eq!(reply["loginName"], "alice");
    assert_eq!(reply["appPassword"], "random-app-secret");
    assert_eq!(
        request(app, "POST", "/login/v2/poll", &headers, &body)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn deep_changes_reach_root_etag_and_file_ids_and_times_survive_restart_and_rename() {
    let (temp, app) = app().await;
    let client = client("a", "alice", "app-a");
    let snapshot = |path: &str, etag: &str| crate::catalogue::Catalogue {
        revision: etag.into(),
        files: [(
            path.to_owned(),
            File {
                path: path.into(),
                etag: etag.into(),
                size: 5,
                diverging: false,
            },
        )]
        .into_iter()
        .collect(),
    };
    let first = index::build(&app, &client, snapshot("a/b/file", "first"))
        .await
        .unwrap();
    let same = index::build(&app, &client, snapshot("a/b/file", "first"))
        .await
        .unwrap();
    assert_eq!(first.nodes[""].etag, same.nodes[""].etag);
    assert_eq!(
        first.nodes["a/b/file"].modified,
        same.nodes["a/b/file"].modified
    );
    let changed = index::build(&app, &client, snapshot("a/b/file", "second"))
        .await
        .unwrap();
    assert_ne!(first.nodes[""].etag, changed.nodes[""].etag);
    assert_ne!(first.nodes["a"].etag, changed.nodes["a"].etag);
    assert_eq!(first.nodes["a/b/file"].id, changed.nodes["a/b/file"].id);
    let renamed = index::build(&app, &client, snapshot("a/b/renamed", "second"))
        .await
        .unwrap();
    assert_eq!(
        changed.nodes["a/b/file"].id,
        renamed.nodes["a/b/renamed"].id
    );
    drop(app);
    let restarted = App::new(config(temp.path())).await.unwrap();
    let reloaded = index::build(&restarted, &client, snapshot("a/b/renamed", "second"))
        .await
        .unwrap();
    assert_eq!(
        renamed.nodes["a/b/renamed"].id,
        reloaded.nodes["a/b/renamed"].id
    );
    assert_eq!(
        renamed.nodes["a/b/renamed"].modified,
        reloaded.nodes["a/b/renamed"].modified
    );
    assert_eq!(
        restarted.store.lock().await.clients[&client.id].device,
        client.device
    );
}

#[test]
fn dav_paths_and_destinations_cannot_cross_users_or_reach_internal_paths() {
    for path in [
        "/remote.php/dav/files/bob/a",
        "/remote.php/dav/files/alice/%2e%2e/bob/a",
        "/remote.php/dav/files/alice/.git/config",
        "/remote.php/dav/files/alice/a%5cb",
        "/remote.php/dav/uploads/alice/../chunk",
    ] {
        assert!(dav::resource(path, "alice").is_err(), "{path}");
    }
    assert!(
        matches!(dav::resource("/remote.php/dav/files/alice/Photo%20album/a.jpg","alice").unwrap(),dav::Resource::Files(path) if path == "Photo album/a.jpg")
    );
    assert!(matches!(
        dav::resource("/remote.php/dav/uploads/alice/session/.file", "alice").unwrap(),
        dav::Resource::Upload { .. }
    ));
}

#[tokio::test]
async fn desktop_move_destinations_accept_origin_relative_paths_with_the_same_boundaries() {
    let (_tmp, app) = app().await;
    for destination in [
        "http://127.0.0.1:8787/remote.php/dav/files/alice/Photo%20album/a.jpg",
        "/remote.php/dav/files/alice/Photo%20album/a.jpg",
    ] {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("Destination", destination.parse().unwrap());
        assert_eq!(
            dav::destination(&app, "alice", &headers).unwrap(),
            "Photo album/a.jpg"
        );
    }
    for destination in [
        "//attacker.example/remote.php/dav/files/alice/a",
        "/\\attacker.example/remote.php/dav/files/alice/a",
        "http://attacker.example/remote.php/dav/files/alice/a",
        "/remote.php/dav/files/bob/a",
        "/remote.php/dav/files/alice/.git/config",
        "/remote.php/dav/files/alice/a?query=1",
        "/remote.php/dav/files/alice/a#fragment",
        "relative/file",
    ] {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("Destination", destination.parse().unwrap());
        assert!(
            dav::destination(&app, "alice", &headers).is_err(),
            "{destination}"
        );
    }
}

#[test]
fn range_and_namespaced_properties_handle_edges_without_claiming_unknown_features() {
    assert_eq!(dav::range(Some("bytes=2-4"), 10).unwrap(), (2, 3, true));
    assert_eq!(dav::range(Some("bytes=-4"), 10).unwrap(), (6, 4, true));
    assert!(dav::range(Some("bytes=10-"), 10).is_err());
    assert!(dav::range(Some("bytes=0-2,4-6"), 10).is_err());
    let body = br#"<d:propfind xmlns:d="DAV:" xmlns:x="urn:other"><d:prop><d:getetag/><x:unknown/></d:prop></d:propfind>"#;
    assert_eq!(
        dav_xml::properties(body).unwrap(),
        vec![
            ("DAV:".into(), "getetag".into()),
            ("urn:other".into(), "unknown".into())
        ]
    );
    assert!(dav_xml::properties(b"<!DOCTYPE test><propfind/>").is_err());
    assert_eq!(
        dav_xml::href("alice", "a & b/c.txt", false),
        "/remote.php/dav/files/alice/a%20%26%20b/c%2Etxt"
    );
}

#[tokio::test]
async fn corrupt_or_newer_persistent_state_is_never_replaced_with_new_identities() {
    let temp = tempfile::tempdir().unwrap();
    let config = config(temp.path());
    std::fs::create_dir_all(&config.state_dir).unwrap();
    std::fs::write(
        config.state_dir.join("clients.json"),
        r#"{"format":999,"next_file_id":1,"clients":{}}"#,
    )
    .unwrap();
    assert!(App::new(config).await.is_err());
}

#[tokio::test]
async fn resume_validators_never_mix_bytes_from_different_file_versions() {
    let (_temp, app) = app().await;
    let client = client("a", "alice", "app-a");
    let node = index::Node {
        path: "a.txt".into(),
        directory: false,
        etag: "current".into(),
        id: 1,
        modified: 1,
        size: 10,
        diverging: false,
    };
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("Range", "bytes=5-".parse().unwrap());
    headers.insert("If-Range", "\"obsolete\"".parse().unwrap());
    let response = dav::download(&app, &client, &node, &headers, true)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["Content-Length"], "10");
    headers.insert("If-Range", "\"current\"".parse().unwrap());
    let response = dav::download(&app, &client, &node, &headers, true)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["Content-Range"], "bytes 5-9/10");
    headers.insert("Range", "bytes=20-".parse().unwrap());
    let response = dav::download(&app, &client, &node, &headers, true)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(response.headers()["Content-Range"], "bytes */10");
    headers.insert("If-None-Match", "\"old\", W/\"current\"".parse().unwrap());
    assert_eq!(
        dav::download(&app, &client, &node, &headers, true)
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_MODIFIED
    );
}
