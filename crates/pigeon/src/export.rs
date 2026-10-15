//! `pigeon export --to DIR [--at DATE|COMMIT] [--deleted] [REPOSITORY…]`
//! (09 §9.9, ADR 0047): writes the group's files as plain files, without
//! `pigeond`, network or key. It reads each repository read-only, takes the
//! tree of the newest `main` (or of `--at`), finds each git-annex object in
//! any of them, checks its SHA-256 before the file takes its name, and
//! writes `MANIFEST.tsv` and, when something is missing, `MISSING.tsv`.
//! `--deleted` adds the last version of each deleted file under `.deleted/`.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use pigeon_core::export::{self as core, Row};
use pigeon_proto::{Arg, Message, Problem};
use sha2::{Digest, Sha256};

use crate::repo::{Blobs, Entry, Repo};

/// What `pigeon export` was asked.
#[derive(Debug, PartialEq, Eq)]
pub struct Export {
    /// The directory to write, missing or empty.
    pub to: PathBuf,
    /// A commit or a date.
    pub at: Option<String>,
    /// Also the files deleted before then.
    pub deleted: bool,
    /// The repositories to read, the current folder when empty.
    pub repos: Vec<PathBuf>,
}

fn said(id: &str, args: &[(&str, Arg)]) -> Message {
    Message {
        id: id.to_owned(),
        args: args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    }
}

fn text(s: &str) -> Arg {
    Arg::Text(s.to_owned())
}

fn count(n: usize) -> Arg {
    Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

/// The objects of every repository, and for working trees the files there,
/// which may hold an object too (an unlocked or thin file).
struct Sources {
    objects: Vec<BTreeMap<String, PathBuf>>,
    works: Vec<PathBuf>,
}

impl Sources {
    /// Where `key` may be, and whether each is an object, which a wrong
    /// hash shows damaged, rather than a file of a working tree.
    fn candidates(&self, key: &str, path: &str) -> Vec<(PathBuf, bool)> {
        let objects = self
            .objects
            .iter()
            .filter_map(|o| Some((o.get(key)?.clone(), true)));
        let works = self.works.iter().map(|w| (w.join(path), false));
        objects.chain(works).collect()
    }
}

/// Copies `from` to `to` while hashing it; its size and SHA-256 hex.
fn copy_hashing(from: &Path, to: &Path) -> std::io::Result<(u64, String)> {
    let mut input = std::fs::File::open(from)?;
    let mut output = std::fs::File::create(to)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut size = 0u64;
    loop {
        let n = input.read(&mut buf)?;
        let Some(chunk) = buf.get(..n).filter(|c| !c.is_empty()) else {
            break;
        };
        hash.update(chunk);
        output.write_all(chunk)?;
        size += n as u64;
    }
    output.sync_all()?;
    Ok((size, hex(&hash.finalize())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn set_mode(path: &Path, executable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o755 } else { 0o644 };
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

/// The outcome of writing one file.
enum Wrote {
    File(u64, String),
    Missing(String),
    Mismatch(String),
}

/// Writes the content of `entry` at `dest`.
fn write_one(entry: &Entry, dest: &Path, blobs: &mut Blobs, sources: &Sources) -> Wrote {
    let Some(blob) = blobs.read(&entry.oid) else {
        return Wrote::Missing(entry.oid.clone());
    };
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let link = entry.mode == "120000";
    let key = if link {
        core::link_key(&String::from_utf8_lossy(&blob))
    } else {
        core::pointer_key(&blob)
    };
    let Some(key) = key else {
        let made = if link {
            std::os::unix::fs::symlink(String::from_utf8_lossy(&blob).as_ref(), dest)
        } else {
            std::fs::write(dest, &blob)
        };
        if made.is_err() {
            return Wrote::Missing(entry.oid.clone());
        }
        return Wrote::File(blob.len() as u64, hex(&Sha256::digest(&blob)));
    };
    let part = dest.with_file_name(format!(
        ".{}.pigeon-part",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));
    let mut seen = false;
    for (from, object) in sources.candidates(&key, &entry.path) {
        let Ok((size, digest)) = copy_hashing(&from, &part) else {
            continue;
        };
        seen |= object;
        if core::matches(&key, size, &digest) && std::fs::rename(&part, dest).is_ok() {
            set_mode(dest, entry.mode == "100755");
            return Wrote::File(size, digest);
        }
    }
    let _ = std::fs::remove_file(&part);
    if seen {
        Wrote::Mismatch(key)
    } else {
        Wrote::Missing(key)
    }
}

fn problem(code: &str, args: &[(&str, &str)]) -> Box<Problem> {
    Box::new(crate::local_problem(code, args))
}

/// Runs the export; the messages to print, or the problem that stops it.
pub fn run(e: &Export) -> Result<Vec<Message>, Box<Problem>> {
    let given = if e.repos.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        e.repos.clone()
    };
    let mut repos = Vec::new();
    for p in &given {
        let shown = p.display().to_string();
        let r =
            Repo::open(p).ok_or_else(|| problem("export-not-a-repository", &[("path", &shown)]))?;
        repos.push(r);
    }
    let at = e.at.as_deref();
    let (history, commit) = repos
        .iter()
        .filter_map(|r| Some((r, r.resolve(at)?)))
        .max_by_key(|(r, c)| r.time(c))
        .ok_or_else(|| problem("export-no-version", &[("at", at.unwrap_or("main"))]))?;
    let to = &e.to;
    if std::fs::read_dir(to).is_ok_and(|mut d| d.next().is_some()) {
        let shown = to.display().to_string();
        return Err(problem("export-not-empty", &[("dir", &shown)]));
    }
    std::fs::create_dir_all(to).map_err(|_| {
        let shown = to.display().to_string();
        problem("export-not-empty", &[("dir", &shown)])
    })?;
    let sources = Sources {
        objects: repos.iter().map(Repo::objects).collect(),
        works: repos.iter().filter_map(|r| r.work.clone()).collect(),
    };
    let current: Vec<Entry> = history
        .tree(&commit, &[])
        .into_iter()
        .filter(|t| core::exported(&t.path) && core::safe(&t.path))
        .collect();
    let paths: BTreeSet<String> = current.iter().map(|t| t.path.clone()).collect();
    let changes = history.last_changes(&commit, false);
    let mut jobs: Vec<(Entry, String, [String; 3])> = Vec::new();
    let unknown = || [commit.clone(), String::new(), String::new()];
    for t in current {
        let c = changes.get(&t.path).cloned().unwrap_or_else(unknown);
        let dest = t.path.clone();
        jobs.push((t, dest, c));
    }
    let deleted_dir = core::free_name(".deleted", &paths);
    let mut deleted = 0;
    if e.deleted {
        for (path, by) in history.last_changes(&commit, true) {
            if paths.contains(&path) || !core::exported(&path) || !core::safe(&path) {
                continue;
            }
            let before = format!("{}^", by[0]);
            for t in history.tree(&before, &[&path]) {
                let c = changes.get(&t.path).cloned().unwrap_or_else(unknown);
                let dest = format!("{deleted_dir}/{}", t.path);
                deleted += 1;
                jobs.push((t, dest, c));
            }
        }
    }
    let mut blobs = history
        .blobs()
        .ok_or_else(|| problem("internal-error", &[]))?;
    let mut rows = Vec::new();
    let mut missing: Vec<(String, String, &str)> = Vec::new();
    for (t, dest, [commit, author, date]) in &jobs {
        match write_one(t, &to.join(dest), &mut blobs, &sources) {
            Wrote::File(size, sha256) => rows.push(Row {
                path: dest.clone(),
                size,
                sha256,
                commit: commit.clone(),
                author: author.clone(),
                date: date.clone(),
            }),
            Wrote::Missing(key) => missing.push((dest.clone(), key, "missing")),
            Wrote::Mismatch(key) => missing.push((dest.clone(), key, "damaged")),
        }
    }
    let mut taken: BTreeSet<String> = jobs.iter().map(|j| j.1.clone()).collect();
    let manifest = core::free_name("MANIFEST.tsv", &taken);
    taken.insert(manifest.clone());
    let write = |name: &str, body: &str| {
        std::fs::write(to.join(name), body).map_err(|_| problem("internal-error", &[]))
    };
    write(&manifest, &core::manifest(&rows))?;
    let dir = to.display().to_string();
    let mut out = vec![said(
        "export-done",
        &[
            ("files", count(rows.len())),
            ("dir", text(&dir)),
            ("commit", text(commit.get(..10).unwrap_or(&commit))),
        ],
    )];
    if e.deleted {
        out.push(said(
            "export-deleted",
            &[("files", count(deleted)), ("folder", text(&deleted_dir))],
        ));
    }
    if !missing.is_empty() {
        let name = core::free_name("MISSING.tsv", &taken);
        let mut body = String::from("path\tkey\twhy\n");
        for (p, k, why) in &missing {
            body.push_str(&format!("{}\t{k}\t{why}\n", p.replace(['\t', '\n'], " ")));
        }
        write(&name, &body)?;
        let damaged = missing.iter().filter(|m| m.2 == "damaged").count();
        out.push(said(
            "export-missing",
            &[
                ("files", count(missing.len())),
                ("damaged", count(damaged)),
                ("list", text(&name)),
            ],
        ));
    }
    Ok(out)
}
