//! A private native D-Bus service checks notification fields and actions
//! without a desktop, external bus, environment mutation or opening a URL.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "native notification protocol fixture"
)]

use std::io::BufRead;
use std::process::{Child, Command, Stdio};

use super::*;

struct Bus(Child);
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

type Notify = (
    String,
    u32,
    String,
    String,
    String,
    Vec<String>,
    BTreeMap<String, zbus::OwnedVariant>,
    i32,
);

async fn session(action: Option<&str>) -> bool {
    let child = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1", "--nopidfile"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut bus = Bus(child);
    let mut address = String::new();
    std::io::BufReader::new(bus.0.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    assert!(!address.trim().is_empty());
    let server = zbus::connection::Builder::address(address.trim())
        .build()
        .await
        .unwrap();
    server.request_name(SERVICE).await.unwrap();
    let mut incoming = zbus::MessageStream::from(&server);
    let client = zbus::connection::Builder::address(address.trim())
        .build()
        .await
        .unwrap();
    let answer = async {
        while let Some(message) = incoming.next().await {
            let message = message.unwrap();
            if !message
                .header()
                .member()
                .is_some_and(|m| m.as_str() == "Notify")
            {
                continue;
            }
            let (app, replaces, icon, summary, body, actions, hints, expires): Notify =
                message.body().deserialize().unwrap();
            assert_eq!(
                (app.as_str(), replaces, icon.as_str(), summary.as_str()),
                ("pigeon", 0, "", "pigeon")
            );
            assert_eq!(body, "A file needs your attention");
            assert_eq!(actions, ["default", "Open pigeon"]);
            assert!(hints.is_empty());
            assert_eq!(expires, -1);
            server.reply(&message.header(), &42u32).await.unwrap();
            // An action for another app's notification must never open us.
            server
                .emit_signal(
                    None::<&str>,
                    PATH,
                    SERVICE,
                    "ActionInvoked",
                    &(99u32, "default"),
                )
                .await
                .unwrap();
            match action {
                Some(action) => server
                    .emit_signal(
                        None::<&str>,
                        PATH,
                        SERVICE,
                        "ActionInvoked",
                        &(42u32, action),
                    )
                    .await
                    .unwrap(),
                None => server
                    .emit_signal(
                        None::<&str>,
                        PATH,
                        SERVICE,
                        "NotificationClosed",
                        &(42u32, 2u32),
                    )
                    .await
                    .unwrap(),
            }
            return;
        }
        panic!("notification call was never received");
    };
    let (clicked, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(
            deliver(&client, "A file needs your attention", "Open pigeon"),
            answer
        )
    })
    .await
    .unwrap();
    clicked.unwrap()
}

#[tokio::test]
async fn only_this_notifications_default_action_opens_pigeon() {
    assert!(session(Some("default")).await);
}

#[tokio::test]
async fn closing_ignores_another_notifications_default_action() {
    assert!(!session(None).await);
}

#[tokio::test]
async fn another_action_does_not_open_pigeon() {
    assert!(!session(Some("other-action")).await);
}
