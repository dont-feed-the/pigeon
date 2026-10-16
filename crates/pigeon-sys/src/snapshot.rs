//! Independent file snapshots: use a copy-on-write clone when supported and
//! otherwise copy. Neither path shares a mutable inode with the source.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Creates a new private snapshot. Fails if the destination already exists.
pub fn copy(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(target_vendor = "apple")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let from = CString::new(source.as_os_str().as_bytes())?;
        let to = CString::new(destination.as_os_str().as_bytes())?;
        // SAFETY: both paths are valid, NUL-terminated strings. clonefile
        // creates a new inode and refuses an existing destination.
        if unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), 0) } == 0 {
            return Ok(());
        }
    }
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)?;
    #[cfg(target_os = "linux")]
    if rustix::fs::ioctl_ficlone(&output, &input).is_ok() {
        return Ok(());
    }
    if let Err(error) = io::copy(&mut input, &mut output) {
        let _ = std::fs::remove_file(destination);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]

    #[test]
    fn snapshots_survive_in_place_saves_and_never_overwrite_a_destination() {
        let temp = tempfile::tempdir().unwrap();
        let from = temp.path().join("source");
        let to = temp.path().join("snapshot");
        std::fs::write(&from, b"old bytes").unwrap();
        super::copy(&from, &to).unwrap();
        std::fs::write(&from, b"new bytes").unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"old bytes");
        assert!(super::copy(&from, &to).is_err());
        assert_eq!(std::fs::read(&to).unwrap(), b"old bytes");
    }
}
