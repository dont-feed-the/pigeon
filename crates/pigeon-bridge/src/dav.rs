//! WebDAV file operations. Bodies are staged outside shared trees, conditional
//! requests become daemon revision checks, and downloads stream verified private
//! copies. Per-client locks also serialize DAV metadata and chunk assembly.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::{StreamExt, stream};
use pigeon_proto::bridge::{Change, Operation, Reply};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::{
    App, Error, Result, auth, bad, forbidden,
    index::{self, Index, Node},
    rpc,
    store::{self, Client},
    unavailable,
};

pub enum Resource {
    Files(String),
    Upload { id: String, part: Option<String> },
}

pub fn resource(raw: &str, user: &str) -> Result<Resource> {
    let path = percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .map_err(bad)?;
    let path = path.as_ref();
    let relative = if let Some(rest) = path.strip_prefix("/remote.php/webdav") {
        if !rest.is_empty() && !rest.starts_with('/') {
            return Err(bad("Unknown DAV path"));
        }
        Some(rest.trim_start_matches('/').trim_end_matches('/'))
    } else if matches!(path, "/remote.php/dav" | "/remote.php/dav/") {
        Some("")
    } else {
        let rest = path
            .strip_prefix("/remote.php/dav/")
            .ok_or_else(|| bad("Unknown DAV path"))?;
        let mut components = rest.splitn(3, '/');
        let kind = components.next().unwrap_or_default();
        let owner = components.next().unwrap_or_default();
        if owner != user {
            return Err(forbidden());
        }
        let tail = components.next().unwrap_or_default().trim_end_matches('/');
        match kind {
            "files" => Some(tail),
            "uploads" => {
                let (id, part) = tail
                    .split_once('/')
                    .map_or((tail, None), |(a, b)| (a, Some(b.to_owned())));
                if !upload_component(id) || part.as_ref().is_some_and(|p| !upload_component(p)) {
                    return Err(bad("Invalid upload path"));
                }
                return Ok(Resource::Upload {
                    id: id.to_owned(),
                    part,
                });
            }
            _ => return Err(bad("Unknown DAV collection")),
        }
    };
    let path = relative.unwrap_or_default();
    if !path.is_empty() && !index::path_valid(path) {
        return Err(forbidden());
    }
    Ok(Resource::Files(path.to_owned()))
}

fn upload_component(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !matches!(s, "." | "..")
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub fn destination(app: &App, user: &str, headers: &HeaderMap) -> Result<String> {
    let url = headers
        .get("Destination")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| bad("Destination is required"))?;
    let url = url::Url::parse(url).map_err(bad)?;
    if url.origin() != app.config.origin().map_err(unavailable)?.origin()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(forbidden());
    }
    match resource(url.path(), user)? {
        Resource::Files(path) if !path.is_empty() => Ok(path),
        _ => Err(forbidden()),
    }
}

pub fn header_text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

pub fn modified(headers: &HeaderMap) -> Result<Option<u64>> {
    header_text(headers, "X-OC-Mtime")
        .map(|s| {
            s.parse::<u64>()
                .map_err(|_| bad("Invalid modification time"))
        })
        .transpose()
}

pub fn preconditions(headers: &HeaderMap, node: Option<&Node>) -> Result<()> {
    let matches = |values: &str| {
        node.is_some_and(|n| {
            values
                .split(',')
                .any(|v| v.trim() == "*" || v.trim() == format!("\"{}\"", n.etag))
        })
    };
    if header_text(headers, "If-Match").is_some_and(|v| !matches(v))
        || header_text(headers, "If-None-Match").is_some_and(matches)
    {
        return Err(Error(
            StatusCode::PRECONDITION_FAILED,
            "The file changed; synchronize again".into(),
        ));
    }
    Ok(())
}

fn cached_match(values: &str, node: &Node) -> bool {
    values.split(',').any(|value| {
        let value = value.trim().strip_prefix("W/").unwrap_or(value.trim());
        value == "*" || value == format!("\"{}\"", node.etag)
    })
}

pub async fn handle(State(app): State<Arc<App>>, request: Request) -> Result<Response> {
    let c = auth::client(&app, request.headers()).await?;
    let lock = app.client_lock(&c.id).await;
    let _lock = lock.lock().await;
    // Revocation may have happened while another transfer held the lock.
    let c = auth::client(&app, request.headers()).await?;
    let target = resource(request.uri().path(), &c.user)?;
    if request.method() == Method::OPTIONS {
        let mut response = StatusCode::NO_CONTENT.into_response();
        response
            .headers_mut()
            .insert("DAV", HeaderValue::from_static("1, 3"));
        response.headers_mut().insert(
            header::ALLOW,
            HeaderValue::from_static(
                "OPTIONS, PROPFIND, GET, HEAD, PUT, MKCOL, DELETE, MOVE, COPY, PROPPATCH",
            ),
        );
        return Ok(response);
    }
    match target {
        Resource::Upload { id, part } => {
            crate::uploads::handle(&app, &c, &id, part.as_deref(), request).await
        }
        Resource::Files(path) => files(&app, &c, &path, request).await,
    }
}

async fn files(app: &App, c: &Client, path: &str, request: Request) -> Result<Response> {
    let index = index::load(app, c).await?;
    let node = index.nodes.get(path);
    let method = request.method().as_str();
    if method == "PROPFIND" {
        let node = node.ok_or_else(|| Error(StatusCode::NOT_FOUND, "File not found".into()))?;
        let depth = header_text(request.headers(), "Depth")
            .unwrap_or("1")
            .to_owned();
        if depth != "0" && depth != "1" {
            return Err(Error(StatusCode::FORBIDDEN, "Use Depth 0 or 1".into()));
        }
        let bytes = axum::body::to_bytes(request.into_body(), 64 * 1024)
            .await
            .map_err(bad)?;
        return crate::dav_xml::listing(c, &index, node, &depth, &bytes);
    }
    if method == "GET" || method == "HEAD" {
        let node = node
            .filter(|n| !n.directory)
            .ok_or_else(|| Error(StatusCode::NOT_FOUND, "File not found".into()))?;
        return download(app, c, node, request.headers(), method == "HEAD").await;
    }
    if path.is_empty() {
        return Err(forbidden());
    }
    preconditions(request.headers(), node)?;
    match method {
        "PUT" => {
            if node.is_some_and(|n| n.directory) {
                return Err(Error(StatusCode::CONFLICT, "A folder has this name".into()));
            }
            parent_exists(
                &index,
                path,
                header_text(request.headers(), "X-NC-WebDAV-Auto-Mkcol") == Some("1"),
            )?;
            let mtime = modified(request.headers())?;
            let checksum = header_text(request.headers(), "OC-Checksum").map(str::to_owned);
            let stage = stage(
                app,
                request.into_body(),
                app.config.max_upload_bytes,
                checksum.as_deref(),
            )
            .await?;
            let (revision, suggested) = apply(
                app,
                c,
                &index,
                vec![Change::Put {
                    path: path.into(),
                    staged: stage.0.display().to_string(),
                }],
            )
            .await?;
            let updated = index::load_at(app, c, Some(&revision)).await?;
            index::mtime(app, c, path, mtime).await?;
            let node = updated
                .nodes
                .get(path)
                .ok_or_else(|| unavailable("The file was not captured"))?;
            Ok(written(
                if index.nodes.contains_key(path) {
                    StatusCode::NO_CONTENT
                } else {
                    StatusCode::CREATED
                },
                node,
                suggested,
                mtime.is_some(),
            ))
        }
        "MKCOL" => {
            if node.is_some() {
                return Err(Error(
                    StatusCode::METHOD_NOT_ALLOWED,
                    "This path already exists".into(),
                ));
            }
            parent_exists(&index, path, false)?;
            let body = axum::body::to_bytes(request.into_body(), 1024)
                .await
                .map_err(bad)?;
            if !body.is_empty() {
                return Err(Error(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "Extended MKCOL bodies are not supported".into(),
                ));
            }
            index::mkdir(app, c, path).await?;
            Ok(StatusCode::CREATED.into_response())
        }
        "DELETE" => {
            let _ = node.ok_or_else(|| Error(StatusCode::NOT_FOUND, "File not found".into()))?;
            let changes = index
                .nodes
                .values()
                .filter(|f| !f.directory && index::under(&f.path, path))
                .map(|f| Change::Remove {
                    path: f.path.clone(),
                })
                .collect::<Vec<_>>();
            if !changes.is_empty() {
                apply(app, c, &index, changes).await?;
            }
            index::remove_metadata(app, c, path).await?;
            Ok(StatusCode::NO_CONTENT.into_response())
        }
        "MOVE" | "COPY" => {
            transfer(app, c, path, &index, request.headers(), method == "MOVE").await
        }
        "PROPPATCH" => {
            let node = node.ok_or_else(|| Error(StatusCode::NOT_FOUND, "File not found".into()))?;
            let bytes = axum::body::to_bytes(request.into_body(), 64 * 1024)
                .await
                .map_err(bad)?;
            crate::dav_xml::unsupported_patch(c, node, &bytes)
        }
        _ => Err(Error(
            StatusCode::METHOD_NOT_ALLOWED,
            "Unsupported DAV method".into(),
        )),
    }
}

pub fn parent_exists(index: &Index, path: &str, automatic: bool) -> Result<()> {
    let parent = path.rsplit_once('/').map_or("", |(p, _)| p);
    if automatic || index.nodes.get(parent).is_some_and(|n| n.directory) {
        Ok(())
    } else {
        Err(Error(
            StatusCode::CONFLICT,
            "The parent folder does not exist".into(),
        ))
    }
}

pub async fn apply(
    app: &App,
    client: &Client,
    index: &Index,
    changes: Vec<Change>,
) -> Result<(String, usize)> {
    let expected = if changes.len() == 1 {
        let path = match &changes[0] {
            Change::Put { path, .. } | Change::Remove { path } => path,
        };
        Some(
            [(
                path.clone(),
                index
                    .nodes
                    .get(path)
                    .filter(|f| !f.directory)
                    .map(|f| f.etag.clone()),
            )]
            .into_iter()
            .collect(),
        )
    } else {
        None
    };
    let socket = app.peers.checked(client).await.map_err(unavailable)?;
    match index::reply(
        rpc::bridge(
            &socket,
            Operation::Apply {
                revision: index.revision.clone(),
                expected,
                changes,
            },
        )
        .await
        .map_err(unavailable)?,
    )? {
        Reply::Applied {
            revision,
            suggested,
        } => Ok((revision, suggested)),
        _ => Err(unavailable("The peer did not record the change")),
    }
}

pub fn written(status: StatusCode, node: &Node, suggested: usize, mtime: bool) -> Response {
    let mut response = status.into_response();
    set(
        response.headers_mut(),
        "ETag",
        &format!("\"{}\"", node.etag),
    );
    set(
        response.headers_mut(),
        "OC-Etag",
        &format!("\"{}\"", node.etag),
    );
    set(
        response.headers_mut(),
        "OC-FileId",
        &format!("{:08}pgbridge", node.id),
    );
    set(
        response.headers_mut(),
        "X-Pigeon-Suggested",
        &suggested.to_string(),
    );
    if mtime {
        response
            .headers_mut()
            .insert("X-OC-Mtime", HeaderValue::from_static("accepted"));
    }
    response
}

pub fn set(headers: &mut HeaderMap, name: &'static str, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name, value);
    }
}

pub struct Temporary(pub PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub async fn stage(app: &App, body: Body, limit: u64, checksum: Option<&str>) -> Result<Temporary> {
    let stage = Temporary(
        app.config
            .state_dir
            .join("staging")
            .join(store::random().map_err(unavailable)?),
    );
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stage.0)
        .await
        .map_err(unavailable)?;
    let mut stream = body.into_data_stream();
    let mut size = 0u64;
    let mut hash = Sha256::new();
    while let Some(bytes) = stream.next().await {
        let bytes = bytes.map_err(bad)?;
        size = size.saturating_add(bytes.len() as u64);
        if size > limit {
            return Err(Error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "The upload exceeds the configured limit".into(),
            ));
        }
        hash.update(&bytes);
        file.write_all(&bytes).await.map_err(unavailable)?;
    }
    file.sync_all().await.map_err(unavailable)?;
    if let Some(checksum) = checksum {
        let (algorithm, expected) = checksum
            .split_once(':')
            .ok_or_else(|| bad("Invalid checksum"))?;
        if !algorithm.eq_ignore_ascii_case("SHA256") {
            return Err(bad("Only SHA256 upload checksums are supported"));
        }
        let actual: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(bad("The uploaded content does not match its checksum"));
        }
    }
    Ok(stage)
}

pub async fn read(app: &App, c: &Client, node: &Node) -> Result<Temporary> {
    let stage = Temporary(
        app.config
            .state_dir
            .join("staging")
            .join(store::random().map_err(unavailable)?),
    );
    let socket = app.peers.checked(c).await.map_err(unavailable)?;
    match index::reply(
        rpc::bridge(
            &socket,
            Operation::Read {
                path: node.path.clone(),
                etag: node.etag.clone(),
                destination: stage.0.display().to_string(),
            },
        )
        .await
        .map_err(unavailable)?,
    )? {
        Reply::Read => Ok(stage),
        _ => Err(unavailable("The file could not be prepared")),
    }
}

pub fn range(value: Option<&str>, size: u64) -> Result<(u64, u64, bool)> {
    let fail = || {
        Error(
            StatusCode::RANGE_NOT_SATISFIABLE,
            "Requested range is unavailable".into(),
        )
    };
    let Some(value) = value else {
        return Ok((0, size, false));
    };
    let spec = value.strip_prefix("bytes=").ok_or_else(fail)?;
    let (from, to) = spec.split_once('-').ok_or_else(fail)?;
    if from.is_empty() {
        let count: u64 = to.parse().map_err(|_| fail())?;
        if count == 0 || size == 0 {
            return Err(fail());
        }
        let count = count.min(size);
        return Ok((size - count, count, true));
    }
    let from: u64 = from.parse().map_err(|_| fail())?;
    let end: u64 = if to.is_empty() {
        size.saturating_sub(1)
    } else {
        to.parse::<u64>()
            .map_err(|_| fail())?
            .min(size.saturating_sub(1))
    };
    if from >= size || end < from {
        return Err(fail());
    }
    Ok((from, end - from + 1, true))
}

pub async fn download(
    app: &App,
    c: &Client,
    node: &Node,
    headers: &HeaderMap,
    head: bool,
) -> Result<Response> {
    if header_text(headers, "If-None-Match").is_some_and(|v| cached_match(v, node)) {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        set(
            response.headers_mut(),
            "ETag",
            &format!("\"{}\"", node.etag),
        );
        return Ok(response);
    }
    preconditions(headers, node.into())?;
    // A stale resume validator must return the complete new representation.
    // Dates are conservatively treated as unavailable validators: upload mtimes
    // can be preserved across different content versions.
    let requested_range =
        if header_text(headers, "If-Range").is_none_or(|v| v == format!("\"{}\"", node.etag)) {
            header_text(headers, "Range")
        } else {
            None
        };
    let (offset, count, partial) = match range(requested_range, node.size) {
        Ok(range) => range,
        Err(error) => {
            let mut response = error.into_response();
            set(
                response.headers_mut(),
                "Content-Range",
                &format!("bytes */{}", node.size),
            );
            return Ok(response);
        }
    };
    let body = if head {
        Body::empty()
    } else {
        let stage = read(app, c, node).await?;
        let mut file = tokio::fs::File::open(&stage.0).await.map_err(unavailable)?;
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(unavailable)?;
        Body::from_stream(stream::try_unfold(
            (file, stage, count),
            |(mut file, stage, left)| async move {
                if left == 0 {
                    return Ok::<_, std::io::Error>(None);
                }
                let mut buffer = vec![0; left.min(64 * 1024) as usize];
                let n = file.read(&mut buffer).await?;
                if n == 0 {
                    return Err(std::io::ErrorKind::UnexpectedEof.into());
                }
                buffer.truncate(n);
                Ok(Some((Bytes::from(buffer), (file, stage, left - n as u64))))
            },
        ))
    };
    let mut response = Response::new(body);
    *response.status_mut() = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    set(response.headers_mut(), "Content-Length", &count.to_string());
    set(
        response.headers_mut(),
        "ETag",
        &format!("\"{}\"", node.etag),
    );
    set(
        response.headers_mut(),
        "Last-Modified",
        &httpdate::fmt_http_date(
            UNIX_EPOCH + Duration::from_secs(node.modified.min(253_402_300_799)),
        ),
    );
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment"),
    );
    if partial {
        set(
            response.headers_mut(),
            "Content-Range",
            &format!("bytes {offset}-{}/{}", offset + count - 1, node.size),
        );
    }
    Ok(response)
}

async fn transfer(
    app: &App,
    c: &Client,
    path: &str,
    index: &Index,
    headers: &HeaderMap,
    moving: bool,
) -> Result<Response> {
    let _source = index
        .nodes
        .get(path)
        .ok_or_else(|| Error(StatusCode::NOT_FOUND, "File not found".into()))?;
    let dest = destination(app, &c.user, headers)?;
    if index::under(&dest, path) || index::under(path, &dest) {
        return Err(Error(
            StatusCode::CONFLICT,
            "Source and destination overlap".into(),
        ));
    }
    let existed = index.nodes.contains_key(&dest);
    if existed && header_text(headers, "Overwrite") == Some("F") {
        return Err(Error(
            StatusCode::PRECONDITION_FAILED,
            "The destination exists".into(),
        ));
    }
    if header_text(headers, "Overwrite").is_some_and(|v| v != "F" && v != "T") {
        return Err(bad("Invalid Overwrite header"));
    }
    parent_exists(index, &dest, false)?;
    let mut stages = Vec::new();
    let mut changes = std::collections::BTreeMap::new();
    if existed {
        for file in index
            .nodes
            .values()
            .filter(|f| !f.directory && index::under(&f.path, &dest))
        {
            changes.insert(
                file.path.clone(),
                Change::Remove {
                    path: file.path.clone(),
                },
            );
        }
    }
    for file in index
        .nodes
        .values()
        .filter(|f| !f.directory && index::under(&f.path, path))
    {
        let stage = read(app, c, file).await?;
        let destination = format!("{dest}{}", file.path.strip_prefix(path).unwrap_or_default());
        changes.insert(
            destination.clone(),
            Change::Put {
                path: destination,
                staged: stage.0.display().to_string(),
            },
        );
        if moving {
            changes.insert(
                file.path.clone(),
                Change::Remove {
                    path: file.path.clone(),
                },
            );
        }
        stages.push(stage);
    }
    let (revision, suggested) = if changes.is_empty() {
        (index.revision.clone(), 0)
    } else {
        apply(app, c, index, changes.into_values().collect()).await?
    };
    // Preserve explicit empty subdirectories and times; only MOVE retains IDs.
    let mut store = app.store.lock().await;
    let mut metadata =
        std::mem::take(&mut store.clients.get_mut(&c.id).ok_or_else(forbidden)?.metadata);
    let originals: Vec<_> = metadata
        .iter()
        .filter(|(p, _)| index::under(p, path))
        .map(|(p, m)| (p.clone(), m.clone()))
        .collect();
    metadata.retain(|p, _| !index::under(p, &dest) && !(moving && index::under(p, path)));
    for (p, mut meta) in originals {
        let target = format!("{dest}{}", p.strip_prefix(path).unwrap_or_default());
        if !moving {
            meta.id = store.next_file_id;
            store.next_file_id = store
                .next_file_id
                .checked_add(1)
                .ok_or_else(|| unavailable("File ID space exhausted"))?;
        }
        metadata.insert(target, meta);
    }
    store.clients.get_mut(&c.id).ok_or_else(forbidden)?.metadata = metadata;
    store.save(&app.config.state_dir).map_err(unavailable)?;
    drop(store);
    let index = index::load_at(app, c, Some(&revision)).await?;
    let node = index
        .nodes
        .get(&dest)
        .ok_or_else(|| unavailable("Destination unavailable"))?;
    Ok(written(
        if existed {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::CREATED
        },
        node,
        suggested,
        false,
    ))
}
