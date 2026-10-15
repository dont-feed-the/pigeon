//! `pigeon-lab`: list, show and run the slice's scenarios (13 §13.12).
//!
//! - `pigeon-lab list`
//! - `pigeon-lab show ID [--json]`
//! - `pigeon-lab model [ID…]`: run in the model.
//! - `pigeon-lab run [--bin DIR] [--work DIR] [--out DIR] [--with-blocked]
//!   [--isolation auto|namespaces|images|dirs] [ID…]`: run with the real
//!   binaries, built with `--features lab,tla-trace`; writes one report per
//!   scenario and `summary.md` to `--out`. Blocked scenarios (slice.rs)
//!   are reported, and run only with `--with-blocked`.
//! - `pigeon-lab spaces [--isolation …]`: checks this host's isolation: a
//!   sized disk fills, a custodian's system directories land on its disk.
//!
//! No ID means every scenario; `--exclude ID` removes one (or the S9 family).
//! `--only-blocked` selects and enables only blocked scenarios. These filters
//! also apply to list, show and model. Exit status is 1 when a check fails.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use pigeon_lab::exec::host::Isolation;
use pigeon_lab::exec::{Config, Exec, unrunnable};
use pigeon_lab::model::ModelRun;
use pigeon_lab::run::{Report, Verdict, run};
use pigeon_lab::scenario::Scenario;
use pigeon_lab::slice;

fn out(text: &str) {
    let _ = writeln!(std::io::stdout().lock(), "{text}");
}

fn pick(ids: &[String]) -> Result<Vec<Scenario>, String> {
    let all = slice::all();
    if ids.is_empty() {
        return Ok(all);
    }
    let mut picked = Vec::new();
    for id in ids {
        let mut found: Vec<Scenario> = all
            .iter()
            .filter(|s| s.id == *id || (id == "S9" && s.id.starts_with("S9-")))
            .cloned()
            .collect();
        if found.is_empty() {
            return Err(format!("no scenario {id}"));
        }
        picked.append(&mut found);
    }
    Ok(picked)
}

/// Filters used by both CI partitions and the blocked-only full run.
fn select(
    ids: &[String],
    excluded: &[String],
    only_blocked: bool,
    blocked: impl Fn(&str) -> bool,
) -> Result<Vec<Scenario>, String> {
    // Validate exclusions too: a typo must not silently repeat a partition.
    let excluded = if excluded.is_empty() {
        Vec::new()
    } else {
        pick(excluded)?
    };
    Ok(pick(ids)?
        .into_iter()
        .filter(|s| !excluded.iter().any(|e| e.id == s.id))
        .filter(|s| !only_blocked || blocked(&s.id))
        .collect())
}

fn line(r: &Report) -> String {
    let fails = r
        .checks
        .iter()
        .filter(|c| matches!(c.verdict, Verdict::Fail(_)))
        .count();
    let skips = r
        .checks
        .iter()
        .filter(|c| matches!(c.verdict, Verdict::Skip(_)))
        .count();
    let verdict = if r.passed() { "pass" } else { "FAIL" };
    let mut text = format!(
        "| {} | {} | {verdict} | {} checks, {fails} failed, {skips} skipped |",
        r.scenario,
        r.interpreter,
        r.checks.len()
    );
    if let Some(f) = r.failure() {
        text.push_str(&format!(" step {}: {} — {:?}", f.step, f.what, f.verdict));
    }
    text
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match go(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            let _ = writeln!(std::io::stderr().lock(), "pigeon-lab: {e}");
            ExitCode::from(2)
        }
    }
}

fn go(args: &[String]) -> Result<bool, String> {
    let (cmd, rest) = args
        .split_first()
        .ok_or("usage: pigeon-lab list|show|model|run …")?;
    let mut ids = Vec::new();
    let mut excluded = Vec::new();
    let mut only_blocked = false;
    let (mut bin, mut work, mut outdir) = (
        PathBuf::from("target/lab/bin"),
        PathBuf::from("target/lab/work"),
        PathBuf::from("target/lab/out"),
    );
    let mut isolation = Isolation::detect();
    let mut json = false;
    let mut with_blocked = false;
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--bin" => bin = PathBuf::from(value()?),
            "--work" => work = PathBuf::from(value()?),
            "--out" => outdir = PathBuf::from(value()?),
            "--json" => json = true,
            "--exclude" => excluded.push(value()?),
            "--only-blocked" => {
                only_blocked = true;
                with_blocked = true;
            }
            "--with-blocked" => with_blocked = true,
            "--isolation" => {
                isolation = match value()?.as_str() {
                    "auto" => Isolation::detect(),
                    "namespaces" => Isolation::Namespaces,
                    "images" => Isolation::Images,
                    "dirs" => Isolation::Dirs,
                    other => return Err(format!("no isolation {other}")),
                }
            }
            id => ids.push(id.to_owned()),
        }
    }
    let selected = || {
        select(&ids, &excluded, only_blocked, |id| {
            slice::blocked(id).is_some()
        })
    };
    match cmd.as_str() {
        "list" => {
            for s in selected()? {
                out(&format!("{:<28} {}", s.id, s.title));
            }
            Ok(true)
        }
        "show" => {
            for s in selected()? {
                if json {
                    out(&serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?);
                } else {
                    out(&s.to_string());
                }
            }
            Ok(true)
        }
        "model" => {
            let mut ok = true;
            for s in selected()? {
                let r = run(&s, &mut ModelRun::default());
                ok &= r.passed();
                out(&line(&r));
            }
            Ok(ok)
        }
        "run" => {
            let scenarios = selected()?;
            std::fs::create_dir_all(&outdir).map_err(|e| e.to_string())?;
            if scenarios.is_empty() {
                std::fs::write(
                    outdir.join("summary.md"),
                    "# Slice scenarios\n\nNo scenarios selected.\n",
                )
                .map_err(|e| e.to_string())?;
                return Ok(true);
            }
            let bin = std::fs::canonicalize(&bin).map_err(|e| format!("{}: {e}", bin.display()))?;
            std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
            let work = std::fs::canonicalize(&work).map_err(|e| e.to_string())?;
            let outdir = std::fs::canonicalize(&outdir).map_err(|e| e.to_string())?;
            let mut ok = true;
            let mut summary = format!(
                "# Slice scenarios ({}, {})\n\n| Scenario | Interpreter | Verdict | Checks | Time |\n|---|---|---|---|---|\n",
                isolation.name(),
                std::env::consts::OS
            );
            for s in scenarios {
                let row = if let Some(why) = unrunnable(&s, isolation) {
                    format!("| {} | exec {} | skip | {why} | |", s.id, isolation.name())
                } else if let Some(why) = slice::blocked(&s.id).filter(|_| !with_blocked) {
                    format!(
                        "| {} | exec {} | blocked | {why} | |",
                        s.id,
                        isolation.name()
                    )
                } else {
                    let cfg = Config {
                        bin: bin.clone(),
                        work: work.clone(),
                        out: outdir.clone(),
                        isolation,
                    };
                    let started = std::time::Instant::now();
                    let r = run(&s, &mut Exec::new(cfg, &s.id));
                    // How long each scenario takes, for scripts/ci-timings.
                    let took = format!(" {} s |", started.elapsed().as_secs());
                    let blocked = slice::blocked(&s.id);
                    ok &= r.passed() || blocked.is_some();
                    let text = serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?;
                    std::fs::write(outdir.join(format!("{}.json", s.id)), text)
                        .map_err(|e| e.to_string())?;
                    let row = match (r.passed(), blocked) {
                        (false, Some(why)) => format!(
                            "{} blocked: {why}",
                            line(&r).replacen("| FAIL |", "| blocked |", 1)
                        ),
                        (true, Some(_)) => {
                            format!("{} (no longer blocked: unmark it in slice.rs)", line(&r))
                        }
                        _ => line(&r),
                    };
                    if row.ends_with('|') {
                        format!("{row}{took}")
                    } else {
                        format!("{row} |{took}")
                    }
                };
                out(&row);
                summary.push_str(&row);
                summary.push('\n');
            }
            std::fs::write(outdir.join("summary.md"), summary).map_err(|e| e.to_string())?;
            Ok(ok)
        }
        "spaces" => spaces(isolation, &work),
        other => Err(format!("no command {other}")),
    }
}

/// Checks the isolation of this host end to end, without pigeond.
fn spaces(isolation: Isolation, work: &std::path::Path) -> Result<bool, String> {
    use pigeon_lab::exec::host::{SERVICE_DIRS, Space};
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let work = std::fs::canonicalize(work)
        .map_err(|e| e.to_string())?
        .join("spaces");
    let _ = pigeon_lab::exec::host::remove_tree(&work);
    let mut laptop = Space::create(isolation, &work, "laptop", 64, false)?;
    let mut custodian = Space::create(isolation, &work, "custodian", 64, true)?;
    let mut ok = true;
    let mut check = |what: &str, good: bool| {
        out(&format!("{} {what}", if good { "ok  " } else { "FAIL" }));
        ok &= good;
    };
    let free = laptop.free_bytes()?;
    if isolation.sized() {
        check(
            &format!("a 64 MiB disk has {} MiB free", free >> 20),
            free <= 64 << 20 && free > 32 << 20,
        );
        let ballast = laptop.root.join("ballast");
        let wrote = std::fs::write(&ballast, vec![0u8; 80 << 20]).is_err();
        check("writing 80 MiB on it fails", wrote);
        let _ = std::fs::remove_file(&ballast);
    }
    let (root, env) = if isolation.service_seam() {
        (
            custodian.root.join("service"),
            vec![(
                "ROOT".to_owned(),
                custodian.root.join("service").display().to_string(),
            )],
        )
    } else {
        (
            std::path::PathBuf::from("/"),
            vec![("ROOT".to_owned(), "/".to_owned())],
        )
    };
    let script = format!("echo here > \"$ROOT/{}/probe\"", SERVICE_DIRS[0]);
    let mut cmd = custodian.command(
        std::path::Path::new("/bin/sh"),
        &["-c".to_owned(), script],
        &env,
    );
    let ran = cmd.status().is_ok_and(|s| s.success());
    let landed = std::fs::read_to_string(
        custodian
            .root
            .join("service")
            .join(SERVICE_DIRS[0])
            .join("probe"),
    )
    .is_ok();
    check(
        &format!(
            "a custodian's {} ({}) lands on its disk",
            SERVICE_DIRS[0],
            root.display()
        ),
        ran && landed,
    );
    if isolation == Isolation::Namespaces {
        let host = std::path::Path::new("/")
            .join(SERVICE_DIRS[0])
            .join("probe");
        check("and not on the host's", !host.exists());
    }
    laptop.teardown();
    custodian.teardown();
    Ok(ok)
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn s5_and_rest_cover_every_scenario_once() {
        let s5 = select(&["S5".into()], &[], false, |_| false).unwrap();
        let rest = select(&[], &["S5".into()], false, |_| false).unwrap();
        assert_eq!(s5.len(), 1);
        assert_eq!(s5.len() + rest.len(), slice::all().len());
        assert!(rest.iter().all(|s| s.id != "S5"));
    }

    #[test]
    fn blocked_selection_filters_and_can_be_empty() {
        let blocked = select(&[], &[], true, |id| id == "S5").unwrap();
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].id, "S5");
        assert!(select(&[], &[], true, |_| false).unwrap().is_empty());
        assert!(
            select(&[], &["S5".into()], true, |id| id == "S5")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn exclusions_support_families_and_reject_typos() {
        let rest = select(&[], &["S9".into()], false, |_| false).unwrap();
        assert!(rest.iter().all(|s| !s.id.starts_with("S9-")));
        assert!(rest.len() < slice::all().len());
        assert!(select(&[], &["S55".into()], false, |_| false).is_err());
    }
}
