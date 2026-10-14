//! The JSON answers of git-annex that pigeon reads, as typed records, and the
//! batch processes that ask for them (03 §3.7). Field names are git-annex's;
//! the contract tests pin them against the minimum version (13 §13.6).

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::batch::Batch;
use crate::env::GitEnv;
use crate::error::{Error, Result};

/// The fields every git-annex JSON answer carries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Answer {
    /// The subcommand, such as `drop`.
    pub command: String,
    /// Whether it succeeded for this input.
    pub success: bool,
    /// The input line it answers.
    pub input: Vec<String>,
    /// The key it acted on, if any.
    pub key: Option<String>,
    /// The file it acted on, if any.
    pub file: Option<String>,
    /// git-annex's note, such as why a drop was unsafe.
    pub note: Option<String>,
    /// Error messages (with `--json-error-messages`).
    #[serde(rename = "error-messages")]
    pub error_messages: Vec<String>,
}

/// One repository holding a copy, in a `whereis` answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Location {
    /// The repository's UUID.
    pub uuid: String,
    /// Its description.
    pub description: String,
    /// Whether it is this repository.
    pub here: bool,
}

/// A `whereis` answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Whereis {
    /// The common fields.
    #[serde(flatten)]
    pub answer: Answer,
    /// Repositories that git-annex's location log says hold a copy.
    pub whereis: Vec<Location>,
    /// Untrusted ones, which never count.
    pub untrusted: Vec<Location>,
}

/// A `find --json` record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Found {
    /// The file.
    pub file: String,
    /// Its key.
    pub key: String,
    /// The key's size in bytes, as git-annex writes it (a string).
    pub bytesize: String,
    /// The key's backend, such as `SHA256E`.
    pub backend: String,
}

/// Which batch subcommand a [`Annex`] process runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Op {
    /// `add --no-check-gitignore --batch --json`: files in, keys out.
    Add,
    /// `whereis --batch-keys --json`.
    Whereis,
    /// `get --batch-keys --json`.
    Get,
    /// `copy --to <remote> --batch-keys --json`.
    CopyTo,
    /// `drop --batch-keys --json --numcopies K --mincopies K`.
    Drop,
}

/// The arguments of the batch process for `op`.
pub fn argv(op: Op, remote: Option<&str>, k: u32) -> Vec<String> {
    let mut v: Vec<String> = vec!["annex".into()];
    let k = k.to_string();
    let tail: Vec<&str> = match op {
        Op::Add => vec![
            "add",
            "--no-check-gitignore",
            "--batch",
            "--json",
            "--json-error-messages",
        ],
        Op::Whereis => vec!["whereis", "--batch-keys", "--json", "--json-error-messages"],
        Op::Get => vec!["get", "--batch-keys", "--json", "--json-error-messages"],
        Op::CopyTo => vec![
            "copy",
            "--to",
            remote.unwrap_or(""),
            "--batch-keys",
            "--json",
            "--json-error-messages",
        ],
        Op::Drop => vec![
            "drop",
            "--batch-keys",
            "--json",
            "--json-error-messages",
            "--numcopies",
            &k,
            "--mincopies",
            &k,
        ],
    };
    v.extend(tail.into_iter().map(str::to_owned));
    v
}

/// The git-annex batch processes of one repository, one per subcommand.
pub struct Annex {
    dir: PathBuf,
    env: GitEnv,
    /// K, the copies a drop must verify elsewhere (ADRs 0002, 0043).
    pub k: u32,
    batches: Vec<((Op, Option<String>), Batch)>,
}

impl Annex {
    /// The processes of the repository at `dir`, each started when first used.
    pub fn new(dir: impl Into<PathBuf>, env: GitEnv, k: u32) -> Annex {
        Annex {
            dir: dir.into(),
            env,
            k,
            batches: Vec::new(),
        }
    }

    fn batch(&mut self, op: Op, remote: Option<&str>) -> Result<&mut Batch> {
        let id = (op, remote.map(str::to_owned));
        if let Some(i) = self.batches.iter().position(|(b, _)| *b == id) {
            return Ok(&mut self.batches[i].1);
        }
        let args = argv(op, remote, self.k);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let batch = Batch::new(self.dir.clone(), self.env.clone(), &refs)?;
        self.batches.push((id, batch));
        let last = self.batches.len() - 1;
        Ok(&mut self.batches[last].1)
    }

    async fn ask<T: serde::de::DeserializeOwned>(
        &mut self,
        op: Op,
        remote: Option<&str>,
        line: &str,
        timeout: Duration,
    ) -> Result<Option<T>> {
        self.batch(op, remote)?.call_json(line, timeout).await
    }

    /// Captures the file at `path` (relative to the working tree) and returns
    /// git-annex's answer, whose `key` is the new key; `None` when git-annex
    /// had nothing to do (no such file).
    pub async fn add(&mut self, path: &str, timeout: Duration) -> Result<Option<Answer>> {
        self.ask(Op::Add, None, path, timeout).await
    }

    /// Where git-annex's location log says `key` is.
    pub async fn whereis(&mut self, key: &str, timeout: Duration) -> Result<Option<Whereis>> {
        self.ask(Op::Whereis, None, key, timeout).await
    }

    /// Fetches `key` into this repository.
    pub async fn get(&mut self, key: &str, timeout: Duration) -> Result<Option<Answer>> {
        self.ask(Op::Get, None, key, timeout).await
    }

    /// Sends `key` to `remote`.
    pub async fn copy_to(
        &mut self,
        remote: &str,
        key: &str,
        timeout: Duration,
    ) -> Result<Option<Answer>> {
        if remote.is_empty() || remote.starts_with('-') {
            return Err(Error::Refused {
                argv: argv(Op::CopyTo, Some(remote), self.k),
                rule: "a remote name",
            });
        }
        self.ask(Op::CopyTo, Some(remote), key, timeout).await
    }

    /// Drops the local copy of `key` if git-annex verifies K copies
    /// elsewhere (by locking them, ADR 0054). `success` is false otherwise.
    pub async fn drop_local(&mut self, key: &str, timeout: Duration) -> Result<Option<Answer>> {
        self.ask(Op::Drop, None, key, timeout).await
    }

    /// The pids of the running batch processes, for pausing them (ADR 0037).
    pub fn pids(&self) -> Vec<u32> {
        self.batches.iter().filter_map(|(_, b)| b.pid()).collect()
    }

    /// Stops every batch process.
    pub async fn stop(&mut self) {
        for (_, b) in &mut self.batches {
            b.stop().await;
        }
    }
}

/// Parses a git-annex key's size field (`-s<bytes>`), if it has one.
pub fn key_size(key: &str) -> Option<u64> {
    let (fields, _) = key.split_once("--")?;
    fields
        .split('-')
        .find_map(|f| f.strip_prefix('s')?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_parse_from_git_annex_json() {
        let drop = r#"{"command":"drop","error-messages":[],"input":["K"],"key":"K","note":"unsafe","success":false,"wanted":[]}"#;
        let a: Answer = serde_json::from_str(drop).unwrap();
        assert!(!a.success && a.key.as_deref() == Some("K") && a.note.as_deref() == Some("unsafe"));
        let w = r#"{"command":"whereis","error-messages":[],"input":["K"],"key":"K","success":true,"untrusted":[],"whereis":[{"description":"t","here":true,"urls":[],"uuid":"u1"}]}"#;
        let w: Whereis = serde_json::from_str(w).unwrap();
        assert_eq!(
            w.whereis,
            vec![Location {
                uuid: "u1".into(),
                description: "t".into(),
                here: true
            }]
        );
    }

    #[test]
    fn every_batch_command_is_allowed_and_sizes_parse() {
        for op in [Op::Add, Op::Whereis, Op::Get, Op::CopyTo, Op::Drop] {
            assert_eq!(
                crate::allow::check(&argv(op, Some("c"), 2)),
                Ok(()),
                "{op:?}"
            );
        }
        assert_eq!(key_size("SHA256E-s6--5891b5.txt"), Some(6));
        assert_eq!(key_size("WORM-s1048576-m17--x"), Some(1048576));
        assert_eq!(key_size("URL--x"), None);
    }
}
