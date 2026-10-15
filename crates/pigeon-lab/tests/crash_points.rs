//! Inventory all daemon crash checkpoints: slice, install-driver and bridge
//! recovery regressions exercise their respective seams.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

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
    // Incoming-install seams are exercised by check-driver-traces rather
    // than the free/keep commands of the S9 EDSL scenarios.
    let driver = std::fs::read_to_string(src.join("../tests/install/tracing.rs")).unwrap();
    for point in [
        "install.after-begin",
        "install.after-aside",
        "install.after-proof",
        "install.after-rename",
        "install.before-index-observation",
        "install.after-done",
        "install.before-done-observation",
    ] {
        assert!(driver.contains(&format!("\"{point}\"")));
        ours.insert(point.to_owned());
    }
    ours.extend(
        [
            "bridge-before-capture",
            "bridge-hide-after-flag",
            "bridge-hide-after-remove",
        ]
        .map(str::to_owned),
    );
    assert_eq!(ours, theirs);
}
