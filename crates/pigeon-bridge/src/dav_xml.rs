//! Namespace-aware DAV property requests and multistatus responses. Unknown
//! properties get their own 404 propstat instead of fabricated capabilities.

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;

use crate::{
    Result, bad,
    index::{self, Index, Node},
    store::Client,
};

const DAV: &str = "DAV:";
const OC: &str = "http://owncloud.org/ns";
const NC: &str = "http://nextcloud.org/ns";

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn href(user: &str, path: &str, directory: bool) -> String {
    let encoded = path
        .split('/')
        .map(|p| {
            percent_encoding::utf8_percent_encode(p, percent_encoding::NON_ALPHANUMERIC).to_string()
        })
        .collect::<Vec<_>>()
        .join("/");
    format!(
        "/remote.php/dav/files/{user}/{encoded}{}",
        if directory && !path.is_empty() {
            "/"
        } else {
            ""
        }
    )
}

pub fn properties(body: &[u8]) -> Result<Vec<(String, String)>> {
    if body.is_empty() {
        return Ok(defaults());
    }
    let mut reader = NsReader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut depth = 0usize;
    let mut prop_depth = None;
    let mut props = Vec::new();
    let mut all = false;
    loop {
        let (ns, event) = reader.read_resolved_event().map_err(bad)?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let local = e.local_name();
                let local = std::str::from_utf8(local.as_ref()).map_err(bad)?.to_owned();
                let ns = match ns {
                    ResolveResult::Bound(ns) => {
                        std::str::from_utf8(ns.as_ref()).map_err(bad)?.to_owned()
                    }
                    _ => String::new(),
                };
                if ns == DAV && local == "allprop" {
                    all = true;
                }
                if prop_depth == Some(depth) {
                    if local.is_empty()
                        || !local
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                    {
                        return Err(bad("Invalid DAV property name"));
                    }
                    props.push((ns.clone(), local.clone()));
                    if props.len() > 128 {
                        return Err(bad("Too many DAV properties"));
                    }
                }
                if ns == DAV && local == "prop" {
                    prop_depth = Some(depth + 1);
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if prop_depth.is_some_and(|p| depth < p) {
                    prop_depth = None;
                }
            }
            Event::DocType(_) => return Err(bad("DTD declarations are not accepted")),
            Event::Eof => break,
            _ => {}
        }
    }
    if all || props.is_empty() {
        Ok(defaults())
    } else {
        Ok(props)
    }
}

fn defaults() -> Vec<(String, String)> {
    [
        (DAV, "resourcetype"),
        (DAV, "getetag"),
        (DAV, "getlastmodified"),
        (DAV, "getcontentlength"),
        (DAV, "getcontenttype"),
        (DAV, "quota-available-bytes"),
        (DAV, "quota-used-bytes"),
        (OC, "fileid"),
        (OC, "id"),
        (OC, "permissions"),
        (OC, "size"),
    ]
    .into_iter()
    .map(|(ns, name)| (ns.into(), name.into()))
    .collect()
}

fn property(c: &Client, node: &Node, ns: &str, name: &str) -> Option<String> {
    Some(match (ns, name) {
        (DAV, "resourcetype") => {
            if node.directory {
                "<d:collection/>".into()
            } else {
                String::new()
            }
        }
        (DAV, "getetag") => escape(&format!("\"{}\"", node.etag)),
        (DAV, "getlastmodified") => httpdate::fmt_http_date(
            std::time::UNIX_EPOCH
                + std::time::Duration::from_secs(node.modified.min(253_402_300_799)),
        ),
        (DAV, "getcontentlength") if !node.directory => node.size.to_string(),
        (DAV, "getcontenttype") => if node.directory {
            "httpd/unix-directory"
        } else {
            "application/octet-stream"
        }
        .into(),
        (DAV, "quota-available-bytes") => "-3".into(),
        (DAV, "quota-used-bytes") | (OC, "size") => node.size.to_string(),
        (OC, "fileid") => node.id.to_string(),
        (OC, "id") => format!("{:08}pgbridge", node.id),
        (OC, "permissions") => if node.directory { "GDNVCK" } else { "GDNVW" }.into(),
        (OC, "checksums") if !node.directory => format!(
            "<oc:checksum>SHA256:{}</oc:checksum>",
            escape(node.etag.split('-').next().unwrap_or_default())
        ),
        (OC, "owner-id") | (OC, "owner-display-name") => escape(&c.user),
        (OC, "data-fingerprint") if node.path.is_empty() => escape(&c.device),
        (NC, "is-encrypted") | (NC, "is-mount-root") | (NC, "has-preview") => "false".into(),
        (OC, "share-types") | (NC, "share-attributes") => String::new(),
        _ => return None,
    })
}

fn response(c: &Client, node: &Node, props: &[(String, String)], unsupported: bool) -> String {
    response_at(
        c,
        node,
        props,
        unsupported,
        &href(&c.user, &node.path, node.directory),
        false,
    )
}

fn response_at(
    c: &Client,
    node: &Node,
    props: &[(String, String)],
    unsupported: bool,
    uri: &str,
    temporary: bool,
) -> String {
    let mut good = String::new();
    let mut missing = String::new();
    for (ns, name) in props {
        let value = if unsupported || temporary && ns != DAV {
            None
        } else {
            property(c, node, ns, name)
        };
        let tag = format!("p:{name} xmlns:p=\"{}\"", escape(ns));
        if let Some(value) = value {
            good.push_str(&format!("<{tag}>{value}</p:{name}>"));
        } else {
            missing.push_str(&format!("<{tag}/>"));
        }
    }
    let mut xml = format!("<d:response><d:href>{}</d:href>", escape(uri));
    for (values, status) in [(good, 200), (missing, 404)] {
        if !values.is_empty() {
            xml.push_str(&format!("<d:propstat><d:prop>{values}</d:prop><d:status>HTTP/1.1 {status} {}</d:status></d:propstat>",if status == 200 { "OK" } else { "Not Found" }));
        }
    }
    xml.push_str("</d:response>");
    xml
}

fn multistatus(body: String) -> Response {
    let mut response = (StatusCode::MULTI_STATUS,format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><d:multistatus xmlns:d=\"DAV:\" xmlns:oc=\"http://owncloud.org/ns\">{body}</d:multistatus>")).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    response
}

pub fn listing(
    c: &Client,
    index: &Index,
    node: &Node,
    depth: &str,
    body: &[u8],
) -> Result<Response> {
    let props = properties(body)?;
    let mut body = response(c, node, &props, false);
    if depth == "1" && node.directory {
        for child in index.nodes.values().filter(|n| {
            n.path != node.path
                && index::under(&n.path, &node.path)
                && n.path.rsplit_once('/').map_or("", |(p, _)| p) == node.path
        }) {
            body.push_str(&response(c, child, &props, false));
        }
    }
    Ok(multistatus(body))
}

pub fn unsupported_patch(c: &Client, node: &Node, body: &[u8]) -> Result<Response> {
    Ok(multistatus(response(c, node, &properties(body)?, true)))
}

/// Lists resumable chunks without granting download access or inventing file IDs.
pub fn upload_listing(c: &Client, id: &str, entries: &[Node], body: &[u8]) -> Result<Response> {
    let props = properties(body)?;
    let mut xml = String::new();
    for node in entries {
        let uri = format!("/remote.php/dav/uploads/{}/{id}/{}", c.user, node.path);
        xml.push_str(&response_at(c, node, &props, false, &uri, true));
    }
    Ok(multistatus(xml))
}
