//! Idle maintenance children (ADR 0037, 15 §15.1). On Linux an fsck child
//! enters the idle I/O class and nice 19 before exec; the daemon's own
//! priority is unchanged. Failure prevents the maintenance child starting.

use std::process::Command;

/// Apply the idle maintenance class to this child, inherited across exec.
/// Platforms without the required primitive fail rather than silently
/// running a scrub at foreground I/O priority.
pub fn idle_on_spawn(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: the child closure captures nothing and performs only direct
    // scheduling syscalls and errno reads before exec: no allocation,
    // mutex, environment access or parent-runtime work occurs after fork.
    unsafe { command.pre_exec(idle_current) };
}

#[cfg(target_os = "linux")]
fn idle_current() -> std::io::Result<()> {
    // SAFETY: both scheduling calls select only the calling process;
    // all values are scalar Linux API constants, with no borrowed memory.
    let nice = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 19) };
    if nice != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // IOPRIO_WHO_PROCESS=1, IOPRIO_CLASS_IDLE=3, class shift=13.
    // SAFETY: valid scalar arguments to the Linux ioprio_set syscall.
    let io = unsafe { libc::syscall(libc::SYS_ioprio_set, 1, 0, 3 << 13) };
    if io != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn idle_current() -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "real inherited child scheduling fixture"
    )]
    fn idle_child_inherits_both_priorities_and_does_not_change_its_parent() {
        // SAFETY: getpriority and ioprio_get only read scalar scheduling
        // state of the calling process, and receive valid API constants.
        let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, 0) };
        // SAFETY: IOPRIO_WHO_PROCESS=1 and pid=0 select this process.
        let io = unsafe { libc::syscall(libc::SYS_ioprio_get, 1, 0) };
        if std::env::var_os("PIGEON_PRIORITY_TEST_CHILD").is_some() {
            assert_eq!(nice, 19);
            assert_eq!(io >> 13, 3);
            return;
        }
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "priority::tests::idle_child_inherits_both_priorities_and_does_not_change_its_parent"])
            .env("PIGEON_PRIORITY_TEST_CHILD", "1");
        idle_on_spawn(&mut command);
        assert!(command.status().unwrap().success());
        // SAFETY: the same read-only scheduling calls as above.
        assert_eq!(unsafe { libc::getpriority(libc::PRIO_PROCESS, 0) }, nice);
        // SAFETY: scalar ioprio_get arguments select this process again.
        assert_eq!(unsafe { libc::syscall(libc::SYS_ioprio_get, 1, 0) }, io);
    }
}
