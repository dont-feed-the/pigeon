//! A lab build's crash point kills the process at the n-th crossing, as
//! `kill -9` would, and not before (13 §13.12). The test runs itself as a
//! child with `PIGEON_LAB_CRASH` set and reads how the child ended.
#![cfg(feature = "lab")]

use std::os::unix::process::ExitStatusExt;
use std::process::Command;

const CHILD: &str = "PIGEON_LAB_CRASH_TEST_CHILD";

#[test]
fn crosses_then_dies_at_the_planned_crossing() {
    if let Ok(times) = std::env::var(CHILD) {
        let times: u64 = times.parse().unwrap();
        for i in 1..=times {
            println!("crossing {i}");
            pigeon_sys::lab::crash_point("test.point");
            pigeon_sys::lab::crash_point("test.other");
        }
        return;
    }
    let me = std::env::current_exe().unwrap();
    let run = |times: u64| {
        Command::new(&me)
            .args([
                "--exact",
                "crosses_then_dies_at_the_planned_crossing",
                "--nocapture",
            ])
            .env(CHILD, times.to_string())
            .env("PIGEON_LAB_CRASH", "test.point:3")
            .output()
            .unwrap()
    };
    let out = run(5);
    assert_eq!(out.status.signal(), Some(9), "killed by SIGKILL");
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(printed.contains("crossing 3") && !printed.contains("crossing 4"));
    let out = run(2);
    assert!(
        out.status.success(),
        "two crossings of a point planned for the third live"
    );
}
