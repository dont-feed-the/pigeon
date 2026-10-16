//! The process executor's plumbing, against stand-ins for `pigeond` and
//! `pigeon` written in sh: devices, env, files, outcomes, supervision,
//! races, measures and the merged trace, in `dirs` isolation.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use pigeon_core::headline::Level;
use pigeon_lab::exec::host::Isolation;
use pigeon_lab::exec::{Config, Exec};
use pigeon_lab::run::{Verdict, run};
use pigeon_lab::scenario::{Handle, Scenario, Steps, expect, race, wait};
use pigeon_lab::slice::{Slice, slice};
use pigeon_lab::step::{CrashPoint, Kind};

const PIGEOND: &str = r#"#!/bin/sh
dir="$XDG_STATE_HOME/pigeon/traces"
mkdir -p "$dir"
printf '{"spec":"CaptureInstall","device":"id-%s","clock":1,"action":"Capture","vars":{}}\n' "$PIGEON_NAME" > "$dir/a.ndjson"
printf '{"spec":"Evict","device":"id-%s","clock":2,"action":"Start","vars":{}}\n' "$PIGEON_NAME" >> "$dir/a.ndjson"
printf '{"spec":"Evict","device":"id-%s","clock":3,"action":"Start","vars":{}}\n' "$PIGEON_NAME" >> "$dir/a.ndjson"
printf '{"spec":"CaptureInstall","device":"id-%s","clock":4,"action":"Capture","vars":{}}\n' "$PIGEON_NAME" >> "$dir/a.ndjson"
exec sleep 600
"#;

const PIGEON: &str = r#"#!/bin/sh
case "$1" in
status) echo '{"type":"Status","headline":{"id":"headline-safe"},"state":"protected","versions":[{"path":"docs/a.txt","number":1,"state":"protected"}],"not_shared":[]}' ;;
invite) echo '{"type":"Said","message":{"id":"invitation","args":{"command":"curl x | sh -s -- --join pgn2-test"}}}' ;;
machines) echo '{"type":"Machines","machines":[{"name":"laptop","device":"id-laptop","custodian":false}]}' ;;
restore) echo '{"code":"disk-full","message":{"id":"problem-disk-full"}}' >&2; exit 1 ;;
*) echo '{"type":"Done"}' ;;
esac
"#;

fn script(dir: &Path, name: &str, body: &str) {
    let p = dir.join(name);
    fs::write(&p, body).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn the_executor_drives_devices_through_their_commands() {
    let tmp = std::env::temp_dir().join(format!("pigeon-lab-smoke-{}", std::process::id()));
    let bin = tmp.join("bin");
    fs::create_dir_all(&bin).unwrap();
    script(&bin, "pigeond", PIGEOND);
    script(&bin, "pigeon", PIGEON);
    let Slice {
        world, l, c1, docs, ..
    } = slice();
    let a = docs.file("a.txt");
    let steps = vec![
        l.write_text(&a, "x"),
        expect(l.has_text(&a, "x")),
        wait(l.sees(&a, Kind::Protected), 5),
        wait(l.headline(Level::Safe), 5),
        l.free(docs),
        l.restore(&a, 1).refused("disk-full"),
        c1.stop(),
        c1.start(),
        l.crash(CrashPoint::CaptureBeforeRef, 1),
        l.measure("smoke"),
        expect(l.no_pointers(docs)),
        race(vec![l.keep(docs), l.write_text(&a, "y")]),
        expect(l.has_text(&a, "y")),
    ];
    let s = Scenario {
        id: "smoke".into(),
        title: "plumbing".into(),
        world,
        steps: Steps(steps),
        needs: vec![],
    };
    let cfg = Config {
        bin,
        work: tmp.join("work"),
        out: tmp.join("out"),
        isolation: Isolation::Dirs,
    };
    let mut exec = Exec::new(cfg, "smoke");
    let report = run(&s, &mut exec);
    assert!(report.passed(), "{:#?}", report.checks);
    let trace = report.checks.last().unwrap();
    assert_eq!(trace.verdict, Verdict::Pass, "{trace:?}");
    assert!(
        report
            .measures
            .iter()
            .any(|m| m.label == "smoke" && m.parts.get("folders").copied().unwrap_or(0) > 0)
    );
    assert!(tmp.join("out/smoke/trace.ndjson").exists());
    let _ = fs::remove_dir_all(&tmp);
}
