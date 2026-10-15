//! `pigeon export` against real git-annex repositories (09 §9.9, ADR 0047):
//! a laptop's folder and a partial bare backup, combined; a damaged object
//! refused; an older version and deleted files. Skipped without git-annex
//! unless `PIGEON_REQUIRE_GIT_ANNEX` is set.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn run(dir: &Path, args: &[&str], date: &str) {
    let ok = Command::new(args[0])
        .args(&args[1..])
        .current_dir(dir)
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .env("GIT_AUTHOR_NAME", "Ana")
        .env("GIT_AUTHOR_EMAIL", "ana@example.org")
        .env("GIT_COMMITTER_NAME", "Ana")
        .env("GIT_COMMITTER_EMAIL", "ana@example.org")
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
}

fn annex_present() -> bool {
    let here = Command::new("git").args(["annex", "version"]).output();
    let present = here.is_ok_and(|o| o.status.success());
    assert!(
        present || std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none(),
        "git-annex is required"
    );
    present
}

fn export(args: &[&str]) -> (bool, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_pigeon"))
        .args(["export", "--json"])
        .args(args)
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    (o.status.success(), text)
}

fn write(at: &Path, body: &str) {
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, body).unwrap();
}

const D1: &str = "2026-01-01T10:00:00Z";
const D2: &str = "2026-03-01T10:00:00Z";

/// A laptop folder `a` with two commits and a bare backup `b` that has only
/// `x.txt`'s objects.
fn setup(root: &Path) {
    let a = root.join("a");
    std::fs::create_dir_all(&a).unwrap();
    for c in [
        &["git", "init", "-q", "-b", "main"][..],
        &["git", "annex", "init", "-q", "laptop"],
        &["git", "config", "annex.backend", "SHA256E"],
        &["git", "config", "annex.addunlocked", "true"],
    ] {
        run(&a, c, D1);
    }
    write(&a.join("x.txt"), "first\n");
    write(&a.join("dir/old.txt"), "old\n");
    write(&a.join("dir/run.sh"), "#!/bin/sh\necho hi\n");
    std::fs::set_permissions(a.join("dir/run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    write(&a.join(".pigeon/roster.toml"), "roster\n");
    run(&a, &["git", "annex", "add", "-q", "x.txt", "dir"], D1);
    run(&a, &["git", "add", ".pigeon"], D1);
    run(&a, &["git", "commit", "-q", "-m", "one"], D1);
    write(&a.join("x.txt"), "second\n");
    std::fs::remove_file(a.join("dir/old.txt")).unwrap();
    run(&a, &["git", "annex", "add", "-q", "x.txt"], D2);
    run(&a, &["git", "add", "-A"], D2);
    run(&a, &["git", "commit", "-q", "-m", "two"], D2);
    run(root, &["git", "clone", "-q", "--bare", "a", "b"], D2);
    run(
        &root.join("b"),
        &["git", "annex", "init", "-q", "backup"],
        D2,
    );
    run(&a, &["git", "remote", "add", "b", "../b"], D2);
    run(
        &a,
        &["git", "annex", "copy", "-q", "--to", "b", "x.txt"],
        D2,
    );
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

#[test]
fn export_writes_checked_files_from_partial_repositories() {
    if !annex_present() {
        return;
    }
    let tmp = std::env::temp_dir().join(format!("pigeon-export-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    setup(&tmp);
    let (a, b) = (tmp.join("a"), tmp.join("b"));
    let s = |p: &Path| p.to_str().unwrap().to_owned();

    // The laptop alone: every current file, pigeon's own left out.
    let out = tmp.join("out1");
    let (ok, text) = export(&["--to", &s(&out), &s(&a)]);
    assert!(ok, "{text}");
    assert_eq!(read(&out.join("x.txt")), "second\n");
    assert_eq!(read(&out.join("dir/run.sh")), "#!/bin/sh\necho hi\n");
    let mode = std::fs::metadata(out.join("dir/run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111);
    assert!(!out.join(".pigeon").exists() && !out.join("dir/old.txt").exists());
    let manifest = pigeon_core::export::read_manifest(&read(&out.join("MANIFEST.tsv"))).unwrap();
    assert_eq!(manifest.len(), 2);
    assert!(
        manifest
            .iter()
            .all(|r| r.author == "Ana" && r.sha256.len() == 64)
    );
    assert!(!out.join("MISSING.tsv").exists());

    // The backup alone has x.txt only; run.sh is listed as missing.
    let out = tmp.join("out2");
    let (ok, text) = export(&["--to", &s(&out), &s(&b)]);
    assert!(ok && text.contains("export-missing"), "{text}");
    assert_eq!(read(&out.join("x.txt")), "second\n");
    assert!(read(&out.join("MISSING.tsv")).contains("dir/run.sh"));

    // A damaged object on the backup is refused, and the laptop's used.
    let objects = b.join("annex/objects");
    let mut stack = vec![objects];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
                stack.push(p);
            } else if read(&p) == "second\n" {
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
                std::fs::write(&p, "sec0nd\n").unwrap();
            }
        }
    }
    let out = tmp.join("out3");
    let (ok, text) = export(&["--to", &s(&out), &s(&b)]);
    assert!(ok && text.contains("export-missing"), "{text}");
    assert!(read(&out.join("MISSING.tsv")).contains("damaged"));
    assert!(!out.join("x.txt").exists());
    let out = tmp.join("out4");
    let (ok, text) = export(&["--to", &s(&out), &s(&b), &s(&a)]);
    assert!(ok && !text.contains("export-missing"), "{text}");
    assert_eq!(read(&out.join("x.txt")), "second\n");

    // An older version by date, and the deleted files.
    let out = tmp.join("out5");
    let (ok, text) = export(&["--to", &s(&out), "--at", "2026-02-01", "--deleted", &s(&a)]);
    assert!(ok, "{text}");
    assert_eq!(read(&out.join("x.txt")), "first\n");
    assert_eq!(read(&out.join("dir/old.txt")), "old\n");
    let out = tmp.join("out6");
    let (ok, text) = export(&["--to", &s(&out), "--deleted", &s(&a)]);
    assert!(ok, "{text}");
    assert_eq!(read(&out.join(".deleted/dir/old.txt")), "old\n");

    // Problems: a folder with files, a path with no repository, no version.
    let (ok, text) = export(&["--to", &s(&out), &s(&a)]);
    assert!(!ok && text.contains("export-not-empty"), "{text}");
    let (ok, text) = export(&["--to", &s(&tmp.join("out7")), &s(&out)]);
    assert!(!ok && text.contains("export-not-a-repository"), "{text}");
    let (ok, text) = export(&["--to", &s(&tmp.join("out8")), "--at", "2020-01-01", &s(&a)]);
    assert!(!ok && text.contains("export-no-version"), "{text}");
    let _ = std::fs::remove_dir_all(&tmp);
}
