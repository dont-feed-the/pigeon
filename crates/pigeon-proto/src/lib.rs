//! Request and response types of pigeond's API and the socket framing.
//!
//! Types only, no I/O (ADR 0021): the CLI, the web interface and the
//! transport helper speak to `pigeond` with these, as newline-delimited JSON
//! over the local socket (09 §9.3), and devices open each iroh stream with
//! the header of [`stream`] (09 §9.8). Messages for people travel as
//! message ids and arguments, never as text (ADR 0028).

pub mod api;
pub mod join;
pub mod stream;
pub mod time;
pub mod wire;

pub use api::{Arg, Message, Problem, Request, Response};
pub use wire::{Envelope, LineDecoder, MAX_LINE, encode, socket_path};

/// The version of the socket protocol, answered to `Hello` (10.1): a client
/// of another version refuses with `version-mismatch`.
pub const PROTOCOL_VERSION: u32 = 1;
