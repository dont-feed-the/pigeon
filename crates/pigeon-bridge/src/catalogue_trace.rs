//! Opt-in observations of catalogue RPC requests, replies and published views.
//! A separate private log per read preserves concurrent calls without imposing
//! an ordering on them. Disabled builds perform no trace I/O or serialization.

#[derive(Default)]
pub struct Trace {
    #[cfg(feature = "tla-trace")]
    file: Option<std::fs::File>,
    #[cfg(feature = "tla-trace")]
    clock: u64,
}

impl Trace {
    pub fn new(socket: &std::path::Path, required: Option<&str>) -> Self {
        #[cfg(feature = "tla-trace")]
        {
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
            if std::env::var("PIGEON_TRACE").as_deref() != Ok("1") {
                return Self::default();
            }
            let open = || -> std::io::Result<std::fs::File> {
                let root = std::env::var_os("PIGEON_TRACE_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| socket.with_extension("bridge-traces"));
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(&root)?;
                let name = crate::store::random().map_err(std::io::Error::other)?;
                std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(root.join(format!("catalogue-{name}.ndjson")))
            };
            let mut trace = Self {
                file: open().ok(),
                clock: 0,
            };
            trace.emit("Begin", || serde_json::json!({"required": required}));
            trace
        }
        #[cfg(not(feature = "tla-trace"))]
        {
            let _ = (socket, required);
            Self::default()
        }
    }

    pub fn emit(&mut self, action: &str, vars: impl FnOnce() -> serde_json::Value) {
        #[cfg(feature = "tla-trace")]
        {
            use std::io::Write;
            if let Some(file) = &mut self.file {
                self.clock += 1;
                let event = serde_json::json!({"spec": "BridgeCatalogue", "clock": self.clock,
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
