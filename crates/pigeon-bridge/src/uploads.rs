//! Nextcloud chunk uploads, isolated by authenticated client. Only a complete
//! final MOVE enters the peer's tree. Upload destinations survive restart, and
//! abandoned chunks expire after 24 hours without changing shared files.

use std::path::{Path, PathBuf};

use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use pigeon_proto::bridge::Change;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    App, Error, Result, bad,
    dav::{self, Temporary},
    forbidden, index,
    store::{self, Client},
    unavailable,
};

#[derive(Serialize, Deserialize)]
struct Upload {
    destination: Option<String>,
    touched: u64,
}

fn read(path: &Path) -> Result<Upload> {
    let bytes = std::fs::read(path.join("upload.json"))
        .map_err(|_| Error(StatusCode::NOT_FOUND, "Upload not found".into()))?;
    serde_json::from_slice(&bytes).map_err(unavailable)
}

fn write(path: &Path, upload: &Upload) -> Result<()> {
    pigeon_sys::fs::write_atomic(
        &path.join("upload.json"),
        &serde_json::to_vec(upload).map_err(unavailable)?,
    )
    .map_err(unavailable)
}

fn chunks(path: &Path) -> Result<Vec<PathBuf>> {
    let mut chunks = std::fs::read_dir(path)
        .map_err(unavailable)?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(unavailable)?
        .into_iter()
        .filter(|e| e.file_name() != "upload.json")
        .map(|e| e.path())
        .collect::<Vec<_>>();
    chunks.sort_by_key(|p| {
        p.file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(u64::MAX)
    });
    if chunks.iter().any(|p| {
        !p.symlink_metadata()
            .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
    }) {
        return Err(unavailable("Invalid upload chunk"));
    }
    Ok(chunks)
}

fn cleanup(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root).map_err(unavailable)?;
    for entry in std::fs::read_dir(root).map_err(unavailable)? {
        let path = entry.map_err(unavailable)?.path();
        if let Ok(upload) = read(&path)
            && store::now().saturating_sub(upload.touched) >= 86400
        {
            std::fs::remove_dir_all(path).map_err(unavailable)?;
        }
    }
    Ok(())
}

fn usage(root: &Path) -> Result<u64> {
    let mut bytes = 0u64;
    for entry in std::fs::read_dir(root).map_err(unavailable)? {
        for chunk in chunks(&entry.map_err(unavailable)?.path())? {
            bytes = bytes.saturating_add(std::fs::metadata(chunk).map_err(unavailable)?.len());
        }
    }
    Ok(bytes)
}

pub async fn handle(
    app: &App,
    c: &Client,
    id: &str,
    part: Option<&str>,
    request: Request,
) -> Result<Response> {
    let root = app.config.state_dir.join("uploads").join(&c.id);
    cleanup(&root)?;
    let folder = root.join(id);
    if request.method().as_str() == "MKCOL" && part.is_none() {
        if folder.exists() {
            return Err(Error(
                StatusCode::METHOD_NOT_ALLOWED,
                "Upload already exists".into(),
            ));
        }
        if std::fs::read_dir(&root).map_err(unavailable)?.count() >= 8 {
            return Err(Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many unfinished uploads".into(),
            ));
        }
        let destination = if request.headers().contains_key("Destination") {
            Some(dav::destination(app, &c.user, request.headers())?)
        } else {
            None
        };
        let body = axum::body::to_bytes(request.into_body(), 1024)
            .await
            .map_err(bad)?;
        if !body.is_empty() {
            return Err(bad("An upload collection has no request body"));
        }
        std::fs::create_dir(&folder).map_err(unavailable)?;
        write(
            &folder,
            &Upload {
                destination,
                touched: store::now(),
            },
        )?;
        return Ok(StatusCode::CREATED.into_response());
    }
    let mut upload = read(&folder)?;
    if request.method().as_str() == "PROPFIND" {
        let depth = dav::header_text(request.headers(), "Depth").unwrap_or("1");
        if !matches!(depth, "0" | "1") {
            return Err(forbidden());
        }
        let mut entries = Vec::new();
        if part.is_none() {
            entries.push(index::Node {
                path: String::new(),
                directory: true,
                etag: store::hash(&upload.touched.to_le_bytes()),
                id: 0,
                modified: upload.touched,
                size: 0,
                diverging: false,
            });
        }
        if depth == "1" || part.is_some() {
            for path in chunks(&folder)? {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| bad("Invalid chunk name"))?;
                if part.is_some_and(|p| p != name) {
                    continue;
                }
                let metadata = std::fs::metadata(&path).map_err(unavailable)?;
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                entries.push(index::Node {
                    path: name.into(),
                    directory: false,
                    etag: store::hash(format!("{name}-{}-{modified}", metadata.len()).as_bytes()),
                    id: 0,
                    modified,
                    size: metadata.len(),
                    diverging: false,
                });
            }
        }
        if entries.is_empty() {
            return Err(Error(StatusCode::NOT_FOUND, "Chunk not found".into()));
        }
        let body = axum::body::to_bytes(request.into_body(), 64 * 1024)
            .await
            .map_err(bad)?;
        return crate::dav_xml::upload_listing(c, id, &entries, &body);
    }
    if request.headers().contains_key("Destination") {
        let dest = dav::destination(app, &c.user, request.headers())?;
        if upload.destination.as_ref().is_some_and(|d| d != &dest) {
            return Err(forbidden());
        }
        upload.destination = Some(dest);
    }
    match (request.method().as_str(), part) {
        ("PUT", Some(part)) if part != ".file" => {
            if part.split('-').count() > 2
                || !part.split('-').all(|s| {
                    !s.is_empty()
                        && s.bytes().all(|b| b.is_ascii_digit())
                        && s.parse::<u64>().is_ok()
                })
            {
                return Err(bad("Chunk names must be numeric offsets or indices"));
            }
            if !folder.join(part).exists() && chunks(&folder)?.len() >= 10000 {
                return Err(bad("Too many upload chunks"));
            }
            let old = std::fs::metadata(folder.join(part)).map_or(0, |m| m.len());
            let remaining = app
                .config
                .max_upload_bytes
                .saturating_sub(usage(&root)?.saturating_sub(old));
            if dav::header_text(request.headers(), "OC-Total-Length").is_some_and(|n| {
                n.parse::<u64>()
                    .map_or(true, |n| n > app.config.max_upload_bytes)
            }) {
                return Err(Error(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "Upload exceeds the configured storage budget".into(),
                ));
            }
            let checksum = dav::header_text(request.headers(), "OC-Checksum").map(str::to_owned);
            let stage =
                dav::stage(app, request.into_body(), remaining, checksum.as_deref()).await?;
            let existed = folder.join(part).exists();
            std::fs::rename(&stage.0, folder.join(part)).map_err(unavailable)?;
            upload.touched = store::now();
            write(&folder, &upload)?;
            Ok(if existed {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::CREATED
            }
            .into_response())
        }
        ("MOVE", Some(".file")) => {
            let dest = dav::destination(app, &c.user, request.headers())?;
            let index = index::load(app, c).await?;
            let existing = index.nodes.get(&dest);
            if existing.is_some_and(|n| n.directory) {
                return Err(Error(StatusCode::CONFLICT, "A folder has this name".into()));
            }
            dav::preconditions(request.headers(), existing)?;
            if existing.is_some() && dav::header_text(request.headers(), "Overwrite") == Some("F") {
                return Err(Error(
                    StatusCode::PRECONDITION_FAILED,
                    "The destination exists".into(),
                ));
            }
            dav::parent_exists(
                &index,
                &dest,
                dav::header_text(request.headers(), "X-NC-WebDAV-Auto-Mkcol") == Some("1"),
            )?;
            let modified = dav::modified(request.headers())?;
            let total = dav::header_text(request.headers(), "OC-Total-Length")
                .map(|s| s.parse::<u64>().map_err(bad))
                .transpose()?;
            let assembled = assemble(
                app,
                &folder,
                total,
                dav::header_text(request.headers(), "OC-Checksum"),
            )
            .await?;
            let (revision, suggested) = dav::apply(
                app,
                c,
                &index,
                vec![Change::Put {
                    path: dest.clone(),
                    staged: assembled.0.display().to_string(),
                }],
            )
            .await?;
            let updated = index::load_at(app, c, Some(&revision)).await?;
            index::mtime(app, c, &dest, modified).await?;
            std::fs::remove_dir_all(&folder).map_err(unavailable)?;
            let node = updated
                .nodes
                .get(&dest)
                .ok_or_else(|| unavailable("The upload was not captured"))?;
            Ok(dav::written(
                if existing.is_some() {
                    StatusCode::NO_CONTENT
                } else {
                    StatusCode::CREATED
                },
                node,
                suggested,
                modified.is_some(),
            ))
        }
        ("DELETE", None) => {
            std::fs::remove_dir_all(folder).map_err(unavailable)?;
            Ok(StatusCode::NO_CONTENT.into_response())
        }
        ("GET" | "HEAD", _) => Err(forbidden()),
        _ => Err(Error(
            StatusCode::METHOD_NOT_ALLOWED,
            "Unsupported upload operation".into(),
        )),
    }
}

async fn assemble(
    app: &App,
    folder: &Path,
    total: Option<u64>,
    checksum: Option<&str>,
) -> Result<Temporary> {
    let chunks = chunks(folder)?;
    if chunks.is_empty() {
        return Err(bad("No chunks have been uploaded"));
    }
    let assembled = Temporary(
        app.config
            .state_dir
            .join("staging")
            .join(store::random().map_err(unavailable)?),
    );
    let mut out = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&assembled.0)
        .await
        .map_err(unavailable)?;
    let mut size = 0u64;
    let mut hash = Sha256::new();
    for chunk in chunks {
        let mut input = tokio::fs::File::open(chunk).await.map_err(unavailable)?;
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let n = input.read(&mut buffer).await.map_err(unavailable)?;
            if n == 0 {
                break;
            }
            size = size.saturating_add(n as u64);
            if size > app.config.max_upload_bytes {
                return Err(Error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Upload too large".into(),
                ));
            }
            hash.update(&buffer[..n]);
            out.write_all(&buffer[..n]).await.map_err(unavailable)?;
        }
    }
    if total.is_some_and(|n| n != size) {
        return Err(bad(
            "The uploaded chunks do not match the total file length",
        ));
    }
    if let Some(checksum) = checksum {
        let (kind, expected) = checksum
            .split_once(':')
            .ok_or_else(|| bad("Invalid checksum"))?;
        let actual: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if !kind.eq_ignore_ascii_case("SHA256") || !actual.eq_ignore_ascii_case(expected) {
            return Err(bad("Upload checksum does not match"));
        }
    }
    out.sync_all().await.map_err(unavailable)?;
    Ok(assembled)
}
