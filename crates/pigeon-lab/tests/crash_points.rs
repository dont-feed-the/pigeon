//! The executor's crash points are pigeond's (13 §13.12): every
//! daemon crash point belongs to the randomized loop workload or the
//! standalone recovery workload. A renamed point fails here, rather than
//! leaving a scenario waiting for a crash that never comes.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]

use std::collections::BTreeSet;
use std::path::Path;

use pigeon_lab::step::CrashPoint;

fn literals(dir: &Path, found: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            literals(&path, found);
        } else if path.extension().is_some_and(|x| x == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            for part in text.split("crash_point(\"").skip(1) {
                if let Some(end) = part.find('"') {
                    found.insert(part[..end].to_owned());
                }
            }
        }
    }
}

#[test]
fn the_crash_points_are_pigeonds() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../pigeond/src");
    let mut theirs = BTreeSet::new();
    literals(&src, &mut theirs);
    let mut ours: BTreeSet<String> = CrashPoint::ALL
        .iter()
        .map(|c| c.name().to_owned())
        .collect();
    // Refounding needs its own invitation/history workload. These are
    // exercised with real SIGKILLs by pigeond/tests/refound_restart.rs;
    // adding them to CrashPoint::ALL would arm unreachable points in S9.
    ours.extend(
        [
            "join-in-place.before-main",
            "join-in-place.after-main",
            "join-in-place.after-index",
            "join-in-place.after-config",
        ]
        .map(str::to_owned),
    );
    assert_eq!(ours, theirs);
}
