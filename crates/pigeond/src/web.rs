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
        // In its own task: a browser that leaves drops the page's future,
        // but a join or a recovery sheet half done must still finish or
        // take back what it did, as on the local socket.
        let task = tokio::spawn(async move { crate::api::handle(&d, request).await });
        Box::pin(async move {
            task.await.unwrap_or_else(|e| {
                let error = Some(e.to_string());
                vec![crate::api::problem("internal-error", &[], error)]
            })
        })
    }
}

/// The web's language: the `language` setting (11 §11.2), else the
/// system's (10 §10.5).
pub fn language(setting: Option<&str>) -> pigeon_i18n::LanguageIdentifier {
    let system = pigeon_i18n::system_languages(|k| std::env::var(k).ok());
    let setting = setting
        .and_then(pigeon_i18n::Setting::parse)
        .unwrap_or(pigeon_i18n::Setting::Auto);
    pigeon_i18n::choose(&setting, &system)
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
    let lang = language(d.config.language.as_deref());
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
