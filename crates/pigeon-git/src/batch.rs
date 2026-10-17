//! Long-running `--batch` processes: one line in, one line out (03 §3.7,
//! ADR 0004). git-annex answers every input line with one JSON line, or a
//! blank line when it has nothing to do for it; the contract tests pin this.
//! A call past its deadline kills the process; the next call starts a new
//! one, so one stuck transfer never blocks the queue (03 §3.7).

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

use crate::env::GitEnv;
use crate::error::{Error, Result};

struct Running {
    group: crate::work::Group,
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
    control: Option<std::sync::Arc<dyn crate::work::Control>>,
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
            control: None,
            starts: 0,
        })
    }

    pub(crate) fn set_work_control(
        &mut self,
        control: Option<std::sync::Arc<dyn crate::work::Control>>,
    ) {
        self.control = control;
    }

    fn start(&mut self) -> Result<&mut Running> {
        if self.running.is_none() {
            let mut cmd = crate::run::command(&self.dir, &self.env, &self.argv)?;
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .process_group(0);
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
            let pid = child
                .id()
                .and_then(|p| i32::try_from(p).ok())
                .and_then(rustix::process::Pid::from_raw)
                .ok_or_else(|| pipe_err(broken()))?;
            self.running = Some(Running {
                group: crate::work::Group::new(pid),
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
        if line.contains(['\n', '\r', '\0']) {
            return Err(Error::Refused {
                argv: self.argv.clone(),
                rule: "one line per input",
            });
        }
        let argv = self.argv.clone();
        let control = self.control.clone();
        let args: Vec<_> = argv.iter().map(String::as_str).collect();
        let controlled = control.as_deref().zip(crate::work::class(&args));
        let _registration = controlled.map(|(control, class)| {
            control.began(class);
            crate::work::Registration(control, class)
        });
        while controlled.is_some_and(|(control, class)| !control.running(class)) {
            tokio::time::sleep(Duration::from_millis(crate::work::POLL_MS)).await;
        }
        self.start()?;
        // The exchange owns its process. Cancelling a partially written or
        // read call must kill it, never hand a stale answer to the next key.
        let Some(mut running) = self.running.take() else {
            return Err(Error::Pipe {
                argv,
                source: std::io::Error::other("not running"),
            });
        };
        let result = {
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
            tokio::pin!(exchange);
            crate::work::deadline(
                &mut exchange,
                &running.group,
                controlled,
                timeout,
                Duration::from_secs(20),
                &argv.join(" "),
            )
            .await
        };
        self.running = Some(running);
        match result {
            Ok(Some(Ok(answer))) => Ok(answer),
            Err(source) | Ok(Some(Err(source))) => {
                self.kill().await;
                Err(Error::Pipe { argv, source })
            }
            Ok(None) => {
                self.kill().await;
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

    /// Ends the process: closes its stdin and lets it finish, then kills it
    /// if it is still running after `grace`. git-annex stages what it queued
    /// for the index only as it ends (Annex/Queue.hs), so a batch `add` is in
    /// the index only after this. True when the process ended on its own.
    pub async fn stop(&mut self, grace: Duration) -> bool {
        let Some(mut running) = self.running.take() else {
            return true;
        };
        let _ = running.group.running(true);
        drop(running.stdin);
        let ended = tokio::time::timeout(grace, running.child.wait()).await;
        if matches!(ended, Ok(Ok(_))) {
            running.group.reaped();
            return true;
        }
        running.group.kill();
        if running.child.wait().await.is_ok() {
            running.group.reaped();
        }
        false
    }

    /// Kills the process at once, for one past its deadline.
    async fn kill(&mut self) {
        if let Some(mut running) = self.running.take() {
            running.group.kill();
            if running.child.wait().await.is_ok() {
                running.group.reaped();
            }
        }
    }

    /// The process id of the running process, for pausing it (ADR 0037).
    pub fn pid(&self) -> Option<u32> {
        self.running.as_ref().and_then(|r| r.child.id())
    }
}
