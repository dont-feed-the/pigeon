//! The framing of the local socket: one JSON object per line (09 §9.3).
//!
//! Every request and response is an [`Envelope`] whose `id` matches a
//! response to its request. After a `Connected` or `Listening` reply the
//! connection carries raw bytes, so the decoder hands back whatever it has
//! read past the last line ([`LineDecoder::into_rest`]).

use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The longest line accepted, its newline excluded: a longer one is an error.
pub const MAX_LINE: usize = 1 << 20;

/// One request or response, with the `id` that matches them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Chosen by the client, unique on its connection.
    pub id: u64,
    /// The request or response itself.
    #[serde(flatten)]
    pub body: T,
}

/// Errors of the framing.
#[derive(Debug, thiserror::Error)]
pub enum WireError {
    /// A line longer than [`MAX_LINE`].
    #[error("line longer than {MAX_LINE} bytes")]
    LineTooLong,
    /// A line that is not the JSON of the expected type.
    #[error("malformed message: {0}")]
    Malformed(#[from] serde_json::Error),
}

/// One line of JSON, newline included. JSON escapes every newline inside a
/// string, so the line holds exactly one.
pub fn encode<T: Serialize>(id: u64, body: &T) -> Result<Vec<u8>, WireError> {
    let mut line = serde_json::to_vec(&Envelope { id, body })?;
    line.push(b'\n');
    Ok(line)
}

/// Cuts a byte stream into lines, whatever the read sizes.
#[derive(Debug, Default)]
pub struct LineDecoder {
    buffer: Vec<u8>,
}

impl LineDecoder {
    /// A decoder with nothing read yet.
    pub fn new() -> Self {
        LineDecoder::default()
    }

    /// Adds bytes read from the connection.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole line, decoded, if one has arrived.
    pub fn next_line<T: DeserializeOwned>(&mut self) -> Result<Option<Envelope<T>>, WireError> {
        match self.buffer.iter().position(|&b| b == b'\n') {
            Some(i) if i > MAX_LINE => Err(WireError::LineTooLong),
            Some(i) => {
                let line: Vec<u8> = self.buffer.drain(..=i).collect();
                Ok(Some(serde_json::from_slice(&line[..i])?))
            }
            None if self.buffer.len() > MAX_LINE => Err(WireError::LineTooLong),
            None => Ok(None),
        }
    }

    /// The bytes read past the last line taken: the start of a raw stream.
    pub fn into_rest(self) -> Vec<u8> {
        self.buffer
    }
}

/// The local socket's path (09 §9.3), from the environment's values:
/// `PIGEON_SOCKET` if set (tests and the lab), else
/// `$XDG_RUNTIME_DIR/pigeon/pigeond.sock`, else the per-user temporary
/// directory `$TMPDIR` (macOS), else `/tmp`.
pub fn socket_path(
    pigeon_socket: Option<&str>,
    xdg_runtime_dir: Option<&str>,
    tmpdir: Option<&str>,
) -> std::path::PathBuf {
    if let Some(p) = pigeon_socket.filter(|p| !p.is_empty()) {
        return p.into();
    }
    let base = xdg_runtime_dir
        .filter(|d| !d.is_empty())
        .or(tmpdir.filter(|d| !d.is_empty()))
        .unwrap_or("/tmp");
    std::path::Path::new(base)
        .join("pigeon")
        .join("pigeond.sock")
}
