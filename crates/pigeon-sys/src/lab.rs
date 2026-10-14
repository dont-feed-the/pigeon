//! The two seams of the lab build (ADR 0057), and nothing else: failure-domain
//! fingerprints read from a file, and named crash points.
//!
//! Without the `lab` feature both are inert: [`crash_point`] does nothing and
//! [`fingerprints_file`] is `None`, so a release build behaves as if this
//! module did not exist. [`LAB`] says which build this is, for `doctor` and
//! for the `lab = true` mark of status reports.

use std::path::PathBuf;

use crate::{Error, Result};

/// Whether this is a lab build (`cargo build --features lab`).
pub const LAB: bool = cfg!(feature = "lab");

/// The variable naming the crash point: `PIGEON_LAB_CRASH=point:n`.
pub const CRASH_VAR: &str = "PIGEON_LAB_CRASH";

/// The variable naming the fingerprints file of a lab build.
pub const FINGERPRINTS_VAR: &str = "PIGEON_LAB_FINGERPRINTS";

/// A crash plan: abort at the `n`-th crossing of `point` (counted from 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrashPlan {
    /// The point's name, such as `install.after-begin`.
    pub point: String,
    /// Which crossing aborts.
    pub n: u64,
}

/// Parses `point:n`. The point is a non-empty name of letters, digits, `.`,
/// `-` and `_`; `n` is at least 1.
pub fn parse_crash_plan(text: &str) -> Result<CrashPlan> {
    let bad = |detail: &str| Error::Malformed {
        what: CRASH_VAR,
        detail: detail.to_owned(),
    };
    let (point, n) = text
        .rsplit_once(':')
        .ok_or_else(|| bad("expected point:n"))?;
    let name_ok = !point.is_empty()
        && point
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if !name_ok {
        return Err(bad("the point is not a name"));
    }
    match n.parse::<u64>() {
        Ok(n) if n >= 1 => Ok(CrashPlan {
            point: point.to_owned(),
            n,
        }),
        _ => Err(bad("n is not a positive integer")),
    }
}

/// Checks the lab variables at start, so that `pigeond` refuses a malformed
/// plan instead of silently never crashing. Returns the plan, if any. In a
/// release build the variables are ignored and this returns `None`.
pub fn check_env() -> Result<Option<CrashPlan>> {
    if !LAB {
        return Ok(None);
    }
    match std::env::var(CRASH_VAR) {
        Ok(text) => parse_crash_plan(&text).map(Some),
        Err(_) => Ok(None),
    }
}

/// Crosses the named crash point. In a lab build whose plan names `point`,
/// the `n`-th crossing kills the process with `SIGKILL`, as `kill -9` would:
/// no destructor runs and no buffer is flushed. Otherwise it does nothing.
#[inline]
pub fn crash_point(point: &'static str) {
    #[cfg(feature = "lab")]
    imp::cross(point);
    #[cfg(not(feature = "lab"))]
    let _ = point;
}

/// The fingerprints file of a lab build (`PIGEON_LAB_FINGERPRINTS`), read by
/// [`crate::fingerprint`] instead of probing the machine. `None` in a release
/// build, whatever the environment says.
pub fn fingerprints_file() -> Option<PathBuf> {
    if !LAB {
        return None;
    }
    std::env::var_os(FINGERPRINTS_VAR).map(PathBuf::from)
}

#[cfg(feature = "lab")]
mod imp {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{CRASH_VAR, CrashPlan, parse_crash_plan};

    static PLAN: OnceLock<Option<CrashPlan>> = OnceLock::new();
    static CROSSINGS: AtomicU64 = AtomicU64::new(0);

    pub(super) fn cross(point: &'static str) {
        let plan = PLAN.get_or_init(|| {
            std::env::var(CRASH_VAR)
                .ok()
                .and_then(|t| parse_crash_plan(&t).ok())
        });
        let Some(plan) = plan else { return };
        if plan.point != point {
            return;
        }
        if CROSSINGS.fetch_add(1, Ordering::SeqCst) + 1 == plan.n {
            let me = rustix::process::getpid();
            if rustix::process::kill_process(me, rustix::process::Signal::KILL).is_ok() {
                // On macOS the signal may land after `kill` returns, so the
                // thread waits for it rather than abort first (CI saw it).
                for _ in 0..500 {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_parse_and_malformed_ones_are_refused() {
        let plan = parse_crash_plan("install.after-begin:3").unwrap();
        assert_eq!(
            plan,
            CrashPlan {
                point: "install.after-begin".into(),
                n: 3
            }
        );
        for bad in [
            "", "point", ":1", "point:0", "point:-1", "point:x", "a b:1", "a:b:2",
        ] {
            assert!(parse_crash_plan(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn a_release_build_has_no_seams() {
        if !LAB {
            assert_eq!(fingerprints_file(), None);
            assert_eq!(check_env().unwrap(), None);
            crash_point("never");
        }
    }
}
