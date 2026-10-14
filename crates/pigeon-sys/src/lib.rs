//! The platform layer of pigeon: everything that talks to the operating system
//! below git and git-annex (14 §14.6, ADRs 0037, 0040, 0049).
//!
//! - [`fs`]: stat tuples, renames that never replace a name, durable writes and
//!   appends, reads that leave the page cache alone;
//! - [`writers`]: the writer scan of install (ADR 0040);
//! - [`signal`]: pausing, resuming and stopping children (ADR 0037);
//! - [`pressure`]: the system's I/O pressure (ADR 0037);
//! - [`paths`]: where pigeon keeps its files (11 §11.2, §11.3);
//! - [`fingerprint`]: failure-domain fingerprints (ADR 0049);
//! - [`lab`]: the two seams of the lab build (ADR 0057), inert in a release.
//!
//! This is the only crate allowed `unsafe`, each block with a `SAFETY:` comment.

pub mod error;
pub mod fingerprint;
pub mod fs;
pub mod lab;
pub mod paths;
pub mod pressure;
pub mod signal;
pub mod writers;

pub use error::{Error, Result};
