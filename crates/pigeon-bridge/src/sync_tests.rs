//! Real authenticated DAV handlers against deterministic daemon socket replies.
//! The fixture exposes uncertain outcomes without inventing successful replies.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::VecDeque;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pigeon_proto::bridge::{File, Operation, Reply, Snapshot};
use pigeon_proto::{Envelope, PROTOCOL_VERSION, Request as Rpc, Response, encode};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tower::ServiceExt;

use crate::{App, router, tests};

fn snapshot(revision: &str, path: &str, tag: &str, diverging: bool) -> Option<Reply> {
    Some(Reply::Snapshot(Snapshot {
        revision: revision.into(),
        files: vec![File {
            path: path.into(),
            etag: tag.into(),
            size: 5,
            diverging,
        }],
        next: None,
    }))
}

fn applied(revision: &str, suggested: usize) -> Option<Reply> {
    Some(Reply::Applied {
        revision: revision.into(),
        suggested,
    })
}

async fn fixture(
    app: &App,
    replies: Vec<(&'static str, Option<Reply>)>,
) -> tokio::task::JoinHandle<()> {
    let client = tests::client("a", "alice", "app-a");
    let listener = UnixListener::bind(app.peers.socket(&client.id)).unwrap();
    let root = app.peers.home(&client.id).join("files");
    tokio::spawn(async move {
        let mut replies: VecDeque<_> = replies.into();
        while !replies.is_empty() {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            let mut line = String::new();
            if stream.read_line(&mut line).await.unwrap() == 0 {
                continue; // Peers::ensure probes connectivity without an RPC.
            }
            let hello: Envelope<Rpc> = serde_json::from_str(&line).unwrap();
            assert_eq!(
                hello.body,
                Rpc::Hello {
                    version: PROTOCOL_VERSION
                }
            );
            stream
                .get_mut()
                .write_all(
                    &encode(
                        1,
                        &Response::Hello {
                            version: PROTOCOL_VERSION,
                        },
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            line.clear();
            stream.read_line(&mut line).await.unwrap();
            let request: Envelope<Rpc> = serde_json::from_str(&line).unwrap();
            let Rpc::Bridge { operation } = request.body else {
                panic!("not a bridge RPC")
            };
            let reply = match operation {
                Operation::Info => Some(Reply::Info {
                    group: "group".into(),
                    person: "alice-person".into(),
                    device: client.device.clone(),
                    annex_uuid: client.annex_uuid.clone(),
                    root: root.display().to_string(),
                    addresses: vec![],
                    shared: true,
                }),
                Operation::Snapshot { cursor, .. } => {
                    let (kind, reply) = replies.pop_front().unwrap();
                    assert_eq!(kind, "snapshot");
                    if let (Some(cursor), Some(Reply::Snapshot(snapshot))) = (&cursor, &reply) {
                        assert_eq!(cursor.revision, snapshot.revision);
                    }
                    reply
                }
                Operation::Apply {
                    expected, changes, ..
                } => {
                    let (kind, reply) = replies.pop_front().unwrap();
                    assert_eq!(kind, "apply");
                    assert_eq!(expected.is_some(), changes.len() == 1);
                    reply
                }
                Operation::Read { destination, .. } => {
                    std::fs::write(destination, b"hello").unwrap();
                    Some(Reply::Read)
                }
                _ => panic!("unexpected operation"),
            };
            if let Some(result) = reply {
                stream
                    .get_mut()
                    .write_all(&encode(2, &Response::Bridge { result }).unwrap())
                    .await
                    .unwrap();
            } // None deliberately closes the connection after receiving Apply.
        }
    })
}

async fn request(
    app: Arc<App>,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("Host", "127.0.0.1:8787")
        .header("Authorization", "Basic YWxpY2U6YXBwLWE=");
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    router(app)
        .oneshot(
            request
                .body(Body::from(if method == "MKCOL" { "" } else { "hello" }))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn successful_suggestion_returns_its_pinned_overlay_validator() {
    let (_state, app) = tests::app().await;
    let server = fixture(
        &app,
        vec![
            ("snapshot", snapshot("base", "a.txt", "old", false)),
            ("apply", applied("saved", 1)),
            ("snapshot", snapshot("saved", "a.txt", "new", true)),
        ],
    )
    .await;
    let response = request(app, "PUT", "/remote.php/dav/files/alice/a.txt", &[]).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(response.headers()["ETag"], "\"new\"");
    assert_eq!(response.headers()["X-Pigeon-Suggested"], "1");
    server.await.unwrap();
}

#[tokio::test]
async fn saved_then_refresh_race_requires_reconciliation_before_retry() {
    let (_state, app) = tests::app().await;
    let server = fixture(
        &app,
        vec![
            ("snapshot", snapshot("base", "a.txt", "old", false)),
            ("apply", applied("saved", 0)),
            (
                "snapshot",
                Some(Reply::Failure {
                    status: 412,
                    message: "Native edit raced refresh".into(),
                }),
            ),
            ("snapshot", snapshot("raced", "a.txt", "new", false)),
            ("snapshot", snapshot("raced", "a.txt", "new", false)),
            ("apply", applied("retry", 0)),
            ("snapshot", snapshot("retry", "a.txt", "retried", false)),
        ],
    )
    .await;
    let url = "/remote.php/dav/files/alice/a.txt";
    assert_eq!(
        request(app.clone(), "PUT", url, &[("If-Match", "\"old\"")])
            .await
            .status(),
        StatusCode::PRECONDITION_FAILED
    );
    // The first save may have committed: repeating its stale validator cannot
    // silently overwrite the now-visible file.
    assert_eq!(
        request(app.clone(), "PUT", url, &[("If-Match", "\"old\"")])
            .await
            .status(),
        StatusCode::PRECONDITION_FAILED
    );
    let response = request(app, "PUT", url, &[("If-Match", "\"new\"")]).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(response.headers()["ETag"], "\"retried\"");
    server.await.unwrap();
}

#[tokio::test]
async fn lost_apply_reply_is_unknown_and_a_later_poll_reconciles() {
    let (_state, app) = tests::app().await;
    let server = fixture(
        &app,
        vec![
            ("snapshot", snapshot("base", "a.txt", "old", false)),
            ("apply", None),
            ("snapshot", snapshot("committed", "a.txt", "new", false)),
            ("snapshot", snapshot("committed", "a.txt", "new", false)),
        ],
    )
    .await;
    let url = "/remote.php/dav/files/alice/a.txt";
    assert_eq!(
        request(app.clone(), "PUT", url, &[]).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let response = request(app.clone(), "HEAD", url, &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["ETag"], "\"new\"");
    let cached = request(app, "HEAD", url, &[("If-None-Match", "\"new\"")]).await;
    assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(cached.headers()["ETag"], "\"new\"");
    server.await.unwrap();
}

#[tokio::test]
async fn move_keeps_batch_condition_and_delete_does_not_invent_a_refresh() {
    let (_state, app) = tests::app().await;
    let server = fixture(
        &app,
        vec![
            ("snapshot", snapshot("base", "a.txt", "old", false)),
            ("apply", applied("moved", 0)),
            ("snapshot", snapshot("moved", "b.txt", "old", false)),
            ("snapshot", snapshot("moved", "b.txt", "old", false)),
            ("apply", applied("deleted", 0)),
        ],
    )
    .await;
    let response = request(
        app.clone(),
        "MOVE",
        "/remote.php/dav/files/alice/a.txt",
        &[("Destination", "/remote.php/dav/files/alice/b.txt")],
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["ETag"], "\"old\"");
    assert_eq!(
        request(app, "DELETE", "/remote.php/dav/files/alice/b.txt", &[])
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    server.await.unwrap();
}

#[tokio::test]
async fn metadata_only_directory_move_returns_its_directory_validator() {
    let (_state, app) = tests::app().await;
    let client = tests::client("a", "alice", "app-a");
    crate::index::mkdir(&app, &client, "empty").await.unwrap();
    let empty = || {
        Some(Reply::Snapshot(Snapshot {
            revision: "base".into(),
            files: vec![],
            next: None,
        }))
    };
    let server = fixture(&app, vec![("snapshot", empty()), ("snapshot", empty())]).await;
    let response = request(
        app,
        "MOVE",
        "/remote.php/dav/files/alice/empty",
        &[("Destination", "/remote.php/dav/files/alice/moved")],
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(response.headers().contains_key("ETag"));
    server.await.unwrap();
}

#[tokio::test]
async fn staged_chunks_need_no_native_save_but_finalization_does() {
    let (_state, app) = tests::app().await;
    let server = fixture(
        &app,
        vec![
            ("snapshot", snapshot("base", "a.txt", "old", false)),
            ("apply", applied("saved", 0)),
            ("snapshot", snapshot("saved", "a.txt", "new", false)),
        ],
    )
    .await;
    let upload = "/remote.php/dav/uploads/alice/upload";
    assert_eq!(
        request(app.clone(), "MKCOL", upload, &[]).await.status(),
        StatusCode::CREATED
    );
    assert_eq!(
        request(app.clone(), "PUT", &format!("{upload}/00001"), &[])
            .await
            .status(),
        StatusCode::CREATED
    );
    let response = request(
        app,
        "MOVE",
        &format!("{upload}/.file"),
        &[
            ("Destination", "/remote.php/dav/files/alice/a.txt"),
            ("OC-Total-Length", "5"),
        ],
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(response.headers()["ETag"], "\"new\"");
    server.await.unwrap();
}
