//! Opt-in Tokio task inspection (ADR 0025). A debug build with Tokio's
//! unstable tracing enabled may listen on loopback after a local explicit
//! switch. No console environment variable can choose a remote address.

use std::net::Ipv4Addr;
use tracing::Subscriber;
use tracing_subscriber::{Layer, registry::LookupSpan};

/// A bounded console layer, enabled only by `PIGEON_TOKIO_CONSOLE=1` in a
/// debug build made with `RUSTFLAGS="--cfg tokio_unstable"`. It always
/// binds 127.0.0.1:6669; release builds return no layer.
pub fn console_layer<S>() -> Option<impl Layer<S>>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    if !cfg!(all(debug_assertions, tokio_unstable))
        || std::env::var("PIGEON_TOKIO_CONSOLE").as_deref() != Ok("1")
    {
        return None;
    }
    Some(
        console_subscriber::ConsoleLayer::builder()
            .server_addr((Ipv4Addr::LOCALHOST, 6669))
            .retention(std::time::Duration::from_secs(60))
            .event_buffer_capacity(1024)
            .spawn(),
    )
}
