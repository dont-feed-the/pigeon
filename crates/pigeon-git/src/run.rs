//! One-shot git and git-annex calls with a deadline. A call past its deadline
//! is killed; a failed call keeps its command, exit status and stderr tail.
//!
//! The kill is a `TERM` first, which git passes on to the git-annex it runs
//! (`clean_on_exit` in git's `run-command.c`), then a `KILL` after a grace
//! period: a `KILL` alone ends git and orphans git-annex, which then keeps
//! its transfer locks (S5).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use crate::env::GitEnv;
use crate::error::{Error, Result, tail};

/// The deadline of a one-shot call unless the caller gives one.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// How long a call past its deadline has to end after `TERM`.
const GRACE: Duration = Duration::from_secs(10);

/// How long a call runs before the log names it (`git-slow`).
const SLOW: Duration = Duration::from_secs(20);

/// `argv` as one line for the log, cut after 200 characters.
fn shown(argv: &[String]) -> String {
    let line = argv.join(" ");
    match line.char_indices().nth(200) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line,
    }
}

/// A repository pigeon drives: its directory and the children's environment.
#[derive(Clone, Debug)]
pub struct Repo {
    /// The working tree, or the bare repository's directory.
    pub dir: PathBuf,
    /// How children are started.
    pub env: GitEnv,
}

/// What a successful call printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// Standard output.
    pub stdout: Vec<u8>,
    /// Standard error, which git-annex fills with progress and notes.
    pub stderr: Vec<u8>,
}

impl Output {
    /// Standard output as text; git's output is UTF-8 for UTF-8 names.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

/// Starts `git` with `args` in `dir`, after [`crate::allow::check`].
pub(crate) fn command(dir: &Path, env: &GitEnv, args: &[String]) -> Result<Command> {
    crate::allow::check(args).map_err(|rule| Error::Refused {
        argv: args.to_vec(),
        rule,
    })?;
    let mut cmd = Command::new("git");
    env.apply(&mut cmd);
    cmd.args(args).current_dir(dir).kill_on_drop(true);
    Ok(cmd)
}

impl Repo {
    /// A repository at `dir` with the environment `env`.
    pub fn new(dir: impl Into<PathBuf>, env: GitEnv) -> Repo {
        Repo {
            dir: dir.into(),
            env,
        }
    }

    /// Runs `git args…` to completion within `timeout`, writing `input` to
    /// its stdin. A non-zero exit is [`Error::Failed`].
    pub async fn run(
        &self,
        args: &[&str],
        input: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<Output> {
        let out = self.run_status(args, input, timeout).await?;
        match out.0 {
            Some(0) => Ok(out.1),
            status => Err(Error::Failed {
                argv: args.iter().map(|s| (*s).to_owned()).collect(),
                status,
                stderr_tail: tail(&out.1.stderr),
            }),
        }
    }

    /// Starts a long-running `git args…`, such as `annex remotedaemon
    /// --foreground`, after [`crate::allow::check`]: its stdin is a pipe whose
    /// closing stops it, its stdout a pipe the caller reads (remotedaemon's
    /// messages), its stderr is discarded, and it is killed on drop.
    /// It leads a process group of its own, so that the fetches it starts
    /// can be stopped with it (13 §13.12).
    pub fn spawn(&self, args: &[&str]) -> Result<tokio::process::Child> {
        let argv: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let mut cmd = command(&self.dir, &self.env, &argv)?;
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .kill_on_drop(true);
        cmd.spawn().map_err(|source| Error::Spawn { argv, source })
    }

    /// Like [`Repo::run`], but returns the exit code (or `None` for a
    /// signal) instead of failing on a non-zero one, for commands whose code
    /// is an answer, such as `matchexpression`.
    pub async fn run_status(
        &self,
        args: &[&str],
        input: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<(Option<i32>, Output)> {
        let argv: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let mut cmd = command(&self.dir, &self.env, &argv)?;
        cmd.stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|source| Error::Spawn {
            argv: argv.clone(),
            source,
        })?;
        let stdin = child.stdin.take();
        let no_pipe = || Error::Pipe {
            argv: argv.clone(),
            source: std::io::Error::other("no output pipe"),
        };
        let mut stdout = child.stdout.take().ok_or_else(no_pipe)?;
        let mut stderr = child.stderr.take().ok_or_else(no_pipe)?;
        let pid = child
            .id()
            .and_then(|p| i32::try_from(p).ok())
            .and_then(rustix::process::Pid::from_raw);
        let work = async {
            let feed = async {
                if let (Some(mut pipe), Some(bytes)) = (stdin, input) {
                    pipe.write_all(bytes).await?;
                    pipe.shutdown().await?;
                }
                Ok::<_, std::io::Error>(())
            };
            let (mut out, mut err) = (Vec::new(), Vec::new());
            // A batch may produce more than a pipeful of output before it
            // consumes all its input. Drain both streams while feeding it.
            tokio::try_join!(
                feed,
                stdout.read_to_end(&mut out),
                stderr.read_to_end(&mut err)
            )?;
            // Reap only after all I/O completes, so timeout signalling still
            // refers to this child rather than a potentially reused PID.
            let status = child.wait().await?;
            Ok::<_, std::io::Error>(std::process::Output {
                status,
                stdout: out,
                stderr: err,
            })
        };
        tokio::pin!(work);
        // A call still running after SLOW is named in the log, so that a
        // hang shows before its deadline ends it.
        let done = match tokio::time::timeout(SLOW.min(timeout), &mut work).await {
            Err(_) if timeout > SLOW => {
                tracing::info!(argv = %shown(&argv), secs = SLOW.as_secs(), "git-slow");
                tokio::time::timeout(timeout - SLOW, &mut work).await
            }
            done => done,
        };
        let out = match done {
            Err(_) => {
                // Not yet waited for, so the pid is still the child's.
                if let Some(pid) = pid {
                    let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
                }
                let _ = tokio::time::timeout(GRACE, &mut work).await;
                return Err(Error::Timeout {
                    argv,
                    after: timeout,
                });
            }
            Ok(Err(source)) => return Err(Error::Pipe { argv, source }),
            Ok(Ok(out)) => out,
        };
        Ok((
            out.status.code(),
            Output {
                stdout: out.stdout,
                stderr: out.stderr,
            },
        ))
    }
}
