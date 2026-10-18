//! A physical absence observation must not mistake a journalled move-aside
//! for completed freeing. This is an observer, never recovery authority.

use std::path::Path;

fn missing(file: &Path) -> bool {
    std::fs::symlink_metadata(file).is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
}

fn reading(journal: &Path) -> Option<Vec<u8>> {
    match std::fs::read(journal) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => None,
    }
}

// Read only the format and entry boundaries needed by the observer. It
// performs no replay and treats unknown/torn metadata as not yet settled.
fn idle(bytes: &[u8], relative: &str) -> Option<bool> {
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return None;
    }
    let mut open = std::collections::BTreeMap::<u64, String>::new();
    let mut begun = std::collections::BTreeSet::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let record: serde_json::Value = serde_json::from_slice(line).ok()?;
        if record.get("format")?.as_u64()? != 1 {
            return None;
        }
        match record.get("op")?.as_str()? {
            "begin" => {
                let sequence = record.get("seq")?.as_u64()?;
                let path = record.get("path")?.as_str()?.to_owned();
                if !begun.insert(sequence) {
                    return None;
                }
                open.insert(sequence, path);
            }
            "done" | "skip" | "raced" | "recovered" => {
                open.remove(&record.get("seq")?.as_u64()?)?;
            }
            "proved" | "released" | "kept" | "preserved" | "index-intent" | "operation" => {}
            _ => return None,
        }
    }
    Some(!open.values().any(|path| path == relative))
}

pub(super) fn settled(file: &Path, journal: &Path, relative: &str) -> bool {
    settled_with(file, journal, relative, || {})
}

/// A valid open transaction can explain a bounded missing-byte observation.
/// Unknown, malformed or other-path records never extend that observation.
pub(super) fn in_progress(journal: &Path, relative: &str) -> bool {
    reading(journal).is_some_and(|bytes| idle(&bytes, relative) == Some(false))
}

fn settled_with(file: &Path, journal: &Path, relative: &str, after_read: impl FnOnce()) -> bool {
    let Some(before) = reading(journal) else {
        return false;
    };
    after_read();
    if !missing(file) {
        return false;
    }
    let Some(after) = reading(journal) else {
        return false;
    };
    before == after && idle(&after, relative) == Some(true) && missing(file)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "isolated native observation fixtures")]
mod tests {
    use super::*;

    #[test]
    fn a_transition_starting_after_the_first_read_is_not_absence() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        let journal = dir.path().join("journal");
        std::fs::write(&file, b"protected bytes").unwrap();
        assert!(!settled_with(&file, &journal, "docs/file", || {
            std::fs::write(
                &journal,
                b"{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"docs/file\"}\n",
            )
            .unwrap();
            std::fs::rename(&file, dir.path().join("aside")).unwrap();
        }));
    }

    #[test]
    fn a_thinning_move_aside_is_not_completed_freeing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        let aside = dir.path().join("aside");
        let journal = dir.path().join("journal");
        std::fs::write(&file, b"unchanged protected bytes").unwrap();
        std::fs::write(
            &journal,
            b"{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"docs/file\"}\n",
        )
        .unwrap();
        std::fs::rename(&file, &aside).unwrap();
        assert!(
            !settled(&file, &journal, "docs/file"),
            "an open move-aside passed as freed"
        );
        std::fs::rename(&aside, &file).unwrap();
        std::fs::write(&journal, b"{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"docs/file\"}\n{\"format\":1,\"seq\":1,\"op\":\"done\"}\n").unwrap();
        assert!(!settled(&file, &journal, "docs/file"));
        std::fs::remove_file(&file).unwrap();
        assert!(settled(&file, &journal, "docs/file"));
    }

    #[test]
    fn malformed_or_torn_journals_cannot_establish_stable_absence() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        let journal = dir.path().join("journal");
        for bytes in [
            "{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"docs/file\"}",
            "{\"format\":2,\"seq\":1,\"op\":\"done\"}\n",
            "not json\n",
            "{\"format\":1,\"seq\":1,\"op\":\"future-operation\"}\n",
            "{\"format\":1,\"seq\":1,\"op\":\"done\"}\n",
            "{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"docs/file\"}\n{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"other/file\"}\n",
        ] {
            std::fs::write(&journal, bytes).unwrap();
            assert!(!settled(&file, &journal, "docs/file"), "accepted {bytes:?}");
        }
    }

    #[test]
    fn another_paths_transition_does_not_block_a_genuinely_absent_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        let journal = dir.path().join("journal");
        std::fs::write(
            &journal,
            b"{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"other/file\"}\n",
        )
        .unwrap();
        assert!(settled(&file, &journal, "docs/file"));
        std::os::unix::fs::symlink("missing-target", &file).unwrap();
        assert!(
            !settled(&file, &journal, "docs/file"),
            "a dangling pointer was counted absent"
        );
    }
}
