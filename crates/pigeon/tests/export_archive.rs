//! Offline export of archived deletions after nested refounding (09 §9.9).
//! Real Git roots and annex object layout, without a daemon or network.
//! Broken, substituted or unsigned archive links must not look complete.
#![allow(clippy::unwrap_used, reason = "tests")]

use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Fixture {
    directory: tempfile::TempDir,
    repo: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("bare");
        std::fs::create_dir(&repo).unwrap();
        let f = Self { directory, repo };
        f.git(&["init", "--bare", "-q", "-b", "main"], None);
        f.git(&["config", "user.name", "Fixture"], None);
        f.git(&["config", "user.email", "fixture@example.invalid"], None);
        f
    }
    fn git(&self, args: &[&str], input: Option<&[u8]>) -> Vec<u8> {
        let mut child = Command::new("git")
            .arg("-C")
            .arg(&self.repo)
            .args(args)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(input) = input {
            child.stdin.take().unwrap().write_all(input).unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
    fn line(&self, args: &[&str], input: Option<&[u8]>) -> String {
        String::from_utf8(self.git(args, input))
            .unwrap()
            .trim()
            .into()
    }
    fn remove(&self, path: &str) {
        self.git(
            &["update-index", "--index-info"],
            Some(format!("0 {}\t{path}\n", "0".repeat(40)).as_bytes()),
        );
    }
    fn file(&self, path: &str, content: &[u8]) {
        let digest: String = Sha256::digest(content)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let key = format!("SHA256E-s{}--{digest}.txt", content.len());
        let object = self.repo.join("annex/objects/aa/bb").join(&key);
        std::fs::create_dir_all(&object).unwrap();
        std::fs::write(object.join(&key), content).unwrap();
        let pointer = format!("/annex/objects/{key}\n");
        let blob = self.line(&["hash-object", "-w", "--stdin"], Some(pointer.as_bytes()));
        self.git(
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "100644",
                &blob,
                path,
            ],
            None,
        );
    }
    fn commit(&self, parent: Option<&str>, message: &str, sign: bool) -> String {
        let tree = self.line(&["write-tree"], None);
        let mut args = vec!["commit-tree", &tree, "-m", message];
        if let Some(parent) = parent {
            args.extend(["-p", parent]);
        }
        let unsigned = self.line(&args, None);
        let commit = if sign {
            let raw = self.git(&["cat-file", "commit", &unsigned], None);
            let signed = pigeon_core::sig::sign(&raw, &[91; 32]).unwrap();
            self.line(
                &["hash-object", "-t", "commit", "-w", "--stdin"],
                Some(&signed),
            )
        } else {
            unsigned
        };
        self.git(&["update-ref", "refs/heads/main", &commit], None);
        commit
    }
    fn refound(&self, old: &str, main: &str, sign: bool) -> String {
        self.git(
            &[
                "update-ref",
                &format!("refs/pigeon/archive/{old}/main"),
                main,
            ],
            None,
        );
        self.commit(
            None,
            &format!("refound\n\nPigeon-Refound: {old} {main}"),
            sign,
        )
    }
    fn export(&self, name: &str, deleted: bool) -> (PathBuf, bool, String) {
        let to = self.directory.path().join(name);
        let mut command = Command::new(env!("CARGO_BIN_EXE_pigeon"));
        command.args(["--json", "export", "--to"]).arg(&to);
        if deleted {
            command.arg("--deleted");
        }
        let out = command
            .arg(&self.repo)
            .env("PIGEON_SOCKET", "/no-pigeon-for-offline-export")
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (to, out.status.success(), text)
    }
}

fn baseline(f: &Fixture) -> (String, String) {
    f.file("deleted.txt", b"old binary\r\n\0content\xff");
    f.file("kept.txt", b"still current\n");
    let root = f.commit(None, "create", true);
    f.git(
        &["update-index", "--index-info"],
        Some(format!("0 {}\tdeleted.txt\n", "0".repeat(40)).as_bytes()),
    );
    let deleted = f.commit(Some(&root), "delete", true);
    (root, deleted)
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

#[test]
fn nested_archives_export_deleted_bytes_once_without_a_daemon() {
    let f = Fixture::new();
    let (old, deleted) = baseline(&f);
    let first = f.refound(&old, &deleted, true);
    f.file("new.txt", b"new group\n");
    let next = f.commit(Some(&first), "save", true);
    f.refound(&first, &next, true);
    let (to, ok, text) = f.export("out", true);
    assert!(ok, "{text}");
    assert_eq!(
        read(&to.join(".deleted/deleted.txt")),
        b"old binary\r\n\0content\xff"
    );
    assert_eq!(read(&to.join("kept.txt")), b"still current\n");
    assert_eq!(read(&to.join("new.txt")), b"new group\n");
    let rows = pigeon_core::export::read_manifest(
        &String::from_utf8(read(&to.join("MANIFEST.tsv"))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        rows.iter()
            .filter(|r| r.path.ends_with("deleted.txt"))
            .count(),
        1
    );
}

#[test]
fn missing_substituted_or_unsigned_links_cannot_claim_a_complete_deleted_export() {
    for bad in ["missing", "substituted", "unsigned"] {
        let f = Fixture::new();
        let (old, deleted) = baseline(&f);
        f.refound(&old, &deleted, bad != "unsigned");
        let reference = format!("refs/pigeon/archive/{old}/main");
        match bad {
            "missing" => {
                f.git(&["update-ref", "-d", &reference], None);
            }
            "substituted" => {
                f.git(&["update-ref", &reference, &old], None);
            }
            _ => {}
        }
        let (to, ok, text) = f.export("refused", true);
        assert!(
            !ok && text.contains("export-history-unverified"),
            "{bad}: {text}"
        );
        assert!(!to.join("kept.txt").exists());
        let (to, ok, text) = f.export("current-only", false);
        assert!(ok, "{text}");
        assert_eq!(read(&to.join("kept.txt")), b"still current\n");
    }
}

#[test]
fn deleted_symlink_collisions_never_write_through_to_outside_files() {
    let f = Fixture::new();
    let outside = f.directory.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("sub.txt"), b"outside sentinel\n").unwrap();
    f.file("link/sub.txt", b"archived inner bytes\n");
    let root = f.commit(None, "create nested file", true);
    f.remove("link/sub.txt");
    let blob = f.line(
        &["hash-object", "-w", "--stdin"],
        Some(outside.to_str().unwrap().as_bytes()),
    );
    f.git(
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "120000",
            &blob,
            "link",
        ],
        None,
    );
    let changed = f.commit(Some(&root), "replace folder with link", true);
    f.remove("link");
    let deleted = f.commit(Some(&changed), "delete link", true);
    f.refound(&root, &deleted, true);
    let (to, ok, text) = f.export("out", true);
    assert!(ok && text.contains("export-missing"), "{text}");
    assert_eq!(read(&outside.join("sub.txt")), b"outside sentinel\n");
    let missing = String::from_utf8(read(&to.join("MISSING.tsv"))).unwrap();
    assert!(
        missing.contains(".deleted/link/sub.txt") && missing.contains("destination-conflict"),
        "{missing}"
    );
}

#[test]
fn manifest_names_avoid_case_folded_user_names_on_every_platform() {
    let f = Fixture::new();
    f.file("manifest.tsv", b"user manifest\n");
    f.file(".DELETED/personal.txt", b"user folder\n");
    f.commit(None, "create", true);
    let (to, ok, text) = f.export("out", true);
    assert!(ok && !text.contains("export-missing"), "{text}");
    assert_eq!(read(&to.join("manifest.tsv")), b"user manifest\n");
    assert_eq!(read(&to.join(".DELETED/personal.txt")), b"user folder\n");
    assert!(to.join("MANIFEST.tsv-2").exists());
}

#[test]
fn a_user_file_with_the_old_temporary_name_is_kept() {
    let f = Fixture::new();
    f.file("kept.txt", b"current content\n");
    f.file(".kept.txt.pigeon-part", b"a real user file\n");
    f.commit(None, "create", true);
    let (to, ok, text) = f.export("out", false);
    assert!(ok && !text.contains("export-missing"), "{text}");
    assert_eq!(read(&to.join("kept.txt")), b"current content\n");
    assert_eq!(
        read(&to.join(".kept.txt.pigeon-part")),
        b"a real user file\n"
    );
}
