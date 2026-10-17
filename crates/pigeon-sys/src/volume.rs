//! Native filesystem identity for create/join preflight and doctor (14 §14.2).
//! Linux binds mountinfo to an open descriptor's mount ID, never a pathname
//! prefix. Apple reports its filesystem name and native MNT_LOCAL hint.
//! This is an observation, not permission to write or a capability proof.

use crate::{Error, Result};
use std::fs::File;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Native identity of the filesystem backing a folder or its existing parent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    /// Kernel filesystem name, such as ext4, btrfs, apfs or fuse.sshfs.
    pub name: String,
    /// Native local-filesystem flag where available. Linux has no equivalent
    /// flag; its caller must recognize supported filesystem drivers explicitly.
    pub local: Option<bool>,
}

/// Inspect an existing target or its closest existing ancestor. No folder,
/// repository or probe file is created, and errors never guess a filesystem.
pub fn inspect(path: &Path) -> Result<Volume> {
    let absolute = std::path::absolute(path).map_err(|e| Error::path("volume-path", path, e))?;
    for ancestor in absolute.ancestors() {
        match std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(ancestor)
        {
            Ok(file) => return inspect_file(&file),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::path("volume-open", ancestor, error)),
        }
    }
    Err(Error::path(
        "volume-open",
        path,
        std::io::Error::from(std::io::ErrorKind::NotFound),
    ))
}

fn malformed(detail: &str) -> Error {
    Error::Malformed {
        what: "filesystem identity",
        detail: detail.into(),
    }
}

#[cfg(target_os = "linux")]
fn inspect_file(file: &File) -> Result<Volume> {
    use rustix::fs::{AtFlags, StatxFlags, statx};
    let stat = statx(file, "", AtFlags::EMPTY_PATH, StatxFlags::MNT_ID)
        .map_err(|e| Error::op("volume-statx", e))?;
    if stat.stx_mask & StatxFlags::MNT_ID.bits() == 0 {
        return Err(malformed("kernel did not report a mount ID"));
    }
    let info = std::fs::read_to_string("/proc/self/mountinfo")
        .map_err(|e| Error::path("volume-mounts", "/proc/self/mountinfo", e))?;
    let name = mount_type(&info, stat.stx_mnt_id)?;
    Ok(Volume { name, local: None })
}

#[cfg(target_os = "linux")]
fn mount_type(info: &str, id: u64) -> Result<String> {
    let mut found = None;
    for line in info.lines() {
        if line
            .split_whitespace()
            .next()
            .and_then(|word| word.parse::<u64>().ok())
            != Some(id)
        {
            continue;
        }
        let name = line
            .split_once(" - ")
            .and_then(|(_, right)| right.split_whitespace().next())
            .filter(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_graphic()))
            .ok_or_else(|| malformed("selected mount record is malformed"))?;
        if found.replace(name.to_owned()).is_some() {
            return Err(malformed("mount ID is ambiguous"));
        }
    }
    found.ok_or_else(|| malformed("open filesystem is absent from mountinfo"))
}

#[cfg(target_vendor = "apple")]
fn inspect_file(file: &File) -> Result<Volume> {
    use std::os::fd::AsRawFd;
    let mut result = std::mem::MaybeUninit::<libc::statfs>::zeroed();
    // SAFETY: the descriptor is owned and valid, and result points to a
    // writable native statfs structure for the duration of this call.
    if unsafe { libc::fstatfs(file.as_raw_fd(), result.as_mut_ptr()) } != 0 {
        return Err(Error::op("volume-statfs", std::io::Error::last_os_error()));
    }
    // SAFETY: the structure was zero-initialized, and fstatfs succeeded.
    let result = unsafe { result.assume_init() };
    let name = result
        .f_fstypename
        .iter()
        .map(|b| *b as u8)
        .take_while(|b| *b != 0)
        .collect::<Vec<_>>();
    let name = String::from_utf8(name)
        .ok()
        .filter(|s| !s.is_empty() && s.is_ascii())
        .ok_or_else(|| malformed("filesystem name is unknown"))?;
    Ok(Volume {
        name,
        local: Some(result.f_flags & libc::MNT_LOCAL.cast_unsigned() != 0),
    })
}

#[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
fn inspect_file(_file: &File) -> Result<Volume> {
    Err(Error::op(
        "volume-probe",
        std::io::Error::from(std::io::ErrorKind::Unsupported),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "native filesystem observation fixtures")]
mod tests {
    use super::*;

    #[test]
    fn inspecting_a_missing_folder_observes_its_parent_without_creating_it() {
        let temp = tempfile::tempdir().unwrap();
        let wanted = temp.path().join("missing/nested");
        let parent = inspect(temp.path()).unwrap();
        assert!(!parent.name.is_empty());
        assert_eq!(inspect(&wanted).unwrap(), parent);
        assert!(!temp.path().join("missing").exists());
    }

    #[test]
    fn inspecting_a_fifo_never_waits_for_a_writer() {
        use std::os::unix::ffi::OsStrExt;
        let temp = tempfile::tempdir().unwrap();
        let fifo = temp.path().join("pipe");
        let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: name is a live NUL-terminated path in this isolated
        // fixture; the mode is a valid scalar native permission constant.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), libc::S_IRUSR) }, 0);
        assert_eq!(inspect(&fifo).unwrap(), inspect(temp.path()).unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_native_descriptor_on_proc_is_distinct_from_the_worktree_volume() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(
            inspect(Path::new("/proc/self/status")).unwrap().name,
            "proc"
        );
        assert_ne!(inspect(temp.path()).unwrap().name, "proc");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mount_identity_ignores_path_prefixes_and_refuses_unknown_or_ambiguous_records() {
        let mounts = "24 1 8:1 / / rw - ext4 /dev/sda rw\n25 24 0:8 / /private\\040data rw - fuse.sshfs remote rw\n";
        assert_eq!(mount_type(mounts, 24).unwrap(), "ext4");
        assert_eq!(mount_type(mounts, 25).unwrap(), "fuse.sshfs");
        assert!(mount_type(mounts, 26).is_err());
        assert!(mount_type(&format!("{mounts}{mounts}"), 24).is_err());
        assert!(mount_type("24 1 8:1 malformed", 24).is_err());
    }
}
