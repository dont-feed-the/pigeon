//! `pigeon-lab`: list, show and run the slice's scenarios (13 §13.12).
//!
//! - `pigeon-lab list`
//! - `pigeon-lab show ID [--json]`
//! - `pigeon-lab model [ID…]`: run in the model.
//! - `pigeon-lab run [--bin DIR] [--work DIR] [--out DIR]
//!   [--isolation auto|namespaces|images|dirs] [ID…]`: run with the real
//!   binaries, built with `--features lab,tla-trace`; writes one report per
//!   scenario and `summary.md` to `--out`.
//!
//! No ID means every scenario. The exit status is 1 when a check fails.

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
    ids.iter()
        .map(|id| {
            all.iter()
                .find(|s| s.id == *id || (id == "S9" && s.id.starts_with("S9-")))
                .cloned()
                .ok_or_else(|| format!("no scenario {id}"))
        })
        .collect()
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
    let (mut bin, mut work, mut outdir) = (
        PathBuf::from("target/lab/bin"),
        PathBuf::from("target/lab/work"),
        PathBuf::from("target/lab/out"),
    );
    let mut isolation = Isolation::detect();
    let mut json = false;
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--bin" => bin = PathBuf::from(value()?),
            "--work" => work = PathBuf::from(value()?),
            "--out" => outdir = PathBuf::from(value()?),
            "--json" => json = true,
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
    match cmd.as_str() {
        "list" => {
            for s in slice::all() {
                out(&format!("{:<28} {}", s.id, s.title));
            }
            Ok(true)
        }
        "show" => {
            for s in pick(&ids)? {
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
            for s in pick(&ids)? {
                let r = run(&s, &mut ModelRun::default());
                ok &= r.passed();
                out(&line(&r));
            }
            Ok(ok)
        }
        "run" => {
            std::fs::create_dir_all(&outdir).map_err(|e| e.to_string())?;
            let bin = std::fs::canonicalize(&bin).map_err(|e| format!("{}: {e}", bin.display()))?;
            std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
            let work = std::fs::canonicalize(&work).map_err(|e| e.to_string())?;
            let outdir = std::fs::canonicalize(&outdir).map_err(|e| e.to_string())?;
            let mut ok = true;
            let mut summary = format!(
                "# Slice scenarios ({}, {})\n\n| Scenario | Interpreter | Verdict | Checks |\n|---|---|---|---|\n",
                isolation.name(),
                std::env::consts::OS
            );
            for s in pick(&ids)? {
                let row = if let Some(why) = unrunnable(&s, isolation) {
                    format!("| {} | exec {} | skip | {why} |", s.id, isolation.name())
                } else {
                    let cfg = Config {
                        bin: bin.clone(),
                        work: work.clone(),
                        out: outdir.clone(),
                        isolation,
                    };
                    let r = run(&s, &mut Exec::new(cfg, &s.id));
                    ok &= r.passed();
                    let text = serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?;
                    std::fs::write(outdir.join(format!("{}.json", s.id)), text)
                        .map_err(|e| e.to_string())?;
                    line(&r)
                };
                out(&row);
                summary.push_str(&row);
                summary.push('\n');
            }
            std::fs::write(outdir.join("summary.md"), summary).map_err(|e| e.to_string())?;
            Ok(ok)
        }
        other => Err(format!("no command {other}")),
    }
}
