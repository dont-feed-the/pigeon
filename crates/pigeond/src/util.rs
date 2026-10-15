//! Small helpers of `pigeond`: hex, random bytes, SHA-256 and HMAC, times
//! in RFC 3339 (UTC), the device key (09 §9.1), and a `join_all`.
//!
//! The device key is kept as the hex of its 32-byte ed25519 secret, mode
//! 0600, rather than in OpenSSH format (13 §13.12, first build).
//!
//! Laws (`tests/codecs.rs`): `unhex(hex(b)) = b`; `parse_rfc3339(rfc3339(t))
//! = t`; the HMAC matches RFC 4231.

use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use sha2::{Digest, Sha256};

/// Lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The bytes of a hex string, `None` if it is not hex.
pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// `N` random bytes from the operating system.
pub fn random<const N: usize>() -> io::Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(b)
}

/// SHA-256 of `bytes`, hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// HMAC-SHA256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        for (d, s) in k.iter_mut().zip(Sha256::digest(key).iter()) {
            *d = *s;
        }
    } else {
        for (d, s) in k.iter_mut().zip(key) {
            *d = *s;
        }
    }
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    let inner = Sha256::new()
        .chain_update(&ipad)
        .chain_update(message)
        .finalize();
    let outer = Sha256::new()
        .chain_update(&opad)
        .chain_update(inner)
        .finalize();
    let mut out = [0u8; 32];
    for (d, s) in out.iter_mut().zip(outer.iter()) {
        *d = *s;
    }
    out
}

/// Nanoseconds since the Unix epoch.
pub fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
}

/// Seconds since the Unix epoch.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

pub use pigeon_proto::time::{date, parse_rfc3339, rfc3339};

/// Writes `bytes` to `path` atomically, readable by the user only.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)
}

/// The device key at `path`, created if there is none.
pub fn device_key(path: &Path) -> io::Result<[u8; 32]> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let bytes =
                unhex(text.trim()).ok_or_else(|| io::Error::other("device key: not hex"))?;
            bytes
                .try_into()
                .map_err(|_| io::Error::other("device key: not 32 bytes"))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let key = random::<32>()?;
            write_private(path, format!("{}\n", hex(&key)).as_bytes())?;
            Ok(key)
        }
        Err(e) => Err(e),
    }
}

/// Runs `futures` concurrently on this task and answers their outputs in
/// order, as `futures::future::join_all` does.
pub async fn join_all<F: std::future::Future>(futures: Vec<F>) -> Vec<F::Output> {
    let mut futures: Vec<std::pin::Pin<Box<F>>> = futures.into_iter().map(Box::pin).collect();
    let mut out: Vec<Option<F::Output>> = futures.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut pending = false;
        for (f, slot) in futures.iter_mut().zip(out.iter_mut()) {
            if slot.is_none() {
                match f.as_mut().poll(cx) {
                    std::task::Poll::Ready(v) => *slot = Some(v),
                    std::task::Poll::Pending => pending = true,
                }
            }
        }
        if pending {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    })
    .await;
    out.into_iter().flatten().collect()
}
