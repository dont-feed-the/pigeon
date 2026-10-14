//! The pigeon daemon's library: what `main.rs` wires together, and what the
//! integration tests drive.
//!
//! `net` is the device's iroh endpoint, the local socket of the transport
//! helper, and the filter between peers and git-annex (09 §9.2–9.4).

pub mod net;
