//! Pausing, resuming and stopping pigeon's children (ADR 0037): when others
//! use the disk, `pigeond` stops its heavy git-annex children with `SIGSTOP`
//! and resumes them with `SIGCONT`, so hashing and transfers pause mid-file.

use rustix::process::{Pid, Signal, kill_process, kill_process_group};

use crate::{Error, Result};

/// What to do with a child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// `SIGSTOP`: pause at once, wherever it is.
    Pause,
    /// `SIGCONT`: resume a paused child.
    Resume,
    /// `SIGTERM`: ask it to end.
    Terminate,
    /// `SIGKILL`: end it now; for a child past its deadline.
    Kill,
}

impl Action {
    fn signal(self) -> Signal {
        match self {
            Action::Pause => Signal::STOP,
            Action::Resume => Signal::CONT,
            Action::Terminate => Signal::TERM,
            Action::Kill => Signal::KILL,
        }
    }
}

fn pid_of(pid: u32) -> Result<Pid> {
    i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or_else(|| Error::Malformed {
            what: "pid",
            detail: pid.to_string(),
        })
}

/// Sends `action` to the process `pid`. A process that already ended is not
/// an error: there is nothing left to pause or stop.
pub fn send(pid: u32, action: Action) -> Result<()> {
    match kill_process(pid_of(pid)?, action.signal()) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
        Err(e) => Err(Error::op("kill", e)),
    }
}

/// Sends `action` to every process of the group `pgid`, such as a
/// git-annex process and the git processes it started.
pub fn send_group(pgid: u32, action: Action) -> Result<()> {
    match kill_process_group(pid_of(pgid)?, action.signal()) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
        Err(e) => Err(Error::op("killpg", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn a_child_pauses_resumes_and_ends() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        send(pid, Action::Pause).unwrap();
        send(pid, Action::Resume).unwrap();
        send(pid, Action::Terminate).unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
        send(pid, Action::Kill).unwrap();
        assert!(
            send(0, Action::Pause).is_err(),
            "pid 0 means a group: refused"
        );
    }
}
