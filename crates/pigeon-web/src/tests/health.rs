//! Home health without a telemetry backend: current/older inventories,
//! stale capacity despite fresh contact, escaped names and text events.

use super::*;
use pigeon_proto::health::{Counts, Disk, HealthReport, MachineHealth, Queue};

fn report() -> HealthReport {
    let now = crate::now();
    HealthReport {
        group_id: "1".repeat(40),
        group: "Family <script>alert(1)</script>".into(),
        at: pigeon_proto::time::rfc3339(now),
        omitted_paths: 7,
        status: StatusReport {
            headline: Message {
                id: "headline-not-checked".into(),
                args: [(
                    "when".into(),
                    pigeon_proto::Arg::Text(pigeon_proto::time::rfc3339(now - 7200)),
                )]
                .into(),
            },
            level: Level::NotChecked,
            state: State::Local,
            versions: Vec::new(),
            not_shared: Vec::new(),
            paused: None,
        },
        current: Some(Counts {
            protected: 2,
            local: 1,
            published: 3,
            uncertain: 4,
            ..Counts::default()
        }),
        retained: Some(Counts {
            protected: 5,
            ..Counts::default()
        }),
        suggestions: Some(6),
        endangered: 8,
        repair_epochs: None,
        machines: vec![MachineHealth {
            machine: Machine {
                name: "Vault <script>alert(2)</script>".into(),
                device: "2".repeat(64),
                custodian: true,
                recovery: false,
                heard_at: Some(pigeon_proto::time::rfc3339(now)),
                this_device: false,
            },
            reported_at: Some(pigeon_proto::time::rfc3339(now - 7200)),
            report_fresh: false,
            disk: Some(Disk {
                used: 1_000_000,
                free: 2_000_000,
                maxsize: 0,
                reserve: 10_000,
                full: false,
            }),
            checking: false,
            repository_lost: false,
            queue: Some(Queue {
                send_bytes: 1,
                receive_bytes: 2000,
                deferred_keys: 3,
                missing_bytes: 4_000_000,
            }),
        }],
    }
}

struct HealthBackend;
impl Backend for HealthBackend {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        let answer = match request {
            pigeon_proto::Request::Health => vec![Response::Health(Box::new(report()))],
            pigeon_proto::Request::Held => vec![Response::Held { held: Vec::new() }],
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

fn plain(s: &str) -> String {
    s.replace(['\u{2068}', '\u{2069}'], "")
}

#[tokio::test]
async fn home_shows_replicated_health_and_preserves_report_age_without_telemetry() {
    let web = Web::new(Arc::new(HealthBackend), "en".parse().unwrap(), 4321);
    let app = web.router();
    let cookie = logged_in(&app, &web).await;
    let page = plain(
        &body(
            app.clone()
                .oneshot(get("/", HOST, Some(&cookie)))
                .await
                .unwrap(),
        )
        .await,
    );
    assert!(page.contains("Current files") && page.contains("Other versions being kept"));
    assert!(page.contains("Safe: 2") && page.contains("Only on this computer: 1"));
    assert!(page.contains("Not checked recently: 4") && page.contains("Safe: 5"));
    assert!(page.contains("Suggestions awaiting a decision: 6"));
    assert!(page.contains("New contact does not update these figures"));
    assert!(page.contains("Waiting to send: 1 byte") && page.contains("To receive: 2 kB"));
    assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!page.contains("<script>alert("));
    assert!(page.contains("src=\"/live.js\"") && page.contains("data-events=\"/events\""));
    let script = body(
        app.oneshot(get("/live.js", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    assert!(script.contains("textContent") && !script.contains("innerHTML"));
    assert!(page.contains("7 further file explanations"));
}

#[test]
fn unavailable_inventory_and_live_values_do_not_turn_into_zero_protected_files() {
    let web = Web::new(Arc::new(HealthBackend), "fr".parse().unwrap(), 4321);
    let mut h = report();
    h.current = None;
    h.suggestions = None;
    let renderer = web.renderer();
    let event: serde_json::Value =
        serde_json::from_str(&crate::health::event(&renderer, &h)).unwrap();
    let values = &event["values"];
    assert!(
        values["health-current"]
            .as_str()
            .unwrap()
            .starts_with("Pas encore vérifié")
    );
    assert!(
        values["health-suggestions"]
            .as_str()
            .unwrap()
            .ends_with("pas encore vérifiées.")
    );
    assert!(plain(values["health-retained"].as_str().unwrap()).contains("En sécurité : 5"));
    let before = event["membership"].clone();
    h.group_id = "3".repeat(40);
    let next: serde_json::Value =
        serde_json::from_str(&crate::health::event(&renderer, &h)).unwrap();
    assert_ne!(before, next["membership"]);
}
