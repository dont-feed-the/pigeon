//! Native desktop notifications on the maintained zbus D-Bus client
//! (ADR 0082). Subscribe before Notify so an immediate action is retained;
//! only this notification's default action may open pigeon's local page.

use std::collections::BTreeMap;

use futures_util::StreamExt;

const SERVICE: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

pub(super) async fn show(body: &str, open: &str, url: Option<String>) {
    let result = async {
        let connection = zbus::Connection::session().await?;
        let clicked = deliver(&connection, body, open).await?;
        if clicked && let Some(url) = url {
            let _ = tokio::process::Command::new("xdg-open")
                .arg(url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;
        }
        Ok::<_, zbus::Error>(())
    }
    .await;
    if let Err(error) = result {
        tracing::debug!(%error, "desktop-notification-unavailable");
    }
}

async fn deliver(connection: &zbus::Connection, body: &str, open: &str) -> zbus::Result<bool> {
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(SERVICE)
        .path(PATH)
        .interface(SERVICE)
        .build()?;
    let mut signals =
        zbus::MessageStream::for_match_rule(rule, connection, std::num::NonZeroUsize::new(16))
            .await?;
    let reply = connection
        .call_method(
            Some(SERVICE),
            PATH,
            Some(SERVICE),
            "Notify",
            &(
                "pigeon",
                0u32,
                "",
                "pigeon",
                body,
                vec!["default", open],
                BTreeMap::<&str, zbus::Variant<'_>>::new(),
                -1i32,
            ),
        )
        .await?;
    let id: u32 = reply.body().deserialize()?;
    while let Some(message) = signals.next().await {
        let message = message?;
        let header = message.header();
        match header.member().map(|m| m.as_str()) {
            Some("ActionInvoked") => {
                let (notification, action): (u32, String) = message.body().deserialize()?;
                if notification == id {
                    return Ok(action == "default");
                }
            }
            Some("NotificationClosed") => {
                let (notification, _): (u32, u32) = message.body().deserialize()?;
                if notification == id {
                    return Ok(false);
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "notices_linux_tests.rs"]
mod tests;
