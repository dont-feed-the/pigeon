//! The web interface as a browser reaches it (10 §10.3): a login link from
//! `pigeon open`, its session cookie, then pages, over plain HTTP/1.1.

use std::io::{Read, Write};
use std::net::TcpStream;

use super::{Daemon, pigeon, said};

/// One request; the answer's head and body.
fn http(port: &str, request: &str) -> (String, String) {
    let mut s = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap_or((&out, ""));
    (head.to_owned(), body.to_owned())
}

/// A logged-in session of `d`'s web interface: its port and cookie.
pub fn login(d: &Daemon) -> (String, String) {
    let open = pigeon(d, &["open"]);
    let url = said(&open, "web-login", "url").unwrap_or_else(|| panic!("open: {open:?}"));
    let rest = url.strip_prefix("http://127.0.0.1:").unwrap();
    let (port, path) = rest.split_once('/').unwrap();
    let (head, _) = http(
        port,
        &format!("GET /{path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"),
    );
    let cookie = head
        .lines()
        .find_map(|l| l.strip_prefix("set-cookie: "))
        .and_then(|c| c.split(';').next())
        .unwrap_or_else(|| panic!("login: {head}"));
    (port.to_owned(), cookie.to_owned())
}

/// The body of the page at `path`.
pub fn page(session: &(String, String), path: &str) -> String {
    let (port, cookie) = session;
    let (head, body) = http(
        port,
        &format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"
        ),
    );
    assert!(head.starts_with("HTTP/1.1 200"), "{path}: {head}\n{body}");
    body
}
