//! Opt-in DAV transaction observations. Each request has a private stream;
//! responses mean constructed HTTP responses, never acknowledged delivery.

#[derive(Default)]
struct Trace {
    #[cfg(feature = "tla-trace")]
    file: Option<std::fs::File>,
    #[cfg(feature = "tla-trace")]
    clock: u64,
}

tokio::task_local! {
    static TRACE: std::cell::RefCell<Trace>;
}

impl Trace {
    fn open(root: &std::path::Path) -> Self {
        #[cfg(feature = "tla-trace")]
        {
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
            if std::env::var("PIGEON_TRACE").as_deref() != Ok("1") {
                return Self::default();
            }
            let open = || -> std::io::Result<std::fs::File> {
                let root = std::env::var_os("PIGEON_TRACE_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| root.join("bridge-traces"));
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(&root)?;
                let id = crate::store::random().map_err(std::io::Error::other)?;
                std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(root.join(format!("sync-{id}.ndjson")))
            };
            Self {
                file: open().ok(),
                clock: 0,
            }
        }
        #[cfg(not(feature = "tla-trace"))]
        {
            let _ = root;
            Self::default()
        }
    }

    fn emit(&mut self, action: &str, vars: impl FnOnce() -> serde_json::Value) {
        #[cfg(feature = "tla-trace")]
        {
            use std::io::Write;
            if let Some(file) = &mut self.file {
                self.clock += 1;
                let event = serde_json::json!({"spec": "BridgeSync", "clock": self.clock,
                    "action": action, "vars": vars()});
                if serde_json::to_writer(&mut *file, &event).is_err()
                    || file.write_all(b"\n").is_err()
                {
                    self.file = None;
                }
            }
        }
        #[cfg(not(feature = "tla-trace"))]
        {
            let _ = (action, vars);
        }
    }
}

pub fn emit(action: &str, vars: impl FnOnce() -> serde_json::Value) {
    let _ = TRACE.try_with(|trace| trace.borrow_mut().emit(action, vars));
}

pub async fn request(
    root: &std::path::Path,
    client: &crate::store::Client,
    method: &str,
    kind: &str,
    path: &str,
    operation: impl std::future::Future<Output = crate::Result<axum::response::Response>>,
) -> crate::Result<axum::response::Response> {
    TRACE
        .scope(std::cell::RefCell::new(Trace::open(root)), async {
            emit("Begin", || {
                serde_json::json!({"client": client.id, "device": client.device,
            "method": method, "kind": kind, "path": path})
            });
            let result = operation.await;
            emit("Response", || {
                let (status, etag) = match &result {
                    Ok(response) => (
                        response.status().as_u16(),
                        response.headers().get("ETag").and_then(|v| v.to_str().ok()),
                    ),
                    Err(error) => (error.0.as_u16(), None),
                };
                serde_json::json!({"status": status, "etag": etag})
            });
            result
        })
        .await
}
