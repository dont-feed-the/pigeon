//! One fresh kernel writer inventory for a set of capture candidates (07 §7.5).
//! This observes open writers; install's credential-aware single-file scan
//! remains separate. A snapshot authorizes nothing for an unqueried inode.

use std::collections::BTreeSet;

use crate::fs::FileId;

/// Observed writers of exactly the queried identities, without a persistent cache.
pub struct Snapshot {
    asked: BTreeSet<FileId>,
    busy: BTreeSet<FileId>,
}

impl Snapshot {
    /// Whether a queried inode had no observed writer. Unknown identities
    /// are not clear: replacing a file after collection requires a new scan.
    pub fn is_clear(&self, id: FileId) -> bool {
        self.asked.contains(&id) && !self.busy.contains(&id)
    }
}

/// Observe the same writable descriptors/shared mappings as [`super::scan`].
/// Linux enumerates processes once for the whole set. Other platforms retain
/// their single-file implementation; callers still need stable-file fences.
pub fn snapshot(ids: impl IntoIterator<Item = FileId>) -> Snapshot {
    let asked: BTreeSet<_> = ids.into_iter().collect();
    #[cfg(target_os = "linux")]
    let busy = linux(std::path::Path::new("/proc"), &asked);
    #[cfg(not(target_os = "linux"))]
    let busy = asked
        .iter()
        .copied()
        .filter(|id| !super::scan(*id).is_clear())
        .collect();
    Snapshot { asked, busy }
}

#[cfg(target_os = "linux")]
fn linux(root: &std::path::Path, asked: &BTreeSet<FileId>) -> BTreeSet<FileId> {
    use std::os::unix::fs::MetadataExt;
    let mut busy = BTreeSet::new();
    if asked.is_empty() {
        return busy;
    }
    let Ok(processes) = std::fs::read_dir(root) else {
        return asked.clone();
    };
    for process in processes.flatten() {
        if process
            .file_name()
            .to_str()
            .and_then(|p| p.parse::<u32>().ok())
            .is_none()
        {
            continue;
        }
        let base = process.path();
        // Like scan(id).is_clear(), capture does not turn unreadable
        // credentials into a writer. Install still uses Scan::clear_for.
        let Ok(fds) = std::fs::read_dir(base.join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(meta) = std::fs::metadata(fd.path()) else {
                continue;
            };
            let id = FileId {
                dev: meta.dev(),
                ino: meta.ino(),
            };
            if !asked.contains(&id) {
                continue;
            }
            let flags = std::fs::read_to_string(base.join("fdinfo").join(fd.file_name()))
                .ok()
                .and_then(|text| {
                    text.lines()
                        .find_map(|line| line.strip_prefix("flags:").map(super::flags_write))
                });
            if flags != Some(false) {
                busy.insert(id);
            }
        }
        if let Ok(maps) = std::fs::read_to_string(base.join("maps")) {
            for id in maps.lines().filter_map(mapping) {
                if asked.contains(&id) {
                    busy.insert(id);
                }
            }
        }
    }
    busy
}

#[cfg(target_os = "linux")]
fn mapping(line: &str) -> Option<FileId> {
    let mut fields = line.split_whitespace();
    let (_, perms, _, device, inode) = (
        fields.next()?,
        fields.next()?,
        fields.next()?,
        fields.next()?,
        fields.next()?,
    );
    let permissions = perms.as_bytes();
    if permissions.get(1) != Some(&b'w') || permissions.get(3) != Some(&b's') {
        return None;
    }
    let (major, minor) = device.split_once(':')?;
    Some(FileId {
        dev: rustix::fs::makedev(
            u32::from_str_radix(major, 16).ok()?,
            u32::from_str_radix(minor, 16).ok()?,
        ),
        ino: inode.parse().ok()?,
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #![allow(clippy::unwrap_used, reason = "private process-inventory fixtures")]
    use super::*;

    #[test]
    fn an_unreadable_process_root_never_marks_candidates_clear() {
        let root = tempfile::tempdir().unwrap();
        let asked = BTreeSet::from([FileId { dev: 1, ino: 2 }]);
        assert_eq!(linux(&root.path().join("missing"), &asked), asked);
    }

    #[test]
    fn mapping_identities_preserve_the_single_file_decoder_rules() {
        for major in [0, 1, 8, 259, 4095] {
            for minor in [0, 1, 255, 256, 1_048_575] {
                for inode in (0..10).chain([u64::MAX]) {
                    for perms in ["rw-s", "rw-p", "r--s", "---p"] {
                        let line =
                            format!("0000-1000 {perms} 0000 {major:x}:{minor:x} {inode} /fixture");
                        let expected = super::super::maps_line_writes(&line, major, minor, inode);
                        assert_eq!(mapping(&line).is_some(), expected);
                        if let Some(id) = mapping(&line) {
                            assert_eq!(
                                id,
                                FileId {
                                    dev: rustix::fs::makedev(major, minor),
                                    ino: inode
                                }
                            );
                        }
                    }
                }
            }
        }
        for malformed in ["", "0 rw-s 0 00:00 not-an-inode", "0 rw-s 0 no-device 1"] {
            assert_eq!(mapping(malformed), None);
        }
    }
}
