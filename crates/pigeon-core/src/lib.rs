//! The pure core of pigeon: roster, authority, catalogue, states, journals.
//!
//! It performs no I/O and depends on no crate that does (14 §14.6, ADR 0058).
//! Each module is an algebra: its types, pure functions, and laws stated in
//! its documentation and checked by hegel-rust property tests in `tests/`
//! (ADR 0012). 13 §13.13 lists the laws, their tests and the TLA+ models
//! they mirror.
#![forbid(unsafe_code)]

pub mod domain;
pub mod filter;
pub mod headline;
pub mod journal;
pub mod queue;
pub mod roster;
pub mod state;
pub mod time;
#[cfg(feature = "tla-trace")]
pub mod trace;
