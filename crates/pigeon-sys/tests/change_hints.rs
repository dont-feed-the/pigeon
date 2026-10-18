//! Native event smoke: a real saved file wakes its owner and advances the
//! constant-size hint revision. No event is used as catalogue authority.
#![allow(clippy::unwrap_used, reason = "isolated native watcher fixture")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[test]
fn a_real_file_save_wakes_the_owner() {
    let dir = tempfile::tempdir().unwrap();
    let wakes = Arc::new(AtomicUsize::new(0));
    let observed = wakes.clone();
    let watcher = pigeon_sys::change_hints::Watch::new(dir.path(), move || {
        observed.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();
    assert!(watcher.reading().healthy);
    std::fs::write(dir.path().join("save.txt"), b"exact saved bytes\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while watcher.reading().revision == 0 || wakes.load(Ordering::Relaxed) == 0 {
        assert!(
            Instant::now() < deadline,
            "native save did not wake its owner"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn removing_the_watched_root_requests_scan_only_fallback() {
    let container = tempfile::tempdir().unwrap();
    let root = container.path().join("watched");
    std::fs::create_dir(&root).unwrap();
    let watcher = pigeon_sys::change_hints::Watch::new(&root, || {}).unwrap();
    std::fs::remove_dir(&root).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let reading = watcher.reading();
        if !reading.healthy && reading.revision > 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "root loss did not request scan-only fallback"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_missing_root_cannot_claim_native_watching() {
    let container = tempfile::tempdir().unwrap();
    assert!(
        pigeon_sys::change_hints::Watch::new(&container.path().join("missing"), || {}).is_err()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn kernel_watches_exclude_annex_metadata_and_directory_symlinks() {
    use std::os::unix::fs::MetadataExt;
    let container = tempfile::tempdir().unwrap();
    let root = container.path().join("watched");
    std::fs::create_dir_all(root.join("user/leaf")).unwrap();
    std::fs::create_dir_all(root.join(".git/annex/objects/a/b/key")).unwrap();
    std::fs::create_dir_all(root.join(".pigeon/cache")).unwrap();
    let outside = container.path().join("outside");
    std::fs::create_dir_all(outside.join("unwatched")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    let watcher = pigeon_sys::change_hints::Watch::new(&root, || {}).unwrap();
    let needle = format!("ino:{:x}", std::fs::metadata(&root).unwrap().ino());
    let info = std::fs::read_dir("/proc/self/fdinfo")
        .unwrap()
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .find(|text| {
            text.lines().any(|line| {
                line.starts_with("inotify ") && line.split_whitespace().any(|word| word == needle)
            })
        })
        .unwrap();
    assert_eq!(
        info.lines()
            .filter(|line| line.starts_with("inotify "))
            .count(),
        3,
        "only root, user and leaf need kernel watches"
    );
    assert!(watcher.reading().healthy);
}

#[cfg(target_os = "linux")]
#[test]
fn a_new_directory_requests_registration_refresh() {
    let root = tempfile::tempdir().unwrap();
    let watcher = pigeon_sys::change_hints::Watch::new(root.path(), || {}).unwrap();
    std::fs::create_dir(root.path().join("new-folder")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while !watcher.reading().rebuild {
        assert!(
            Instant::now() < deadline,
            "new directory did not request registration refresh"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
