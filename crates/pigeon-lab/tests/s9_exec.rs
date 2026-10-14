//! S9 with random crash schedules against the real binaries (13 §13.12):
//! 50 per push, 1000 in models-full, as `HEGEL_TEST_CASES` says. It runs
//! only when `PIGEON_LAB_BIN` names the lab build's binaries. Each case
//! runs three daemons for a minute or more, so hegel's `TooSlow` health
//! check, which judges generation by wall time, is suppressed.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::PathBuf;

use hegel::{HealthCheck, TestCase};
use pigeon_lab::exec::host::Isolation;
use pigeon_lab::exec::{Config, Exec};
use pigeon_lab::run::run;
use pigeon_lab::slice;

#[hegel::test(suppress_health_check = [HealthCheck::TooSlow])]
fn random_crash_schedules_lose_and_duplicate_nothing(tc: TestCase) {
    let Ok(bin) = std::env::var("PIGEON_LAB_BIN") else {
        return;
    };
    // While the random schedules are blocked (slice::BLOCKED), they wait.
    if slice::blocked("S9-random").is_some() {
        return;
    }
    let out =
        PathBuf::from(std::env::var("PIGEON_LAB_OUT").unwrap_or_else(|_| "target/lab/s9".into()));
    let plan = common::schedule(&tc);
    let id = format!(
        "S9-random-{}",
        plan.iter()
            .map(|(i, p, n)| format!("{i}{}{n}", p.name()))
            .collect::<Vec<_>>()
            .join("+")
    );
    let cfg = Config {
        bin: std::fs::canonicalize(bin).unwrap(),
        // Short: unix sockets live under it (pigeon-lab's short_name).
        work: PathBuf::from("/tmp/lab-s9"),
        out,
        isolation: Isolation::detect(),
    };
    let report = run(&slice::s9(&id, &plan), &mut Exec::new(cfg, &id));
    assert!(report.passed(), "{plan:?}: {:?}", report.failure());
}
