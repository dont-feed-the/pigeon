//! The pure core of pigeon: roster, authority, catalogue, states, journals.
//!
//! It performs no I/O and depends on no crate that does (14 §14.6, spec 14 §14.6).
//! Each module is an algebra: its types, pure functions, and laws stated in
//! its documentation and checked by hegel-rust property tests in `tests/`
//! (ADR 0012). 13 §13.13 lists the laws, their tests and the TLA+ models
//! they mirror.
#![forbid(unsafe_code)]

pub mod authority;
pub mod capacity;
pub mod catalogue;
pub mod commit;
pub mod consent;
pub mod export;
pub mod filter;
pub mod headline;
pub mod held;
pub mod journal;
pub mod names;
pub mod notify;
pub mod proposal;
pub mod queue;
pub mod report;
pub mod roster;
pub mod sig;
pub mod state;
pub mod time;
#[cfg(feature = "tla-trace")]
pub mod trace;
