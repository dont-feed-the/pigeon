//! A paper key requires the real session form, stays out of caches, and
//! escapes names while embedding only QR geometry (09 §9.9).

use std::sync::Mutex;

use super::*;

#[derive(Default)]
struct Paper(Mutex<usize>);

impl Backend for Paper {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        let answer = match request {
            pigeon_proto::Request::People => vec![Response::People {
                people: vec![pigeon_proto::api::PersonEntry {
                    name: "Ana".into(),
                    admin: true,
                    this_person: true,
                    recovery_recommended: true,
                    recovery_refresh_recommended: false,
                    machines: vec!["laptop".into()],
                }],
            }],
            pigeon_proto::Request::RecoverySheet => {
                *self.0.lock().unwrap() += 1;
                let raw = serde_like_sheet();
                vec![said("recovery-sheet-text", "sheet", raw)]
            }
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

fn serde_like_sheet() -> String {
    // A renderer needs only these public labels; no secret fixture key.
    let json = br#"{"group":"genesis","name":"Family <script>bad()</script>","person":"ana","custodians":[]}"#;
    format!(
        "pgr1-{}-checksum",
        data_encoding::BASE32_NOPAD
            .encode(json)
            .to_ascii_lowercase()
    )
}

#[tokio::test]
async fn preparing_a_sheet_checks_csrf_escapes_names_and_disables_caching() {
    let backend = Arc::new(Paper::default());
    let w = Web::new(backend.clone(), "fr".parse().unwrap(), 4321);
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let people = body(
        app.clone()
            .oneshot(get("/machines", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(people.contains("Préparer une fiche de récupération"));
    assert!(people.contains("n’a qu’un seul ordinateur"));
    let token = people
        .split("name=\"form\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let denied = app
        .clone()
        .oneshot(post("/machines/recovery-sheet", &cookie, "form=wrong"))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(*backend.0.lock().unwrap(), 0);
    let result = app
        .oneshot(post(
            "/machines/recovery-sheet",
            &cookie,
            &format!("form={token}"),
        ))
        .await
        .unwrap();
    assert_eq!(result.headers()[header::CACHE_CONTROL], "no-store");
    let printed = body(result).await;
    assert_eq!(*backend.0.lock().unwrap(), 1);
    assert!(printed.contains("Groupe : Family &lt;script&gt;bad()&lt;/script&gt;"));
    assert!(!printed.contains("<script>"));
    assert!(printed.contains("<svg") && printed.contains(&serde_like_sheet()));
    assert!(printed.contains("Imprimer de votre navigateur"));
}
