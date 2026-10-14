//! `git cat-file --batch`: objects by name, read through one long-running
//! process per repository (03 §3.7).

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

use crate::env::GitEnv;
use crate::error::{Error, Result};

/// An object read from the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    /// Its id.
    pub oid: String,
    /// `blob`, `tree`, `commit` or `tag`.
    pub kind: String,
    /// Its content.
    pub data: Vec<u8>,
}

/// Parses the header line `<oid> <type> <size>`, or `<name> missing`.
pub fn parse_header(line: &str) -> std::result::Result<Option<(String, String, usize)>, String> {
    let words: Vec<&str> = line.split(' ').collect();
    match words.as_slice() {
        [_, "missing"] | [_, "ambiguous"] => Ok(None),
        [oid, kind, size] => {
            let size = size.parse().map_err(|_| format!("bad size in {line:?}"))?;
            Ok(Some(((*oid).to_owned(), (*kind).to_owned(), size)))
        }
        _ => Err(format!("bad header {line:?}")),
    }
}

/// One `git cat-file --batch` process, restarted after a failure.
pub struct CatFile {
    dir: PathBuf,
    env: GitEnv,
    running: Option<(Child, ChildStdin, BufReader<ChildStdout>)>,
}

const ARGV: [&str; 2] = ["cat-file", "--batch"];

impl CatFile {
    /// A reader of the repository at `dir`, started at the first call.
    pub fn new(dir: impl Into<PathBuf>, env: GitEnv) -> CatFile {
        CatFile {
            dir: dir.into(),
            env,
            running: None,
        }
    }

    fn argv() -> Vec<String> {
        ARGV.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Reads the object `name` (an oid, or `rev:path`); `None` if missing.
    pub async fn get(&mut self, name: &str, timeout: Duration) -> Result<Option<Object>> {
        if name.contains('\n') || name.is_empty() {
            return Err(Error::Refused {
                argv: Self::argv(),
                rule: "one name per line",
            });
        }
        if self.running.is_none() {
            let mut cmd = crate::run::command(&self.dir, &self.env, &Self::argv())?;
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let mut child = cmd.spawn().map_err(|source| Error::Spawn {
                argv: Self::argv(),
                source,
            })?;
            let none = || Error::Pipe {
                argv: Self::argv(),
                source: std::io::Error::other("no pipe"),
            };
            let stdin = child.stdin.take().ok_or_else(none)?;
            let stdout = BufReader::new(child.stdout.take().ok_or_else(none)?);
            self.running = Some((child, stdin, stdout));
        }
        let Some((_, stdin, stdout)) = self.running.as_mut() else {
            return Ok(None);
        };
        let exchange = async {
            stdin.write_all(format!("{name}\n").as_bytes()).await?;
            stdin.flush().await?;
            let mut header = String::new();
            if stdout.read_line(&mut header).await? == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "process ended",
                ));
            }
            let parsed =
                parse_header(header.trim_end_matches('\n')).map_err(std::io::Error::other)?;
            let Some((oid, kind, size)) = parsed else {
                return Ok(None);
            };
            let mut data = vec![0u8; size + 1];
            stdout.read_exact(&mut data).await?;
            data.pop();
            Ok(Some(Object { oid, kind, data }))
        };
        match tokio::time::timeout(timeout, exchange).await {
            Ok(Ok(obj)) => Ok(obj),
            Ok(Err(source)) => {
                self.running = None;
                Err(Error::Pipe {
                    argv: Self::argv(),
                    source,
                })
            }
            Err(_) => {
                self.running = None;
                Err(Error::Timeout {
                    argv: Self::argv(),
                    after: timeout,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_parse() {
        assert_eq!(
            parse_header("abc blob 12").unwrap(),
            Some(("abc".to_owned(), "blob".to_owned(), 12))
        );
        assert_eq!(parse_header("HEAD:x missing").unwrap(), None);
        assert!(parse_header("abc blob").is_err() && parse_header("abc blob x").is_err());
    }
}
