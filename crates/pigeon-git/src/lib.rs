//! Drivers for git and git-annex: the only crate that runs them (ADRs 0004,
//! 0010). Every process gets pigeon's environment ([`env`], spec 03 §3.7) and
//! passes the allow-list of 03 §3.7 ([`allow`]) before it starts.
//!
//! - [`run`]: one-shot calls with a deadline;
//! - [`batch`]: long-running `--batch` processes, restarted after a failure;
//! - [`catfile`]: `git cat-file --batch`;
//! - [`object_reply`]: complete ordered one-shot object and size replies;
//! - [`annex`]: the JSON answers of git-annex and its batch processes;
//! - [`status`]: `git status --porcelain=v2 -z`;
//! - [`version`]: the minimum versions of 14 §14.3.

pub mod allow;
pub mod annex;
pub mod batch;
pub mod catfile;
pub mod env;
pub mod error;
pub mod object_reply;
mod progress;
pub mod run;
pub mod status;
pub mod version;
pub mod work;

pub use env::GitEnv;
pub use error::{Error, Result};
pub use run::{DEFAULT_TIMEOUT, Output, Repo};
