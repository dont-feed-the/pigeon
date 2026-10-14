//! Errors of the platform layer: the operation, the path and the cause
//! (ADR 0023).

use std::io;
use std::path::PathBuf;

/// The result of an operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// A failed system operation. The cause is kept as the source.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A system call on a path failed.
    #[error("{op} {path:?}")]
    Path {
        /// The operation, such as `rename`.
        op: &'static str,
        /// The path it was applied to.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// A system call on no particular path failed.
    #[error("{op}")]
    Op {
        /// The operation, such as `kill`.
        op: &'static str,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// A file pigeon reads had a form it does not know.
    #[error("{what}: {detail}")]
    Malformed {
        /// What was read, such as `PIGEON_LAB_CRASH`.
        what: &'static str,
        /// What was wrong with it.
        detail: String,
    },
}

impl Error {
    pub(crate) fn path(op: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Error::Path {
            op,
            path: path.into(),
            source,
        }
    }

    pub(crate) fn op(op: &'static str, source: impl Into<io::Error>) -> Self {
        Error::Op {
            op,
            source: source.into(),
        }
    }

    /// The underlying I/O error kind, if any.
    pub fn kind(&self) -> Option<io::ErrorKind> {
        match self {
            Error::Path { source, .. } | Error::Op { source, .. } => Some(source.kind()),
            Error::Malformed { .. } => None,
        }
    }
}
