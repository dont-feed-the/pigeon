//! The platform layer of pigeon: everything that talks to the operating system
//! below git and git-annex (14 §14.6, ADRs 0037 and 0046, spec 07 §7.5).
//!
//! - [`fs`]: stat tuples, renames that never replace a name, durable writes and
//!   appends, reads that leave the page cache alone;
//! - [`writers`]: the writer scan of install (spec 07 §7.5);
//! - [`signal`]: pausing, resuming and stopping children (ADR 0037);
//! - [`pressure`]: the system's I/O pressure (ADR 0037);
//! - [`paths`]: where pigeon keeps its files (11 §11.2, §11.3);
//! - [`snapshot`]: private copies of a file, cloned where the file system can;
//! - [`lab`]: the two seams of the lab build (13 §13.12), inert in a release;
//! - [`clock`]: the local time zone's offset, for conflict names (05 §5.3);
//! - [`host`]: this computer's name, for a device given none (09 §9.6).
//!
//! This is the only crate allowed `unsafe`, each block with a `SAFETY:` comment.

pub mod clock;
pub mod error;
pub mod fs;
pub mod host;
pub mod lab;
pub mod paths;
pub mod pressure;
pub mod signal;
pub mod snapshot;
pub mod writers;

pub use error::{Error, Result};
