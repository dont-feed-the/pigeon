//! S9 with random crash schedules against the real binaries (13 §13.12):
//! 50 per push, 1000 in models-full, as `HEGEL_TEST_CASES` says. It runs
//! only when `PIGEON_LAB_BIN` names the lab build's binaries, and while
//! `S9-random` is blocked only when `PIGEON_LAB_WITH_BLOCKED` is set. Each case
//! runs three daemons for a minute or more, so hegel's `TooSlow` health
//! check, which judges generation by wall time, is suppressed. Each case
//! writes its logs under a name numbered in run order, so that hegel's
//! replays of a failing case do not overwrite them, and a failure is
//! printed and appended to `failures.txt` at once: hegel prints no message
//! for a failure its replays do not confirm.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use hegel::{HealthCheck, TestCase};
use pigeon_lab::exec::host::Isolation;
use pigeon_lab::exec::{Config, Exec};
use pigeon_lab::run::run;
use pigeon_lab::slice;

/// The cases run so far in this process.
static CASES: AtomicUsize = AtomicUsize::new(0);

#[hegel::test(suppress_health_check = [HealthCheck::TooSlow])]
fn random_crash_schedules_lose_and_duplicate_nothing(tc: TestCase) {
    let Ok(bin) = std::env::var("PIGEON_LAB_BIN") else {
        return;
    };
    // While the random schedules are blocked (slice::BLOCKED), they run only
    // when asked, like `pigeon-lab run --with-blocked`.
    if slice::blocked("S9-random").is_some()
        && std::env::var_os("PIGEON_LAB_WITH_BLOCKED").is_none()
    {
        return;
    }
    let out =
        PathBuf::from(std::env::var("PIGEON_LAB_OUT").unwrap_or_else(|_| "target/lab/s9".into()));
    let plan = common::schedule(&tc);
    let n = CASES.fetch_add(1, Ordering::SeqCst);
    let id = format!(
        "S9-random-{n:03}-{}",
        plan.iter()
            .map(|(i, p, n)| format!("{i}{}{n}", p.name()))
            .collect::<Vec<_>>()
            .join("+")
    );
    let cfg = Config {
        bin: std::fs::canonicalize(bin).unwrap(),
        // Short: unix sockets live under it (pigeon-lab's short_name).
        work: PathBuf::from(
            std::env::var("PIGEON_LAB_WORK").unwrap_or_else(|_| "/tmp/lab-s9".into()),
        ),
        out: out.clone(),
        isolation: Isolation::detect(),
    };
    let started = std::time::Instant::now();
    let report = run(&slice::s9(&id, &plan), &mut Exec::new(cfg, &id));
    // How long each schedule takes, for scripts/ci-timings.
    let took = format!("{id} {}\n", started.elapsed().as_secs());
    let _ = std::fs::create_dir_all(&out);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join("timings.txt"))
        .and_then(|mut f| std::io::Write::write_all(&mut f, took.as_bytes()));
    if !report.passed() {
        let line = format!("{id}: {plan:?}: {:?}\n", report.failure());
        eprint!("FAILED {line}");
        let _ = std::fs::create_dir_all(&out);
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(out.join("failures.txt"))
            .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
    }
    assert!(report.passed(), "{plan:?}: {:?}", report.failure());
}
