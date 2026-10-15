//! `pigeon free --without-checking PATH` (08 §8.7): on a laptop alone, the
//! files of `old/` are on no other computer. Without `--yes` the CLI shows
//! that risk and refuses with `needs-confirmation`; with it, the files are
//! hidden, their content is dropped although it was the last copy, and a
//! drop record of this device is committed on `main`.
//!
//! Needs what `tests/slice.rs` needs.
#![cfg(feature = "lab")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use common::{SERIAL, dirs, pigeon, said, start, until};
use pigeon_proto::Response;
use pigeon_proto::api::{Arg, State};

fn progress(rs: &[Response], id: &str, arg: &str) -> Option<i64> {
    rs.iter().find_map(|r| match r {
        Response::Progress { message, .. } if message.id == id => match message.args.get(arg) {
            Some(Arg::Number(n)) => Some(*n),
            _ => None,
        },
        _ => None,
    })
}

fn objects_holding(dir: &std::path::Path, body: &str) -> usize {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if std::fs::read_to_string(&p).is_ok_and(|t| t == body) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn a_drop_without_checking_asks_then_drops_the_last_copy_and_records_it() {
    let _one = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((_tmp, root, sockets)) = dirs() else {
        return;
    };
    let a1 = start(&root, sockets.path(), "a1");
    let all = [&a1];
    let fa = root.join("ana");
    std::fs::create_dir_all(fa.join("old")).unwrap();
    std::fs::write(fa.join("keep.txt"), "kept here\n").unwrap();
    std::fs::write(fa.join("old/a.txt"), "only copy a\n").unwrap();
    std::fs::write(fa.join("old/b.txt"), "only copy b\n").unwrap();
    let f = fa.to_str().unwrap();
    let created = pigeon(&a1, &["create", "Family", f, "--k", "1", "--name", "ana-1"]);
    assert!(said(&created, "created", "group").is_some(), "{created:?}");
    until(&a1, State::Local, &all);
    let objects = fa.join(".git/annex/objects");
    assert_eq!(objects_holding(&objects, "only copy a\n"), 1);

    let old = fa.join("old");
    let old = old.to_str().unwrap();
    let unasked = pigeon(&a1, &["free", "--without-checking", old]);
    assert_eq!(
        progress(&unasked, "drop-risk", "alone"),
        Some(2),
        "{unasked:?}"
    );
    assert!(
        unasked
            .iter()
            .any(|r| matches!(r, Response::Problem(p) if p.code == "needs-confirmation")),
        "{unasked:?}"
    );
    assert!(fa.join("old/a.txt").exists(), "nothing is dropped unasked");

    let done = pigeon(&a1, &["free", "--without-checking", old, "--yes"]);
    assert_eq!(
        said(&done, "drop-done", "files").as_deref(),
        Some("2"),
        "{done:?}"
    );
    assert_eq!(
        said(&done, "drop-done", "dropped").as_deref(),
        Some("2"),
        "{done:?}"
    );
    assert!(!fa.join("old/a.txt").exists() && !fa.join("old/b.txt").exists());
    assert_eq!(
        objects_holding(&objects, "only copy a\n"),
        0,
        "the last copy is gone"
    );
    assert_eq!(
        std::fs::read_to_string(fa.join("keep.txt")).unwrap(),
        "kept here\n"
    );
    let records = std::process::Command::new("git")
        .args([
            "-C",
            f,
            "ls-tree",
            "-r",
            "--name-only",
            "main",
            ".pigeon/records",
        ])
        .output()
        .unwrap();
    let records = String::from_utf8_lossy(&records.stdout);
    assert!(records.starts_with(".pigeon/records/drop-"), "{records}");
}
