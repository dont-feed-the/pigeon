//! Immutable reference births for purge (08 §8.6). A birth is a path/key
//! absent from every parent, not a new blob or a path in the latest tree.
//! The index extends from exact retained tips and never authorizes release
//! until its entire reading is complete. It writes no repository state.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::purge::Reference;

use crate::git::{Error, Git};

/// Append-only metadata derived from immutable Git objects. The caller
/// must match `tips` against a fresh retention reading before using it.
#[derive(Clone, Debug, Default)]
pub struct Index {
    /// The complete tips covered by the latest successful extension.
    pub tips: BTreeSet<String>,
    /// Every known retained reference birth, including proposals/archives.
    pub references: BTreeSet<Reference<String, String>>,
    parents: BTreeMap<String, Vec<String>>,
    pointers: BTreeMap<String, Option<String>>,
}

fn invalid(detail: &str) -> Error {
    Error::Parse {
        argv: vec!["purge reference index".into()],
        detail: detail.into(),
    }
}

impl Index {
    async fn pointers(
        &mut self,
        git: &Git,
        ids: impl Iterator<Item = String>,
    ) -> Result<(), Error> {
        let missing: BTreeSet<String> = ids.filter(|id| !self.pointers.contains_key(id)).collect();
        let missing: Vec<String> = missing.into_iter().collect();
        for chunk in missing.chunks(256) {
            let found = git.objects(chunk).await?;
            if found.len() != chunk.len() {
                return Err(invalid("incomplete pointer batch"));
            }
            for (id, object) in chunk.iter().zip(found) {
                let Some((kind, bytes)) = object else {
                    return Err(invalid("missing historical blob"));
                };
                if kind != "blob" {
                    return Err(invalid("a pointer entry is not a blob"));
                }
                let key = pigeon_core::authority::is_pointer(&bytes)
                    .then(|| crate::git::pointer_key(&bytes))
                    .flatten();
                self.pointers.insert(id.clone(), key);
            }
        }
        Ok(())
    }

    /// Extend from every retained tip. Partial failures leave `tips`
    /// unchanged, so callers cannot treat a truncated history as complete.
    pub async fn extend(&mut self, git: &Git, tips: &[String]) -> Result<(), Error> {
        if tips.is_empty() || tips.iter().any(|tip| !crate::purge_file::commit_id(tip)) {
            return Err(invalid("no accepted retention tips"));
        }
        let mut args = vec!["rev-list", "--reverse", "--topo-order", "--parents"];
        args.extend(tips.iter().map(String::as_str));
        let list = git.run(&args).await?;
        for line in list.lines() {
            let mut words = line.split_whitespace();
            let Some(id) = words.next() else {
                return Err(invalid("empty commit record"));
            };
            if self.parents.contains_key(id) {
                continue;
            }
            let parents: Vec<String> = words.map(str::to_owned).collect();
            if !crate::purge_file::commit_id(id)
                || parents.iter().any(|p| !self.parents.contains_key(p))
            {
                return Err(invalid("incomplete parent order"));
            }
            let mut args = vec![
                "diff-tree",
                "--root",
                "-r",
                "--raw",
                "-z",
                "--no-renames",
                "--no-abbrev",
                "--no-commit-id",
            ];
            if let Some(parent) = parents.first() {
                args.push(parent);
            }
            args.push(id);
            let output = git
                .repo
                .run(&args, None, pigeon_git::DEFAULT_TIMEOUT)
                .await?;
            let changes = changed(&output.stdout)?;
            self.pointers(git, changes.values().cloned()).await?;
            let mut candidates: BTreeMap<String, String> = changes
                .into_iter()
                .filter_map(|(path, oid)| {
                    self.pointers
                        .get(&oid)
                        .and_then(Clone::clone)
                        .map(|key| (path, key))
                })
                .collect();
            let paths: Vec<String> = candidates.keys().cloned().collect();
            for parent in &parents {
                let entries = at_paths(git, parent, &paths).await?;
                self.pointers(git, entries.values().cloned()).await?;
                candidates.retain(|path, key| {
                    entries
                        .get(path)
                        .and_then(|oid| self.pointers.get(oid))
                        .and_then(Option::as_ref)
                        != Some(key)
                });
            }
            self.references
                .extend(candidates.into_iter().map(|(path, key)| Reference {
                    birth: id.to_owned(),
                    path,
                    key,
                }));
            self.parents.insert(id.to_owned(), parents);
            tokio::task::yield_now().await;
        }
        self.tips = tips.iter().cloned().collect();
        Ok(())
    }

    /// Exact ancestry including the tip. Unknown objects fail closed.
    pub fn ancestors(&self, tip: &str) -> Option<BTreeSet<String>> {
        let mut todo = vec![tip.to_owned()];
        let mut seen = BTreeSet::new();
        while let Some(id) = todo.pop() {
            if seen.insert(id.clone()) {
                todo.extend(self.parents.get(&id)?.iter().cloned());
            }
        }
        Some(seen)
    }
}

fn changed(raw: &[u8]) -> Result<BTreeMap<String, String>, Error> {
    let mut chunks = raw.split(|b| *b == 0);
    let mut entries = BTreeMap::new();
    while let Some(meta) = chunks.next().filter(|m| !m.is_empty()) {
        let Some(path) = chunks.next() else {
            return Err(invalid("missing changed path"));
        };
        let meta = std::str::from_utf8(meta).map_err(|_| invalid("non-ASCII diff record"))?;
        let path = std::str::from_utf8(path).map_err(|_| invalid("nonportable historical path"))?;
        let fields: Vec<&str> = meta.split(' ').collect();
        if fields.len() != 5 || !meta.starts_with(':') {
            return Err(invalid("malformed raw diff"));
        }
        if matches!(fields[1], "100644" | "100755") && !crate::git::is_pigeon_path(path) {
            entries.insert(path.to_owned(), fields[3].to_owned());
        }
    }
    Ok(entries)
}

async fn at_paths(
    git: &Git,
    commit: &str,
    paths: &[String],
) -> Result<BTreeMap<String, String>, Error> {
    let mut entries = BTreeMap::new();
    for chunk in paths.chunks(128) {
        let literals: Vec<String> = chunk.iter().map(|p| format!(":(literal){p}")).collect();
        let mut args = vec!["ls-tree", "-r", "-z", "--full-tree", commit, "--"];
        args.extend(literals.iter().map(String::as_str));
        let output = git
            .repo
            .run(&args, None, pigeon_git::DEFAULT_TIMEOUT)
            .await?;
        for row in output.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let row = std::str::from_utf8(row).map_err(|_| invalid("nonportable parent path"))?;
            let Some((meta, path)) = row.split_once('\t') else {
                return Err(invalid("malformed parent entry"));
            };
            let fields: Vec<&str> = meta.split(' ').collect();
            if fields.len() != 3 || fields[1] != "blob" {
                return Err(invalid("malformed parent blob"));
            }
            entries.insert(path.to_owned(), fields[2].to_owned());
        }
    }
    Ok(entries)
}
