//! Exercise real HTTP authorization and RPC identity observations across a
//! failed roster removal and bridge restart. Queued legacy portal requests
//! must reload authorization after taking the per-client operation lock.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use axum::http::StatusCode;

use crate::{
    App, auth, store,
    tests::{client, config, peer_ui, request},
};

const COOKIE: &[(&str, &str)] = &[("Cookie", "pigeon-bridge-session=browser")];

#[tokio::test]
async fn revocation_sends_the_exact_key_when_two_clients_share_a_name() {
    let temp = tempfile::tempdir().unwrap();
    let config = config(temp.path());
    let a = client("a", "alice", "app-a");
    let mut c = client("c", "alice", "app-c");
    c.name = a.name.clone();
    let mut saved = store::Store::load(&config.state_dir).unwrap();
    for client in [a.clone(), c.clone()] {
        saved.clients.insert(client.id.clone(), client);
    }
    saved.save(&config.state_dir).unwrap();
    let app = App::new(config).await.unwrap();
    let calls = peer_ui::peer(&app, None).await;
    auth::revoke(&app, &c).await.unwrap();
    let calls = calls.lock().await;
    assert!(calls.iter().any(|request| matches!(request,
        pigeon_proto::Request::RemoveDeviceId { device } if device == &c.device)));
    assert!(!calls.iter().any(|request| matches!(
        request,
        pigeon_proto::Request::Machines | pigeon_proto::Request::RemoveDevice { .. }
    )));
    let saved = store::Store::load(&app.config.state_dir).unwrap();
    assert!(!saved.clients[&a.id].revoked);
    assert!(saved.clients[&c.id].revoked);
}

#[tokio::test]
async fn identities_routes_and_revocation_survive_restart_when_roster_is_offline() {
    let temp = tempfile::tempdir().unwrap();
    let config = config(temp.path());
    let a = client("a", "alice", "app-a");
    let c = client("c", "alice", "app-c");
    let mut saved = store::Store::load(&config.state_dir).unwrap();
    for client in [a.clone(), c.clone()] {
        saved.clients.insert(client.id.clone(), client);
    }
    saved.save(&config.state_dir).unwrap();
    let app = App::new(config.clone()).await.unwrap();
    peer_ui::signed_in(&app).await;
    let a_calls = peer_ui::peer(&app, Some(a.clone())).await;
    let c_calls = peer_ui::peer(&app, Some(c.clone())).await;
    let control = peer_ui::peer(&app, None).await;
    for id in [&a.id, &c.id] {
        assert_eq!(
            request(
                app.clone(),
                "GET",
                &format!("/clients/{id}/peer/suggestions"),
                COOKIE,
                ""
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        request(app.clone(), "GET", "/peer/suggestions", COOKIE, "")
            .await
            .status(),
        StatusCode::OK
    );
    assert!(!control.lock().await.is_empty());
    // Stop the control fixture from answering roster reads by removing its
    // socket path. Local revocation must remain committed despite this error.
    std::fs::remove_file(&config.users["alice"].control_socket).unwrap();
    assert!(auth::revoke(&app, &a).await.is_err());
    assert!(store::Store::load(&config.state_dir).unwrap().clients[&a.id].revoked);
    let before = a_calls.lock().await.len();
    assert_eq!(
        request(
            app.clone(),
            "GET",
            &format!("/clients/{}/peer/suggestions", a.id),
            COOKIE,
            ""
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(a_calls.lock().await.len(), before);
    app.shutdown().await;
    drop(app);
    let app = App::new(config.clone()).await.unwrap();
    peer_ui::signed_in(&app).await;
    assert_eq!(
        request(
            app.clone(),
            "GET",
            &format!("/clients/{}/peer/suggestions", a.id),
            COOKIE,
            ""
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            app.clone(),
            "GET",
            &format!("/clients/{}/peer/suggestions", c.id),
            COOKIE,
            ""
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert!(!c_calls.lock().await.is_empty());
    assert_eq!(app.store.lock().await.clients[&c.id].device, c.device);
    assert_eq!(
        app.store.lock().await.clients[&c.id].annex_uuid,
        c.annex_uuid
    );
}

#[tokio::test]
async fn queued_legacy_portal_requests_recheck_after_revocation() {
    // Poll each real handler up to its blocked client-lock acquisition, commit
    // revoke under that lock, then resume. No sleeps or scheduler races decide
    // whether the stale pre-lock authorization path was exercised.
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    for (method, suffix, body) in [
        ("GET", "", ""),
        ("GET", "/file?path=a.txt", ""),
        ("POST", "/resolve", "csrf=alice-form&path=a.txt&choice=ours"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let config = config(temp.path());
        let a = client("a", "alice", "app-a");
        let mut saved = store::Store::load(&config.state_dir).unwrap();
        saved.clients.insert(a.id.clone(), a.clone());
        saved.save(&config.state_dir).unwrap();
        let app = App::new(config).await.unwrap();
        peer_ui::signed_in(&app).await;
        let calls = peer_ui::peer(&app, Some(a.clone())).await;
        let lock = app.client_lock(&a.id).await;
        let guard = lock.lock().await;
        let headers = if method == "POST" {
            vec![
                ("Cookie", "pigeon-bridge-session=browser"),
                ("Content-Type", "application/x-www-form-urlencoded"),
            ]
        } else {
            COOKIE.to_vec()
        };
        let path = format!("/clients/{}{suffix}", a.id);
        let mut pending = std::pin::pin!(request(app.clone(), method, &path, &headers, body));
        assert!(matches!(
            pending
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        auth::revoke_local(&app, &a.id).await.unwrap();
        drop(guard);
        let response = pending.await;
        assert!(matches!(
            response.status(),
            StatusCode::FORBIDDEN | StatusCode::OK
        ));
        // The detail page renders its revoked notice; download/resolve deny.
        if suffix.is_empty() {
            assert_eq!(response.status(), StatusCode::OK);
        } else {
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        assert!(
            calls.lock().await.is_empty(),
            "{suffix} reached a revoked peer"
        );
    }
}
