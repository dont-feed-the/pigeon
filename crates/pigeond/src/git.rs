//! The git calls of the loops, over pigeon-git's checked driver (03 §3.7):
//! refs, trees, blobs and the keys a tree names.
//!
//! `pigeond` commits with `commit-tree` and moves a ref with `update-ref`
//! and its expected old value, so that no ref moves under a concurrent
//! writer. Each commit is signed with the device key ([`set_signer`]) as
//! `git commit -S` with `gpg.format=ssh` would sign it (ADR 0007), then
//! written back with `hash-object -t commit`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use std::time::Duration;

use pigeon_core::authority::{Entry, Tree};

use pigeon_git::{GitEnv, Repo};

/// A git error.
pub type Error = pigeon_git::Error;

/// The zero object ID: "must not exist" as an old value.
pub const ZERO: &str = "0000000000000000000000000000000000000000";

/// One file of a tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeFile {
    /// Its mode, such as `100644`.
    pub mode: String,
    /// Its blob.
    pub oid: String,
    /// Its path.
    pub path: String,
}

/// The device key every commit is signed with, once the daemon set it.
static SIGNER: OnceLock<[u8; 32]> = OnceLock::new();

/// Sets the device key commits are signed with (ADR 0007); a second call
/// changes nothing.
pub fn set_signer(secret: [u8; 32]) {
    let _ = SIGNER.set(secret);
}

/// The device whose key this daemon uses for local contributions.
pub(crate) fn signer_device() -> Option<String> {
    SIGNER.get().map(|key| pigeon_core::sig::device_id(key).0)
}

/// A repository, as the loops call git in it.
#[derive(Clone, Debug)]
pub struct Git {
    /// The driver.
    pub repo: Repo,
}

/// Whether `path` is pigeon's own, never captured: `.pigeon/` and
/// `.gitattributes` at the top (03 §3.3).
pub fn is_pigeon_path(path: &str) -> bool {
    path == ".gitattributes" || path == ".pigeon" || path.starts_with(".pigeon/")
}

/// The key of an annex pointer blob.
pub fn pointer_key(blob: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(blob).ok()?;
    let line = text.lines().next()?;
    let key = line.strip_prefix("/annex/objects/")?;
    (!key.is_empty() && !key.contains('/')).then(|| key.to_owned())
}

impl Git {
    /// The repository at `dir`.
    pub fn new(dir: impl Into<std::path::PathBuf>, env: GitEnv) -> Git {
        Git {
            repo: Repo::new(dir, env),
        }
    }

    /// Runs `git args…`, its output as text.
    pub async fn run(&self, args: &[&str]) -> Result<String, Error> {
        self.run_for(args, pigeon_git::DEFAULT_TIMEOUT).await
    }

    /// Runs `git args…` within `timeout`.
    pub async fn run_for(&self, args: &[&str], timeout: Duration) -> Result<String, Error> {
        Ok(self.repo.run(args, None, timeout).await?.text())
    }

    /// Runs `git args…` with `input` on its stdin.
    pub async fn run_in(&self, args: &[&str], input: &[u8]) -> Result<String, Error> {
        let out = self
            .repo
            .run(args, Some(input), pigeon_git::DEFAULT_TIMEOUT)
            .await?;
        Ok(out.text())
    }

    /// Whether `git args…` exits 0.
    pub async fn ok(&self, args: &[&str]) -> bool {
        matches!(
            self.repo
                .run_status(args, None, pigeon_git::DEFAULT_TIMEOUT)
                .await,
            Ok((Some(0), _))
        )
    }

    /// The object a name resolves to, if any.
    pub async fn rev(&self, name: &str) -> Option<String> {
        let out = self
            .run(&["rev-parse", "--verify", "-q", name])
            .await
            .ok()?;
        let oid = out.trim();
        (!oid.is_empty()).then(|| oid.to_owned())
    }

    /// Whether `ancestor` is `descendant` or one of its ancestors.
    pub async fn contains(&self, descendant: &str, ancestor: &str) -> bool {
        let range = format!("^{descendant}");
        match self
            .run(&["rev-list", "--count", ancestor, range.as_str()])
            .await
        {
            Ok(n) => n.trim() == "0",
            Err(_) => false,
        }
    }

    /// The root commit of `commit`'s history, if it has one only.
    pub async fn root(&self, commit: &str) -> Option<String> {
        let out = self
            .run(&["rev-list", "--max-parents=0", commit])
            .await
            .ok()?;
        let roots: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
        match roots.as_slice() {
            [one] => Some((*one).to_owned()),
            _ => None,
        }
    }

    /// Moves `name` to `new` if it is at `old` (`None`: absent).
    pub async fn update_ref(&self, name: &str, new: &str, old: Option<&str>) -> Result<(), Error> {
        self.run(&["update-ref", name, new, old.unwrap_or(ZERO)])
            .await
            .map(|_| ())
    }

    /// A commit of `tree` with `parents`.
    pub async fn commit_tree(
        &self,
        tree: &str,
        parents: &[&str],
        message: &str,
    ) -> Result<String, Error> {
        let mut args = vec!["commit-tree", tree];
        for p in parents {
            args.push("-p");
            args.push(p);
        }
        args.push("-m");
        args.push(message);
        let commit = self.run(&args).await?.trim().to_owned();
        match SIGNER.get() {
            Some(secret) => self.sign(&commit, secret).await,
            None => Ok(commit),
        }
    }

    /// The commit `commit` signed with `secret`, written to the object
    /// store.
    pub async fn sign(&self, commit: &str, secret: &[u8; 32]) -> Result<String, Error> {
        let raw = self.object(commit).await?;
        let signed = pigeon_core::sig::sign(&raw, secret).map_err(|e| Error::Parse {
            argv: vec!["cat-file".into(), commit.to_owned()],
            detail: format!("signing: {e:?}"),
        })?;
        let args = ["hash-object", "-t", "commit", "-w", "--stdin"];
        Ok(self.run_in(&args, &signed).await?.trim().to_owned())
    }

    /// The bytes of the object `name`.
    pub async fn object(&self, name: &str) -> Result<Vec<u8>, Error> {
        let found = self.objects(&[name.to_owned()]).await?;
        found
            .into_iter()
            .next()
            .flatten()
            .map(|(_, data)| data)
            .ok_or_else(|| Error::Parse {
                argv: vec!["cat-file".into(), "--batch".into()],
                detail: format!("{name} is missing"),
            })
    }

    /// Every entry of `commit`'s tree, files and gitlinks, by path.
    pub async fn tree(&self, commit: &str) -> Result<Tree, Error> {
        let out = self
            .repo
            .run(
                &["ls-tree", "-r", "-z", "--full-tree", commit],
                None,
                pigeon_git::DEFAULT_TIMEOUT,
            )
            .await?;
        let mut tree = Tree::new();
        for rec in out.stdout.split(|b| *b == 0) {
            let rec = String::from_utf8_lossy(rec);
            let Some((meta, path)) = rec.split_once('\t') else {
                continue;
            };
            if let [mode, kind, oid] = meta.split(' ').collect::<Vec<_>>().as_slice() {
                let entry = Entry {
                    mode: (*mode).to_owned(),
                    kind: (*kind).to_owned(),
                    oid: (*oid).to_owned(),
                };
                tree.insert(path.to_owned(), entry);
            }
        }
        Ok(tree)
    }

    /// Every entry of the index at stage 0, by path, as [`Git::tree`]
    /// gives a commit's.
    pub async fn index(&self) -> Result<Tree, Error> {
        let out = self
            .repo
            .run(&["ls-files", "-s", "-z"], None, pigeon_git::DEFAULT_TIMEOUT)
            .await?;
        let mut tree = Tree::new();
        for rec in out.stdout.split(|b| *b == 0) {
            let rec = String::from_utf8_lossy(rec);
            let Some((meta, path)) = rec.split_once('\t') else {
                continue;
            };
            if let [mode, oid, "0"] = meta.split(' ').collect::<Vec<_>>().as_slice() {
                let kind = if *mode == "160000" { "commit" } else { "blob" };
                let entry = Entry {
                    mode: (*mode).to_owned(),
                    kind: kind.to_owned(),
                    oid: (*oid).to_owned(),
                };
                tree.insert(path.to_owned(), entry);
            }
        }
        Ok(tree)
    }

    /// Objects by name, through one `cat-file --batch`: `(kind, data)`, or
    /// `None` for a missing one.
    pub async fn objects(&self, names: &[String]) -> Result<Vec<Option<(String, Vec<u8>)>>, Error> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let mut input = names.join("\n").into_bytes();
        input.push(b'\n');
        let out = self
            .repo
            .run(
                &["cat-file", "--batch"],
                Some(&input),
                pigeon_git::DEFAULT_TIMEOUT,
            )
            .await?
            .stdout;
        let mut rest = out.as_slice();
        let mut found = Vec::with_capacity(names.len());
        while !rest.is_empty() {
            let Some(nl) = rest.iter().position(|b| *b == b'\n') else {
                break;
            };
            let header = String::from_utf8_lossy(rest.get(..nl).unwrap_or_default()).into_owned();
            rest = rest.get(nl + 1..).unwrap_or_default();
            match pigeon_git::catfile::parse_header(&header) {
                Ok(Some((_, kind, size))) => {
                    let data = rest.get(..size).unwrap_or_default().to_vec();
                    rest = rest.get(size + 1..).unwrap_or_default();
                    found.push(Some((kind, data)));
                }
                _ => found.push(None),
            }
        }
        Ok(found)
    }

    /// The bytes of `rev:path`, if it exists.
    pub async fn blob(&self, rev_path: &str) -> Option<Vec<u8>> {
        let mut found = self.objects(&[rev_path.to_owned()]).await.ok()?;
        found.pop().flatten().map(|(_, data)| data)
    }

    /// The files of `commit`'s tree.
    pub async fn tree_files(&self, commit: &str) -> Result<Vec<TreeFile>, Error> {
        let out = self
            .repo
            .run(
                &["ls-tree", "-r", "-z", "--full-tree", commit],
                None,
                pigeon_git::DEFAULT_TIMEOUT,
            )
            .await?;
        let mut files = Vec::new();
        for rec in out.stdout.split(|b| *b == 0) {
            let rec = String::from_utf8_lossy(rec);
            let Some((meta, path)) = rec.split_once('\t') else {
                continue;
            };
            let parts: Vec<&str> = meta.split(' ').collect();
            if let [mode, "blob", oid] = parts.as_slice() {
                files.push(TreeFile {
                    mode: (*mode).to_owned(),
                    oid: (*oid).to_owned(),
                    path: path.to_owned(),
                });
            }
        }
        Ok(files)
    }

    /// The annexed files of `commit`: path → key.
    pub async fn keys(&self, commit: &str) -> Result<BTreeMap<String, String>, Error> {
        let files: Vec<TreeFile> = self
            .tree_files(commit)
            .await?
            .into_iter()
            .filter(|f| !is_pigeon_path(&f.path) && f.mode.starts_with("100"))
            .collect();
        let oids: Vec<String> = files.iter().map(|f| f.oid.clone()).collect();
        let blobs = self.objects(&oids).await?;
        Ok(files
            .into_iter()
            .zip(blobs)
            .filter_map(|(f, b)| Some((f.path, pointer_key(&b?.1)?)))
            .collect())
    }

    /// Keys reachable from every tree in `tips`' histories, including
    /// overwritten and deleted files. No purge is implemented yet, so all
    /// these references retain content. Inspect small blobs only: user
    /// content is represented by annex pointers, never large Git blobs.
    pub async fn retained_keys(&self, tips: &[String]) -> Result<BTreeSet<String>, Error> {
        if tips.is_empty() {
            return Ok(BTreeSet::new());
        }
        let input = format!("{}\n", tips.join("\n"));
        let list = self
            .run_in(
                &["rev-list", "--objects", "--no-object-names", "--stdin"],
                input.as_bytes(),
            )
            .await?;
        let ids: Vec<String> = list.lines().map(str::to_owned).collect();
        let mut keys = BTreeSet::new();
        for chunk in ids.chunks(256) {
            let input = format!("{}\n", chunk.join("\n"));
            let checked = self
                .run_in(
                    &[
                        "cat-file",
                        "--batch-check=%(objectname) %(objecttype) %(objectsize)",
                    ],
                    input.as_bytes(),
                )
                .await?;
            let mut blobs = Vec::new();
            for line in checked.lines() {
                let fields: Vec<&str> = line.split_whitespace().collect();
                match fields.as_slice() {
                    [oid, "blob", size] if size.parse::<usize>().is_ok_and(|n| n <= 2048) => {
                        blobs.push((*oid).to_owned());
                    }
                    [_, "blob" | "tree" | "commit" | "tag", _] => {}
                    _ => {
                        return Err(Error::Parse {
                            argv: vec!["cat-file".to_owned()],
                            detail: format!("retained history has an unreadable object: {line}"),
                        });
                    }
                }
            }
            let objects = self.objects(&blobs).await?;
            if objects.len() != blobs.len() {
                return Err(Error::Parse {
                    argv: vec!["cat-file".to_owned()],
                    detail: "incomplete retained-history response".to_owned(),
                });
            }
            for object in objects {
                let Some((_, bytes)) = object else {
                    return Err(Error::Parse {
                        argv: vec!["cat-file".to_owned()],
                        detail: "retained history has a missing pointer".to_owned(),
                    });
                };
                if let Some(key) = pointer_key(&bytes) {
                    keys.insert(key);
                }
            }
        }
        Ok(keys)
    }

    /// The committer time of `commit`, in seconds.
    pub async fn commit_time(&self, commit: &str) -> Option<i64> {
        let data = self.blob(commit).await?;
        let text = String::from_utf8_lossy(&data);
        let line = text.lines().find(|l| l.starts_with("committer "))?;
        let mut words = line.rsplit(' ');
        words.next()?;
        words.next()?.parse().ok()
    }

    /// The value of a configuration key.
    pub async fn config(&self, key: &str) -> Option<String> {
        let out = self.run(&["config", "--get", key]).await.ok()?;
        let v = out.trim();
        (!v.is_empty()).then(|| v.to_owned())
    }
}
