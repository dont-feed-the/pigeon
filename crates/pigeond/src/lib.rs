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
pub mod asides;
pub mod capture;
pub mod capture_file;
pub mod change;
pub mod clock_file;
pub mod config;
pub mod conflicts_file;
pub mod custody;
pub mod daemon;
pub mod decide;
pub mod decision_file;
pub mod evict;
pub mod fold;
pub mod folders;
pub mod git;
pub mod group;
pub mod held;
pub mod held_file;
pub mod history;
pub mod install;
pub mod invite;
pub mod join;
pub mod journal_file;
pub mod leave;
pub mod locks;
pub mod logs;
pub mod merge;
pub mod net;
pub mod notices;
pub mod pause;
pub mod prefs;
pub mod proposal;
pub mod recovery;
pub mod renames;
pub mod repair;
pub mod report;
pub mod report_file;
pub mod resolve;
pub mod restore;
pub mod retention;
pub mod roster_file;
pub mod settings;
pub mod status;
pub mod suggest;
pub mod thin;
pub mod trace;
pub mod util;
pub mod verify;
pub mod web;
