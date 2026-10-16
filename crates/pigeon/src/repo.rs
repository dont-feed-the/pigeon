//! Read-only access to one local repository for `pigeon export` (09 §9.9,
//! ADR 0047): git's own commands, run synchronously and without `pigeond`,
//! and an index of the git-annex objects present, laptop or custodian, bare
//! or not.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// One repository given to `pigeon export`.
pub struct Repo {
    /// Its git directory.
    pub git_dir: PathBuf,
    /// Its working tree, unless it is bare.
    pub work: Option<PathBuf>,
}

/// One entry of `git ls-tree -r`.
pub struct Entry {
    /// `100644`, `100755` or `120000`.
    pub mode: String,
    /// The blob.
    pub oid: String,
    /// The path in the tree.
    pub path: String,
}

fn git(dir: &Path) -> Command {
    let mut c = Command::new("git");
    c.arg("-C")
        .arg(dir)
        .args(["-c", "core.quotePath=false"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    c
}

impl Repo {
    /// The repository at `path`, `None` if git finds none there.
    pub fn open(path: &Path) -> Option<Repo> {
        let out = |args: &[&str]| -> Option<String> {
            let o = git(path).args(args).output().ok()?;
            o.status
                .success()
                .then(|| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        };
        let git_dir = PathBuf::from(out(&["rev-parse", "--absolute-git-dir"])?);
        let bare = out(&["rev-parse", "--is-bare-repository"])? == "true";
        let work = if bare {
            None
        } else {
            out(&["rev-parse", "--show-toplevel"]).map(PathBuf::from)
        };
        Some(Repo { git_dir, work })
    }

    fn cmd(&self) -> Command {
        git(&self.git_dir)
    }

    /// `git args…`'s output, `None` if it fails.
    pub fn out(&self, args: &[&str]) -> Option<Vec<u8>> {
        let o = self.cmd().args(args).output().ok()?;
        o.status.success().then_some(o.stdout)
    }

    fn line(&self, args: &[&str]) -> Option<String> {
        let o = self.out(args)?;
        let s = String::from_utf8_lossy(&o).trim().to_owned();
        (!s.is_empty()).then_some(s)
    }

    /// The commit `at` names here: a commit, or a date for the last commit
    /// of `main` before it; `main` itself without `at`.
    pub fn resolve(&self, at: Option<&str>) -> Option<String> {
        let Some(at) = at else {
            return self.line(&["rev-parse", "--verify", "-q", "refs/heads/main^{commit}"]);
        };
        let commit = format!("{at}^{{commit}}");
        self.line(&["rev-parse", "--verify", "-q", &commit])
            .or_else(|| {
                let before = format!("--before={at}");
                self.line(&[
                    "rev-list",
                    "-1",
                    "--first-parent",
                    &before,
                    "refs/heads/main",
                ])
            })
    }

    /// The commit time of `commit`, seconds since the epoch.
    pub fn time(&self, commit: &str) -> i64 {
        self.line(&["log", "-1", "--format=%ct", commit])
            .and_then(|t| t.parse().ok())
            .unwrap_or(0)
    }

    /// Current history followed by at most 31 signed archive links.
    /// Offline export checks the signature binding each link, canonical
    /// archive ref and old genesis. It does not run the daemon's authority
    /// fold; export remains a local, content-hash-checked rescue operation.
    pub fn histories(&self, commit: &str) -> Result<Vec<String>, ()> {
        let mut current = commit.to_owned();
        let mut histories = Vec::new();
        let mut seen = BTreeSet::new();
        for _ in 0..32 {
            if !seen.insert(current.clone()) {
                return Err(());
            }
            histories.push(current.clone());
            let root = self
                .line(&["rev-list", "--max-parents=0", &current])
                .ok_or(())?;
            let raw = self.out(&["cat-file", "commit", &root]).ok_or(())?;
            let claimed = String::from_utf8_lossy(&raw)
                .lines()
                .any(|l| l.starts_with("Pigeon-Refound:"));
            let Some(link) = pigeon_core::refound::archive(&raw) else {
                return if claimed { Err(()) } else { Ok(histories) };
            };
            let reference = format!("refs/pigeon/archive/{}/main^{{commit}}", link.group);
            if self
                .line(&["rev-parse", "--verify", "-q", &reference])
                .as_deref()
                != Some(&link.main)
                || self
                    .line(&["rev-list", "--max-parents=0", &link.main])
                    .as_deref()
                    != Some(&link.group)
            {
                return Err(());
            }
            current = link.main;
        }
        Err(())
    }

    /// The blobs and links of `treeish`, recursively, with `paths` only if
    /// some are given.
    pub fn tree(&self, treeish: &str, paths: &[&str]) -> Vec<Entry> {
        let mut args = vec!["ls-tree", "-r", "-z", "--full-tree", treeish, "--"];
        args.extend_from_slice(paths);
        let out = self.out(&args).unwrap_or_default();
        out.split(|b| *b == 0)
            .filter_map(|rec| {
                let rec = std::str::from_utf8(rec).ok()?;
                let (meta, path) = rec.split_once('\t')?;
                let mut m = meta.split(' ');
                let (mode, kind, oid) = (m.next()?, m.next()?, m.next()?);
                (kind == "blob").then(|| Entry {
                    mode: mode.to_owned(),
                    oid: oid.to_owned(),
                    path: path.to_owned(),
                })
            })
            .collect()
    }

    /// For each path changed in the history of `commit`, its last commit,
    /// author and date; with `deleted`, the commits that deleted paths.
    pub fn last_changes(&self, commit: &str, deleted: bool) -> BTreeMap<String, [String; 3]> {
        let mut args = vec![
            "log",
            "--no-renames",
            "--name-only",
            "--format=%x01%H%x09%an%x09%aI",
        ];
        if deleted {
            args.push("--diff-filter=D");
        }
        args.push(commit);
        let out = self.out(&args).unwrap_or_default();
        let mut last = BTreeMap::new();
        let mut current: Option<[String; 3]> = None;
        for line in String::from_utf8_lossy(&out).lines() {
            if let Some(head) = line.strip_prefix('\u{1}') {
                let mut f = head.splitn(3, '\t').map(str::to_owned);
                current = Some([
                    f.next().unwrap_or_default(),
                    f.next().unwrap_or_default(),
                    f.next().unwrap_or_default(),
                ]);
            } else if !line.is_empty()
                && let Some(c) = &current
            {
                last.entry(line.to_owned()).or_insert_with(|| c.clone());
            }
        }
        last
    }

    /// The git-annex objects this repository holds, by key.
    pub fn objects(&self) -> BTreeMap<String, PathBuf> {
        let mut found = BTreeMap::new();
        let mut stack = vec![self.git_dir.join("annex").join("objects")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if !e.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }
                let inner = p.join(e.file_name());
                if inner.is_file() {
                    found.insert(e.file_name().to_string_lossy().into_owned(), inner);
                } else {
                    stack.push(p);
                }
            }
        }
        found
    }

    /// A reader of blobs, `git cat-file --batch`.
    pub fn blobs(&self) -> Option<Blobs> {
        let mut child = self
            .cmd()
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .ok()?;
        let input = child.stdin.take()?;
        let output = BufReader::new(child.stdout.take()?);
        Some(Blobs {
            child,
            input,
            output,
        })
    }
}

/// A running `git cat-file --batch`.
pub struct Blobs {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Blobs {
    /// The bytes of blob `oid`, `None` if this repository lacks it.
    pub fn read(&mut self, oid: &str) -> Option<Vec<u8>> {
        writeln!(self.input, "{oid}").ok()?;
        self.input.flush().ok()?;
        let mut header = String::new();
        self.output.read_line(&mut header).ok()?;
        let mut f = header.split_whitespace();
        let (_, kind, size) = (f.next()?, f.next()?, f.next());
        let size: usize = size?.parse().ok()?;
        let mut buf = vec![0; size + 1];
        self.output.read_exact(&mut buf).ok()?;
        buf.pop();
        (kind == "blob").then_some(buf)
    }
}

impl Drop for Blobs {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
