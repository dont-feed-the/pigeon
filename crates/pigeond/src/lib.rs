//! The pigeon daemon: its loops, the iroh endpoint and the local socket.
//!
//! [`daemon::run`] is `pigeond`'s main: directories, settings, device key,
//! endpoint, socket, logs, and the rounds of the loops of the safety slice
//! (13 §13.12): capture, accept, install (keep), place, repair,
//! free and keep, and the settings check. Each loop takes its decisions
//! from pigeon-core and does I/O only through pigeon-git, pigeon-sys and
//! [`net`].

pub mod admission;
pub mod api;
pub mod capture;
pub mod change;
pub mod clock_file;
pub mod config;
pub mod custody;
pub mod daemon;
pub mod evict;
pub mod git;
pub mod group;
pub mod history;
pub mod invite;
pub mod journal_file;
pub mod locks;
pub mod logs;
pub mod net;
pub mod recovery;
pub mod repair;
pub mod report;
pub mod report_file;
pub mod restore;
pub mod roster_file;
pub mod settings;
pub mod status;
pub mod thin;
pub mod trace;
pub mod util;
pub mod verify;
