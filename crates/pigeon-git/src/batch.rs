//! Long-running `--batch` processes: one line in, one line out (03 §3.7,
//! ADR 0004). git-annex answers every input line with one JSON line, or a
//! blank line when it has nothing to do for it; the contract tests pin this.
//! A call past its deadline kills the process; the next call starts a new
//! one, so one stuck transfer never blocks the queue (ADR 0056).

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

use crate::env::GitEnv;
use crate::error::{Error, Result};

struct Running {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

/// One long-running batch process, restarted when it fails.
pub struct Batch {
    dir: PathBuf,
    env: GitEnv,
    argv: Vec<String>,
    running: Option<Running>,
    /// How many times the process was started.
    pub starts: u32,
}

impl Batch {
    /// A batch process `git argv…` in `dir`, started at the first call. The
    /// arguments are checked now, so a refused command fails at once.
    pub fn new(dir: impl Into<PathBuf>, env: GitEnv, argv: &[&str]) -> Result<Batch> {
        let argv: Vec<String> = argv.iter().map(|s| (*s).to_owned()).collect();
        crate::allow::check(&argv).map_err(|rule| Error::Refused {
            argv: argv.clone(),
            rule,
        })?;
        Ok(Batch {
            dir: dir.into(),
            env,
            argv,
            running: None,
            starts: 0,
        })
    }

    fn start(&mut self) -> Result<&mut Running> {
        if self.running.is_none() {
            let mut cmd = crate::run::command(&self.dir, &self.env, &self.argv)?;
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let mut child = cmd.spawn().map_err(|source| Error::Spawn {
                argv: self.argv.clone(),
                source,
            })?;
            let broken = || std::io::Error::other("no pipe");
            let pipe_err = |source| Error::Pipe {
                argv: self.argv.clone(),
                source,
            };
            let stdin = child.stdin.take().ok_or_else(|| pipe_err(broken()))?;
            let stdout = BufReader::new(child.stdout.take().ok_or_else(|| pipe_err(broken()))?);
            self.starts += 1;
            self.running = Some(Running {
                child,
                stdin,
                stdout,
            });
        }
        self.running.as_mut().ok_or_else(|| Error::Pipe {
            argv: self.argv.clone(),
            source: std::io::Error::other("not running"),
        })
    }

    /// Sends one input line and returns the answering line without its
    /// newline (empty for a blank answer). On a deadline, an I/O error or the
    /// end of the process, the process is killed and the error returned.
    pub async fn call(&mut self, line: &str, timeout: Duration) -> Result<String> {
        if line.contains('\n') {
            return Err(Error::Refused {
                argv: self.argv.clone(),
                rule: "one line per input",
            });
        }
        let argv = self.argv.clone();
        let running = self.start()?;
        let exchange = async {
            running
                .stdin
                .write_all(format!("{line}\n").as_bytes())
                .await?;
            running.stdin.flush().await?;
            let mut answer = String::new();
            let n = running.stdout.read_line(&mut answer).await?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "process ended",
                ));
            }
            Ok(answer.trim_end_matches('\n').to_owned())
        };
        let result = tokio::time::timeout(timeout, exchange).await;
        match result {
            Ok(Ok(answer)) => Ok(answer),
            Ok(Err(source)) => {
                self.stop().await;
                Err(Error::Pipe { argv, source })
            }
            Err(_) => {
                self.stop().await;
                Err(Error::Timeout {
                    argv,
                    after: timeout,
                })
            }
        }
    }

    /// Sends one line and parses the JSON answer; `None` for a blank answer.
    pub async fn call_json<T: serde::de::DeserializeOwned>(
        &mut self,
        line: &str,
        timeout: Duration,
    ) -> Result<Option<T>> {
        let answer = self.call(line, timeout).await?;
        if answer.trim().is_empty() {
            return Ok(None);
        }
        serde_json::from_str(&answer)
            .map(Some)
            .map_err(|e| Error::Parse {
                argv: self.argv.clone(),
                detail: format!("{e}: {answer}"),
            })
    }

    /// Ends the process: closes its stdin, then kills it.
    pub async fn stop(&mut self) {
        if let Some(mut running) = self.running.take() {
            drop(running.stdin);
            let _ = running.child.start_kill();
            let _ = running.child.wait().await;
        }
    }

    /// The process id of the running process, for pausing it (ADR 0037).
    pub fn pid(&self) -> Option<u32> {
        self.running.as_ref().and_then(|r| r.child.id())
    }
}
