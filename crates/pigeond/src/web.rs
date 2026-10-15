//! Serves pigeon-web for this daemon (10 §10.3, ADR 0021): on 127.0.0.1
//! only, on a port the system picks, written to `<runtime>/web-port` (11
//! §11.2); its pages ask the same handler as the local socket.

use std::sync::Arc;

use crate::daemon::Daemon;

/// The loops, as the web pages reach them.
struct Backend(Arc<Daemon>);

impl pigeon_web::Backend for Backend {
    fn request(&self, request: pigeon_proto::Request) -> pigeon_web::Answer {
        let d = self.0.clone();
        Box::pin(async move { crate::api::handle(&d, request).await })
    }
}

/// Starts the web interface; without it, `pigeon open` says so.
pub async fn serve(d: &Arc<Daemon>) {
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", 0)).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, "web-not-started");
            return;
        }
    };
    let port = listener.local_addr().map_or(0, |a| a.port());
    let system = pigeon_i18n::system_languages(|k| std::env::var(k).ok());
    // The `language` setting (11 §11.2), else the system's (10 §10.5).
    let setting = d
        .config
        .language
        .as_deref()
        .and_then(pigeon_i18n::Setting::parse)
        .unwrap_or(pigeon_i18n::Setting::Auto);
    let lang = pigeon_i18n::choose(&setting, &system);
    let web = pigeon_web::Web::new(Arc::new(Backend(d.clone())), lang, port);
    let _ = std::fs::create_dir_all(&d.dirs.runtime);
    let _ = pigeon_sys::fs::write_atomic(
        &d.dirs.runtime.join("web-port"),
        port.to_string().as_bytes(),
    );
    let router = web.router();
    let _ = d.web.set(web);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            tracing::warn!(error = %e, "web-stopped");
        }
    });
    tracing::info!(port, "web-started");
}
