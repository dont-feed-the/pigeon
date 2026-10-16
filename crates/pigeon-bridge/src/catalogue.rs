//! Read bounded catalogue pages into the bridge's working index. Discard a
//! partial listing on revision changes; callers only persist complete views.

use std::collections::BTreeMap;
use std::path::Path;

use axum::http::StatusCode;
use pigeon_proto::bridge::{Cursor, File, MAX_PAGE_FILES, Operation, Reply};

use crate::{Error, Result, index, rpc, unavailable};

pub struct Catalogue {
    pub revision: String,
    pub files: BTreeMap<String, File>,
}

/// Ordinary listings retry changes three times. A post-save refresh is pinned
/// to the acknowledged revision, so it cannot return another save's ETag.
pub async fn read(socket: &Path, revision: Option<&str>) -> Result<Catalogue> {
    let mut trace = crate::catalogue_trace::Trace::new(socket, revision);
    for attempt in 0..3 {
        match once(socket, revision, &mut trace).await {
            Err(Error(StatusCode::PRECONDITION_FAILED, _)) if revision.is_none() => {
                if attempt == 2 {
                    trace.emit("Abort", || serde_json::json!({"status": 503}));
                    return Err(unavailable(
                        "The catalogue keeps changing; synchronize again",
                    ));
                }
                trace.emit("Retry", || serde_json::json!({}));
            }
            result => {
                if let Err(error) = &result {
                    trace.emit("Abort", || serde_json::json!({"status": error.0.as_u16()}));
                }
                return result;
            }
        }
    }
    Err(unavailable("Cannot finish the catalogue listing"))
}

async fn once(
    socket: &Path,
    required: Option<&str>,
    trace: &mut crate::catalogue_trace::Trace,
) -> Result<Catalogue> {
    let mut cursor = required.map(|revision| Cursor {
        revision: revision.to_owned(),
        after: None,
    });
    let mut files = BTreeMap::new();
    let mut revision = required.map(str::to_owned);
    loop {
        trace.emit("Request", || serde_json::json!({"cursor": cursor}));
        let result = rpc::bridge(
            socket,
            Operation::Snapshot {
                cursor: cursor.clone(),
                limit: MAX_PAGE_FILES,
            },
        )
        .await
        .map_err(unavailable);
        trace.emit("Reply", || match &result {
            Ok(reply) => serde_json::json!({"reply": reply}),
            Err(_) => serde_json::json!({"reply": {"kind": "failure", "status": 503}}),
        });
        let page = match index::reply(result?)? {
            Reply::Snapshot(page) => page,
            _ => return Err(unavailable("The peer returned no catalogue page")),
        };
        if page.files.len() > MAX_PAGE_FILES
            || page.revision.is_empty()
            || revision.as_ref().is_some_and(|r| r != &page.revision)
        {
            return Err(unavailable("Inconsistent catalogue page"));
        }
        revision = Some(page.revision.clone());
        let mut last = cursor.and_then(|c| c.after);
        let nonempty = !page.files.is_empty();
        for file in page.files {
            if !index::path_valid(&file.path) || last.as_ref().is_some_and(|p| p >= &file.path) {
                return Err(unavailable("Unordered or invalid catalogue page"));
            }
            last = Some(file.path.clone());
            files.insert(file.path.clone(), file);
        }
        match page.next {
            Some(next) if nonempty && next.revision == page.revision && next.after == last => {
                cursor = Some(next);
                trace.emit("Continue", || serde_json::json!({"cursor": cursor, "files": files.values().collect::<Vec<_>>()}));
            }
            Some(_) => return Err(unavailable("Invalid catalogue continuation")),
            None => {
                trace.emit("Publish", || serde_json::json!({"revision": page.revision, "files": files.values().collect::<Vec<_>>()}));
                return Ok(Catalogue {
                    revision: page.revision,
                    files,
                });
            }
        }
    }
}

#[cfg(test)]
#[path = "catalogue_tests.rs"]
mod tests;
