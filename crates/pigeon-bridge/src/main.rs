//! Runs the Nextcloud bridge from an explicit configuration file, or hashes a
//! portal password read from standard input. HTTPS is normally terminated by a
//! reverse proxy preserving Host; the configured public origin is authoritative.

use std::io::{Read, Write};
use std::path::Path;

use argon2::{Argon2, PasswordHasher, password_hash::SaltString};

fn hash_password() -> Result<(), String> {
    let mut password = String::new();
    std::io::stdin()
        .take(1025)
        .read_to_string(&mut password)
        .map_err(|e| e.to_string())?;
    let password = password.trim_end_matches(['\r', '\n']);
    if password.is_empty() || password.len() > 1024 {
        return Err("Read a nonempty password of at most 1024 bytes from stdin".into());
    }
    let mut salt = [0; 16];
    getrandom::fill(&mut salt).map_err(|e| e.to_string())?;
    let salt = SaltString::encode_b64(&salt).map_err(|e| e.to_string())?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| e.to_string())?;
    writeln!(std::io::stdout(), "{hash}").map_err(|e| e.to_string())
}

async fn run(path: &Path) -> Result<(), String> {
    let config = pigeon_bridge::config::Config::load(path)?;
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(|e| e.to_string())?;
    let app = pigeon_bridge::App::new(config.clone()).await?;
    let unavailable = app.resume_peers().await;
    if unavailable > 0 {
        let _ = writeln!(
            std::io::stderr(),
            "{unavailable} peer(s) are unavailable; existing identities have been retained"
        );
    }
    writeln!(
        std::io::stderr(),
        "Pigeon Bridge listening at {} (public origin {})",
        config.listen,
        config.public_url
    )
    .map_err(|e| e.to_string())?;
    let result = axum::serve(listener, pigeon_bridge::router(app.clone()))
        .with_graceful_shutdown(async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut terminate) => {
                    tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
                }
                Err(_) => {
                    let _ = tokio::signal::ctrl_c().await;
                }
            }
        })
        .await
        .map_err(|e| e.to_string());
    app.shutdown().await;
    result
}

fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [command] if command == "hash-password" => hash_password(),
        [flag, path] if flag == "--config" => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())
            .and_then(|runtime| runtime.block_on(run(Path::new(path)))),
        _ => Err(
            "usage: pigeon-bridge --config FILE | pigeon-bridge hash-password < password-file"
                .into(),
        ),
    };
    if let Err(error) = result {
        let _ = writeln!(std::io::stderr(), "pigeon-bridge: {error}");
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}
