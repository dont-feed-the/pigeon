//! The transport helper git-annex runs to reach a peer over iroh (09 §9.2).
//!
//! git-annex's generic P2P transport runs `git-annex-p2p-iroh` in three
//! modes: `address` prints this device's ID; `<device>` relays git-annex's
//! P2P protocol on stdin and stdout to that peer; `<device> <socket>` serves
//! incoming peers on remotedaemon's unix socket. The helper is a thin
//! client of `pigeond` over the local socket (ADRs 0006, 0011, 09 §9.2): it
//! names its repository by path, and `pigeond` does the rest. It fails
//! at once, with a problem code on stderr, when `pigeond` cannot help.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

use pigeon_proto::{LineDecoder, PROTOCOL_VERSION, Request, Response, encode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            let _ = writeln!(std::io::stderr(), "git-annex-p2p-iroh: {code}");
            ExitCode::FAILURE
        }
    }
}

/// The repository git-annex runs the helper for: `GIT_DIR`, else the
/// working directory, as an absolute path.
fn repository() -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|_| "no-working-directory".to_owned())?;
    let path = match std::env::var_os("GIT_DIR") {
        Some(d) => cwd.join(d),
        None => cwd,
    };
    Ok(path.to_string_lossy().into_owned())
}

fn connect() -> Result<UnixStream, String> {
    let path = pigeon_sys::paths::client_socket();
    UnixStream::connect(path).map_err(|_| "daemon-not-running".to_owned())
}

/// Sends one request and reads its response; returns what follows it.
fn ask(stream: &mut UnixStream, request: &Request) -> Result<(Response, Vec<u8>), String> {
    let line = encode(1, request).map_err(|_| "internal".to_owned())?;
    stream
        .write_all(&line)
        .map_err(|_| "daemon-not-running".to_owned())?;
    let mut decoder = LineDecoder::new();
    let mut buf = [0u8; 4096];
    loop {
        if let Some(e) = decoder
            .next_line::<Response>()
            .map_err(|_| "version-mismatch".to_owned())?
        {
            return match e.body {
                Response::Problem(p) => Err(p.code),
                r => Ok((r, decoder.into_rest())),
            };
        }
        let n = stream
            .read(&mut buf)
            .map_err(|_| "daemon-not-running".to_owned())?;
        if n == 0 {
            return Err("daemon-not-running".to_owned());
        }
        decoder.push(&buf[..n]);
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let mut stream = connect()?;
    let hello = Request::Hello {
        version: PROTOCOL_VERSION,
    };
    match ask(&mut stream, &hello)? {
        (Response::Hello { version }, _) if version == PROTOCOL_VERSION => {}
        _ => return Err("version-mismatch".to_owned()),
    }
    let repository = repository()?;
    match args {
        [mode] if mode == "address" => match ask(&mut stream, &Request::Address { repository })? {
            (Response::Address { device }, _) => {
                let mut out = std::io::stdout();
                writeln!(out, "{device}").map_err(|_| "stdout".to_owned())
            }
            _ => Err("unexpected-response".to_owned()),
        },
        [device] => {
            let request = Request::Connect {
                repository,
                device: device.clone(),
            };
            match ask(&mut stream, &request)? {
                (Response::Connected, rest) => relay(stream, &rest),
                _ => Err("unexpected-response".to_owned()),
            }
        }
        [_, socket] => {
            let socket = std::env::current_dir()
                .map(|d| d.join(socket))
                .unwrap_or_else(|_| PathBuf::from(socket));
            let request = Request::Listen {
                repository,
                socket: socket.to_string_lossy().into_owned(),
            };
            match ask(&mut stream, &request)? {
                (Response::Listening, _) => {
                    let mut buf = [0u8; 64];
                    while stream.read(&mut buf).map(|n| n > 0).unwrap_or(false) {}
                    Ok(())
                }
                _ => Err("unexpected-response".to_owned()),
            }
        }
        _ => Err("usage".to_owned()),
    }
}

/// Copies stdin to the peer and the peer to stdout until the peer closes.
fn relay(stream: UnixStream, rest: &[u8]) -> Result<(), String> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(rest).map_err(|_| "stdout".to_owned())?;
    stdout.flush().map_err(|_| "stdout".to_owned())?;
    let mut up = stream.try_clone().map_err(|_| "socket".to_owned())?;
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin().lock(), &mut up);
        let _ = up.shutdown(std::net::Shutdown::Write);
    });
    let mut down = stream;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = down.read(&mut buf).map_err(|_| "peer-reset".to_owned())?;
        if n == 0 {
            return Ok(());
        }
        stdout
            .write_all(&buf[..n])
            .map_err(|_| "stdout".to_owned())?;
        stdout.flush().map_err(|_| "stdout".to_owned())?;
    }
}
