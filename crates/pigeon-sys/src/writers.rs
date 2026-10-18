//! The writer scan of install (spec 07 §7.5): which processes hold a file open for
//! writing, or map it shared and writable. On Linux it reads `/proc/*/fd`,
//! `fdinfo` and `maps`; on macOS it asks `libproc` for each process's vnode
//! descriptors and memory regions. A process the scan may not inspect is
//! counted, with its credentials where the system tells them, so the caller
//! can judge whether it could write the file.

use crate::fs::{FileId, Stat};

#[path = "writer_set.rs"]
mod set;
pub use set::{Snapshot, snapshot};

/// What a scan found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// Processes that may write the file, by pid.
    pub writers: Vec<u32>,
    /// Processes whose descriptors could not be read (another user's).
    pub unreadable: u32,
    /// Their credentials, from `/proc/<pid>/status` on Linux; `None` for a
    /// process whose credentials could not be read either, and on macOS.
    pub hidden: Vec<Option<Creds>>,
}

/// The credentials the kernel checks a write against: the file-system user
/// ID, and the file-system group ID with the supplementary groups.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creds {
    /// The file-system user ID.
    pub uid: u32,
    /// The file-system group ID and the supplementary groups.
    pub gids: Vec<u32>,
}

impl Creds {
    /// Whether a process of another user could open `st` for writing (07
    /// §7.5 step 5). Root and the owner's own processes are not another
    /// user's; a process in the file's group gets the group bits, any other
    /// the other bits, as the kernel decides.
    pub fn another_may_write(&self, st: &Stat) -> bool {
        if self.uid == 0 || self.uid == st.uid {
            return false;
        }
        if self.gids.contains(&st.gid) {
            st.mode & 0o020 != 0
        } else {
            st.mode & 0o002 != 0
        }
    }
}

/// The credentials in the text of `/proc/<pid>/status`: the fourth (file
/// system) value of `Uid:` and `Gid:`, and `Groups:`.
pub fn status_creds(text: &str) -> Option<Creds> {
    let ids = |key: &str| {
        text.lines().find_map(|l| l.strip_prefix(key)).map(|rest| {
            rest.split_whitespace()
                .filter_map(|v| v.parse::<u32>().ok())
                .collect::<Vec<_>>()
        })
    };
    let uid = *ids("Uid:")?.get(3)?;
    let gid = *ids("Gid:")?.get(3)?;
    let mut gids = ids("Groups:").unwrap_or_default();
    gids.push(gid);
    Some(Creds { uid, gids })
}

impl Scan {
    /// Whether no process can write the file, as far as the scan could see.
    pub fn is_clear(&self) -> bool {
        self.writers.is_empty()
    }

    /// Whether nothing may write `st` (07 §7.5 step 5): no writer seen, and
    /// no process left uninspected that belongs to another user who could
    /// write it. One whose credentials are unknown counts when the file is
    /// group- or other-writable.
    pub fn clear_for(&self, st: &Stat) -> bool {
        self.is_clear()
            && self.hidden.iter().all(|c| match c {
                Some(c) => !c.another_may_write(st),
                None => st.mode & 0o022 == 0,
            })
    }
}

/// Scans every process for writers of the file `id`.
pub fn scan(id: FileId) -> Scan {
    imp::scan(id)
}

/// Whether an `fdinfo` `flags:` value (octal) opens for writing: the access
/// mode `O_WRONLY` (1) or `O_RDWR` (2).
pub fn flags_write(octal: &str) -> bool {
    u32::from_str_radix(octal.trim(), 8).is_ok_and(|f| matches!(f & 0o3, 1 | 2))
}

/// Whether a line of `/proc/<pid>/maps` maps `(major, minor, ino)` shared and
/// writable (`rw-s`).
pub fn maps_line_writes(line: &str, major: u32, minor: u32, ino: u64) -> bool {
    let mut f = line.split_whitespace();
    let (Some(_), Some(perms), Some(_), Some(dev), Some(inode)) =
        (f.next(), f.next(), f.next(), f.next(), f.next())
    else {
        return false;
    };
    let p = perms.as_bytes();
    if p.len() < 4 || p[1] != b'w' || p[3] != b's' {
        return false;
    }
    let Some((ma, mi)) = dev.split_once(':') else {
        return false;
    };
    let dev_ok =
        u32::from_str_radix(ma, 16) == Ok(major) && u32::from_str_radix(mi, 16) == Ok(minor);
    dev_ok && inode.parse::<u64>() == Ok(ino)
}

#[cfg(target_os = "linux")]
mod imp {
    use std::os::unix::fs::MetadataExt;

    use super::{FileId, Scan, flags_write, maps_line_writes, status_creds};

    pub(super) fn scan(id: FileId) -> Scan {
        let mut out = Scan::default();
        let Ok(procs) = std::fs::read_dir("/proc") else {
            out.unreadable += 1;
            out.hidden.push(None);
            return out;
        };
        let (major, minor) = (rustix::fs::major(id.dev), rustix::fs::minor(id.dev));
        for entry in procs.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|n| n.parse::<u32>().ok())
            else {
                continue;
            };
            let base = entry.path();
            let Ok(fds) = std::fs::read_dir(base.join("fd")) else {
                if base.exists() {
                    out.unreadable += 1;
                    let status = std::fs::read_to_string(base.join("status"));
                    out.hidden
                        .push(status.ok().as_deref().and_then(status_creds));
                }
                continue;
            };
            let mut writes = false;
            for fd in fds.flatten() {
                let Ok(meta) = std::fs::metadata(fd.path()) else {
                    continue;
                };
                if meta.dev() != id.dev || meta.ino() != id.ino {
                    continue;
                }
                let info = std::fs::read_to_string(base.join("fdinfo").join(fd.file_name()));
                let flags = info.ok().and_then(|t| {
                    t.lines()
                        .find_map(|l| l.strip_prefix("flags:").map(flags_write))
                });
                if flags != Some(false) {
                    writes = true;
                    break;
                }
            }
            if !writes && let Ok(maps) = std::fs::read_to_string(base.join("maps")) {
                writes = maps
                    .lines()
                    .any(|l| maps_line_writes(l, major, minor, id.ino));
            }
            if writes {
                out.writers.push(pid);
            }
        }
        out
    }
}

#[cfg(target_vendor = "apple")]
mod imp {
    //! `libproc` declarations that the `libc` crate lacks, from
    //! `<sys/proc_info.h>`.
    use std::ffi::{c_int, c_void};
    use std::mem::{MaybeUninit, size_of};

    use super::{FileId, Scan};

    const PROC_PIDLISTFDS: c_int = 1;
    /// The next region backed by a file, skipping the others and submaps
    /// (the shared cache) in the kernel: one call per mapped file, where
    /// `PROC_PIDREGIONPATHINFO` (8) takes one per region of every map.
    const PROC_PIDREGIONPATHINFO2: c_int = 22;
    /// A descriptor's vnode without its path, which the kernel would build.
    const PROC_PIDFDVNODEINFO: c_int = 1;
    const PROX_FDTYPE_VNODE: u32 = 1;
    const FWRITE: u32 = 0x2;
    const VM_PROT_WRITE: u32 = 0x2;
    const VM_INHERIT_SHARE: u32 = 0;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcFileinfo {
        fi_openflags: u32,
        fi_status: u32,
        fi_offset: i64,
        fi_type: i32,
        fi_guardflags: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VnodeFdinfo {
        pfi: ProcFileinfo,
        pvi: libc::vnode_info,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcRegioninfo {
        pri_protection: u32,
        pri_max_protection: u32,
        pri_inheritance: u32,
        pri_flags: u32,
        pri_offset: u64,
        pri_behavior: u32,
        pri_user_wired_count: u32,
        pri_user_tag: u32,
        pri_pages_resident: u32,
        pri_pages_shared_now_private: u32,
        pri_pages_swapped_out: u32,
        pri_pages_dirtied: u32,
        pri_ref_count: u32,
        pri_shadow_depth: u32,
        pri_share_mode: u32,
        pri_private_pages_resident: u32,
        pri_shared_pages_resident: u32,
        pri_obj_id: u32,
        pri_depth: u32,
        pri_address: u64,
        pri_size: u64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcRegionWithPathinfo {
        prp_prinfo: ProcRegioninfo,
        prp_vip: libc::vnode_info_path,
    }

    fn matches(st: &libc::vinfo_stat, id: FileId) -> bool {
        u64::from(st.vst_dev) == (id.dev & 0xffff_ffff) && st.vst_ino == id.ino
    }

    fn pids() -> Vec<c_int> {
        let mut buf: Vec<c_int> = vec![0; 4096];
        loop {
            let bytes = c_int::try_from(buf.len() * size_of::<c_int>()).unwrap_or(c_int::MAX);
            // SAFETY: the buffer is valid for `bytes` bytes and the call writes
            // at most that many; it returns the number of pids written.
            let n = unsafe { libc::proc_listallpids(buf.as_mut_ptr().cast::<c_void>(), bytes) };
            let n = usize::try_from(n).unwrap_or(0);
            if n < buf.len() || buf.len() >= 1 << 20 {
                buf.truncate(n);
                return buf;
            }
            buf.resize(buf.len() * 2, 0);
        }
    }

    fn fds(pid: c_int) -> Option<Vec<libc::proc_fdinfo>> {
        let entry = size_of::<libc::proc_fdinfo>();
        // SAFETY: a null buffer of size 0 asks for the size needed.
        let need = unsafe { libc::proc_pidinfo(pid, PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
        let need = usize::try_from(need).ok().filter(|n| *n > 0)?;
        let mut buf: Vec<libc::proc_fdinfo> = Vec::with_capacity(need / entry + 16);
        let bytes = c_int::try_from(buf.capacity() * entry).ok()?;
        // SAFETY: the buffer has capacity for `bytes` bytes; the call writes
        // whole `proc_fdinfo` records and returns the bytes written, which
        // bounds the length set below.
        let got =
            unsafe { libc::proc_pidinfo(pid, PROC_PIDLISTFDS, 0, buf.as_mut_ptr().cast(), bytes) };
        let got = usize::try_from(got).ok().filter(|n| *n > 0)?;
        // SAFETY: the first `got / entry` records were written by the kernel.
        unsafe { buf.set_len((got / entry).min(buf.capacity())) };
        Some(buf)
    }

    fn fd_writes(pid: c_int, fd: i32, id: FileId) -> bool {
        let mut info = MaybeUninit::<VnodeFdinfo>::zeroed();
        let size = c_int::try_from(size_of::<VnodeFdinfo>()).unwrap_or(0);
        // SAFETY: `info` is valid for `size` bytes, and zeroed so any bytes the
        // call leaves unwritten are a valid value of this plain-data struct.
        let got = unsafe {
            libc::proc_pidfdinfo(pid, fd, PROC_PIDFDVNODEINFO, info.as_mut_ptr().cast(), size)
        };
        if got != size {
            return false;
        }
        // SAFETY: the call filled the whole struct (got == size).
        let info = unsafe { info.assume_init() };
        matches(&info.pvi.vi_stat, id) && info.pfi.fi_openflags & FWRITE != 0
    }

    /// The `(protection, inheritance, share mode)` of every region of `pid`
    /// that maps the file `id` (`fill_procregioninfo_onlymappedvnodes` in
    /// XNU's `osfmk/kern/bsd_kern.c` reports no share mode: 0).
    pub(super) fn regions(pid: c_int, id: FileId) -> Vec<(u32, u32, u32)> {
        let size = c_int::try_from(size_of::<ProcRegionWithPathinfo>()).unwrap_or(0);
        let mut out = Vec::new();
        let mut address: u64 = 0;
        for _ in 0..100_000 {
            let mut info = MaybeUninit::<ProcRegionWithPathinfo>::zeroed();
            // SAFETY: `info` is valid for `size` bytes and zeroed, so it is a
            // valid value of this plain-data struct whatever the call writes.
            let got = unsafe {
                libc::proc_pidinfo(
                    pid,
                    PROC_PIDREGIONPATHINFO2,
                    address,
                    info.as_mut_ptr().cast(),
                    size,
                )
            };
            if got <= 0 {
                break;
            }
            // SAFETY: zeroed plain data, possibly filled in by the call.
            let info = unsafe { info.assume_init() };
            let r = info.prp_prinfo;
            if matches(&info.prp_vip.vip_vi.vi_stat, id) {
                out.push((r.pri_protection, r.pri_inheritance, r.pri_share_mode));
            }
            let next = r.pri_address.saturating_add(r.pri_size);
            if next <= address {
                break;
            }
            address = next;
        }
        out
    }

    /// A region maps the file shared and writable when its protection allows
    /// writes and `mmap` made it with `MAP_SHARED`, which XNU records as
    /// `VM_INHERIT_SHARE` (`bsd/kern/kern_mman.c`). The share mode says how
    /// many maps hold the object, not how it was mapped: a lone `MAP_SHARED`
    /// mapping reports `SM_PRIVATE`. A process that changes the inheritance
    /// with `minherit` is not seen.
    fn maps_writable(pid: c_int, id: FileId) -> bool {
        regions(pid, id)
            .into_iter()
            .any(|(prot, inherit, _)| prot & VM_PROT_WRITE != 0 && inherit == VM_INHERIT_SHARE)
    }

    pub(super) fn scan(id: FileId) -> Scan {
        let mut out = Scan::default();
        for pid in pids() {
            if pid <= 0 {
                continue;
            }
            let Some(fds) = fds(pid) else {
                out.unreadable += 1;
                out.hidden.push(None);
                continue;
            };
            let writes = fds
                .iter()
                .filter(|f| f.proc_fdtype == PROX_FDTYPE_VNODE)
                .any(|f| fd_writes(pid, f.proc_fd, id))
                || maps_writable(pid, id);
            if writes {
                out.writers.push(pid.unsigned_abs());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;

    #[test]
    fn parses_flags_and_maps() {
        assert!(flags_write("0100001") && flags_write("02") && !flags_write("0100000"));
        assert!(!flags_write("x"));
        let line = "7f00-7f10 rw-s 00000000 fd:01 1234 /home/m/a.jpg";
        assert!(maps_line_writes(line, 0xfd, 1, 1234));
        assert!(!maps_line_writes(line, 0xfd, 1, 1235));
        assert!(!maps_line_writes(
            "7f00-7f10 rw-p 00000000 fd:01 1234 /a",
            0xfd,
            1,
            1234
        ));
        assert!(!maps_line_writes(
            "7f00-7f10 r--s 00000000 fd:01 1234 /a",
            0xfd,
            1,
            1234
        ));
    }

    fn file(mode: u32, uid: u32, gid: u32) -> Stat {
        Stat {
            id: FileId { dev: 1, ino: 2 },
            size: 0,
            mtime_ns: 0,
            ctime_ns: 0,
            nlink: 1,
            mode: 0o100000 | mode,
            uid,
            gid,
        }
    }

    #[test]
    fn reads_the_file_system_credentials_of_a_status() {
        let text =
            "Name:\tcat\nUid:\t1000\t1000\t1000\t1001\nGid:\t100\t100\t100\t101\nGroups:\t4 27 \n";
        let c = status_creds(text).unwrap();
        assert_eq!(c.uid, 1001);
        assert_eq!(c.gids, vec![4, 27, 101]);
        assert_eq!(status_creds("Name:\tcat\n"), None);
        let none = status_creds("Uid:\t7\t7\t7\t7\nGid:\t7\t7\t7\t7\n").unwrap();
        assert_eq!(none.gids, vec![7]);
    }

    #[test]
    fn only_another_user_who_could_write_makes_a_hidden_process_count() {
        let other = |gids: Vec<u32>| Creds { uid: 2000, gids };
        // umask 002 with a private group: 664, group 1000 only Ana's.
        let mine = file(0o664, 1000, 1000);
        assert!(!other(vec![2000]).another_may_write(&mine));
        assert!(
            other(vec![2000, 1000]).another_may_write(&mine),
            "in its group"
        );
        assert!(
            !Creds {
                uid: 0,
                gids: vec![0]
            }
            .another_may_write(&mine),
            "root, as before"
        );
        assert!(
            !Creds {
                uid: 1000,
                gids: vec![1000]
            }
            .another_may_write(&mine),
            "Ana's own"
        );
        assert!(other(vec![2000]).another_may_write(&file(0o666, 1000, 1000)));
        // In the file's group, the group bits decide, not the other bits.
        assert!(!other(vec![1000]).another_may_write(&file(0o646, 1000, 1000)));

        let scan = |hidden| Scan {
            writers: Vec::new(),
            unreadable: 1,
            hidden,
        };
        assert!(scan(vec![Some(other(vec![2000]))]).clear_for(&mine));
        assert!(
            !scan(vec![None]).clear_for(&mine),
            "unknown credentials on a 664 file"
        );
        assert!(scan(vec![None]).clear_for(&file(0o644, 1000, 1000)));
        let written = Scan {
            writers: vec![1],
            unreadable: 0,
            hidden: Vec::new(),
        };
        assert!(!written.clear_for(&file(0o644, 1000, 1000)));
    }

    #[test]
    fn sees_this_process_writing_and_not_reading() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(&p, b"data").unwrap();
        let id = crate::fs::lstat(&p).unwrap().unwrap().id;
        let me = std::process::id();
        let reader = std::fs::File::open(&p).unwrap();
        assert!(!scan(id).writers.contains(&me), "a reader is no writer");
        let writer = OpenOptions::new().append(true).open(&p).unwrap();
        assert!(scan(id).writers.contains(&me), "an open writer is seen");
        drop((reader, writer));
        assert!(!scan(id).writers.contains(&me));
    }

    #[test]
    fn sees_a_shared_writable_mapping_and_not_a_private_one() {
        use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("m");
        std::fs::write(&p, vec![0u8; 4096]).unwrap();
        let id = crate::fs::lstat(&p).unwrap().unwrap().id;
        let me = std::process::id();
        // Maps the file through a descriptor closed at once, so that only
        // the mapping can make this process a writer.
        let map = |flags| {
            let file = OpenOptions::new().read(true).write(true).open(&p).unwrap();
            // SAFETY: a fresh mapping of a 4096-byte file we own, unmapped
            // below; nothing else aliases it.
            unsafe {
                mmap(
                    std::ptr::null_mut(),
                    4096,
                    ProtFlags::READ | ProtFlags::WRITE,
                    flags,
                    &file,
                    0,
                )
            }
            .unwrap()
        };
        let regions = || {
            #[cfg(target_vendor = "apple")]
            let seen = imp::regions(i32::try_from(me).unwrap(), id);
            #[cfg(not(target_vendor = "apple"))]
            let seen: Vec<(u32, u32, u32)> = Vec::new();
            seen
        };
        let private = map(MapFlags::PRIVATE);
        assert!(
            !scan(id).writers.contains(&me),
            "a private mapping writes no file; regions {:?}",
            regions()
        );
        let shared = map(MapFlags::SHARED);
        assert!(
            scan(id).writers.contains(&me),
            "a shared writable mapping is a writer; regions {:?}",
            regions()
        );
        // SAFETY: the mapping made above, of 4096 bytes.
        unsafe { munmap(shared, 4096) }.unwrap();
        // SAFETY: the mapping made above, of 4096 bytes.
        unsafe { munmap(private, 4096) }.unwrap();
        assert!(!scan(id).writers.contains(&me));
    }
}
