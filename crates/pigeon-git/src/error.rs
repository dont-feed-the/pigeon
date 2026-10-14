//! Errors of the git and git-annex drivers. A failure keeps the command, its
//! exit status and the end of its stderr (03 §3.7, ADR 0023).

use std::time::Duration;

/// The result of a driver call.
pub type Result<T> = std::result::Result<T, Error>;

/// How much of a failed command's stderr is kept: its last 4 KiB.
pub const STDERR_TAIL: usize = 4096;

/// A failed call to git or git-annex.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The command is not one pigeon may run (03 §3.7).
    #[error("refused {argv:?}: {rule}")]
    Refused {
        /// The arguments after `git`.
        argv: Vec<String>,
        /// The rule it breaks.
        rule: &'static str,
    },
    /// The process could not start.
    #[error("spawn {argv:?}")]
    Spawn {
        /// The arguments after `git`.
        argv: Vec<String>,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// Reading from or writing to the process failed.
    #[error("pipe {argv:?}")]
    Pipe {
        /// The arguments after `git`.
        argv: Vec<String>,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// The call took longer than its deadline; the process was killed.
    #[error("{argv:?} timed out after {after:?}")]
    Timeout {
        /// The arguments after `git`.
        argv: Vec<String>,
        /// The deadline.
        after: Duration,
    },
    /// The process exited unsuccessfully.
    #[error("{argv:?} exited with {status:?}: {stderr_tail}")]
    Failed {
        /// The arguments after `git`.
        argv: Vec<String>,
        /// The exit code, or `None` if a signal ended it.
        status: Option<i32>,
        /// The end of its stderr.
        stderr_tail: String,
    },
    /// The output was not what the contract tests pin.
    #[error("{argv:?}: unexpected output: {detail}")]
    Parse {
        /// The arguments after `git`.
        argv: Vec<String>,
        /// What was wrong.
        detail: String,
    },
    /// git or git-annex is older than the oldest version CI tests (14 §14.3).
    #[error("{program} {found} is older than {minimum}")]
    TooOld {
        /// `git` or `git-annex`.
        program: &'static str,
        /// The version found.
        found: String,
        /// The minimum.
        minimum: String,
    },
}

/// The last [`STDERR_TAIL`] bytes of `stderr`, as text.
pub fn tail(stderr: &[u8]) -> String {
    let start = stderr.len().saturating_sub(STDERR_TAIL);
    String::from_utf8_lossy(&stderr[start..])
        .trim_end()
        .to_owned()
}
