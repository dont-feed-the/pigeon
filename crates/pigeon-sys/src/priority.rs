//! Maintenance children (ADR 0037, 15 §15.1). An fsck child enters nice 19
//! and Linux's idle I/O class or Darwin's throttled disk policy before exec.
//! The daemon's priority is unchanged. Failure prevents the child starting.

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

#[cfg(target_os = "macos")]
fn idle_current() -> std::io::Result<()> {
    // SAFETY: only the calling process is selected, with scalar priorities.
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 19) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Darwin sys/resource.h: disk=0, process=0, throttle=3. The policy is
    // inherited across exec; the native child fixture checks that boundary.
    // SAFETY: this libc wrapper receives only documented scalar constants.
    if unsafe { apple::setiopolicy_np(0, 0, 3) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
mod apple {
    unsafe extern "C" {
        pub(super) fn setiopolicy_np(
            kind: libc::c_int,
            scope: libc::c_int,
            policy: libc::c_int,
        ) -> libc::c_int;
        #[cfg(test)]
        pub(super) fn getiopolicy_np(kind: libc::c_int, scope: libc::c_int) -> libc::c_int;
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn idle_current() -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
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
        let io = disk_policy();
        assert!(io >= 0, "native disk policy must be readable");
        if std::env::var_os("PIGEON_PRIORITY_TEST_CHILD").is_some() {
            assert_eq!(nice, 19);
            #[cfg(target_os = "linux")]
            assert_eq!(io >> 13, 3);
            #[cfg(target_os = "macos")]
            assert_eq!(io, 3);
            return;
        }
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "priority::tests::idle_child_inherits_both_priorities_and_does_not_change_its_parent"])
            .env("PIGEON_PRIORITY_TEST_CHILD", "1");
        idle_on_spawn(&mut command);
        assert!(command.status().unwrap().success());
        // SAFETY: the same read-only scheduling calls as above.
        assert_eq!(unsafe { libc::getpriority(libc::PRIO_PROCESS, 0) }, nice);
        assert_eq!(disk_policy(), io);
    }

    fn disk_policy() -> i64 {
        #[cfg(target_os = "linux")]
        // SAFETY: IOPRIO_WHO_PROCESS=1 and pid=0 select this process.
        return unsafe { libc::syscall(libc::SYS_ioprio_get, 1, 0) };
        #[cfg(target_os = "macos")]
        // SAFETY: documented disk/process selectors only read current policy.
        return i64::from(unsafe { apple::getiopolicy_np(0, 0) });
    }
}
