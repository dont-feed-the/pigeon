//! Set observations preserve native open-writer/mapping checks while sharing
//! process enumeration. The ignored comparison measures local primitive cost.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, reason = "private native writer fixtures")]

use pigeon_sys::{fs::FileId, writers};
use std::fs::OpenOptions;

fn id(path: &std::path::Path) -> FileId {
    pigeon_sys::fs::lstat(path).unwrap().unwrap().id
}

#[test]
fn mixed_readers_writers_and_hardlinks_match_the_single_file_scan() {
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..8)
        .map(|n| root.path().join(format!("{n}.txt")))
        .collect();
    for path in &paths {
        std::fs::write(path, b"native writer fixture").unwrap();
    }
    let _reader = std::fs::File::open(&paths[0]).unwrap();
    let writer = OpenOptions::new().append(true).open(&paths[1]).unwrap();
    let alias = root.path().join("alias");
    std::fs::hard_link(&paths[1], &alias).unwrap();
    let ids: Vec<_> = paths.iter().map(|p| id(p)).collect();
    let observed = writers::snapshot(ids.iter().copied());
    for file in &ids {
        assert_eq!(observed.is_clear(*file), writers::scan(*file).is_clear());
    }
    assert!(!observed.is_clear(id(&alias)));
    assert!(observed.is_clear(ids[0]));
    assert!(!observed.is_clear(FileId {
        dev: u64::MAX,
        ino: u64::MAX
    }));
    drop(writer);
    assert!(writers::snapshot(ids).is_clear(id(&paths[1])));
}

#[test]
fn a_shared_mapping_is_busy_after_its_descriptor_closes() {
    use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("mapping");
    std::fs::write(&path, [0u8; 4096]).unwrap();
    let identity = id(&path);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    // SAFETY: a private mapping of our 4096-byte fixture, unmapped below.
    let private = unsafe {
        mmap(
            std::ptr::null_mut(),
            4096,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::PRIVATE,
            &file,
            0,
        )
    }
    .unwrap();
    // SAFETY: a new 4096-byte mapping of our private file, unmapped below.
    let region = unsafe {
        mmap(
            std::ptr::null_mut(),
            4096,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::SHARED,
            &file,
            0,
        )
    }
    .unwrap();
    drop(file);
    assert!(!writers::snapshot([identity]).is_clear(identity));
    // SAFETY: this exact live mapping was made above and is not aliased.
    unsafe { munmap(region, 4096) }.unwrap();
    assert!(writers::snapshot([identity]).is_clear(identity));
    // SAFETY: this exact private mapping was made above and remains live.
    unsafe { munmap(private, 4096) }.unwrap();
}

#[test]
fn replacement_inodes_require_another_observation() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    let replacement = root.path().join("replacement");
    std::fs::write(&path, b"old").unwrap();
    std::fs::write(&replacement, b"new").unwrap();
    let observed = writers::snapshot([id(&path)]);
    std::fs::rename(&replacement, &path).unwrap();
    assert!(!observed.is_clear(id(&path)));
    assert!(writers::snapshot([id(&path)]).is_clear(id(&path)));
}

#[test]
#[ignore = "manual local process-inventory comparison, not a resource budget"]
#[allow(
    clippy::print_stdout,
    reason = "manual comparison reports measured durations"
)]
fn compare_256_candidates_on_the_same_kernel_inventory() {
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..256)
        .map(|n| root.path().join(format!("{n}.txt")))
        .collect();
    for path in &paths {
        std::fs::write(path, b"native writer fixture").unwrap();
    }
    let _writer = OpenOptions::new().append(true).open(&paths[7]).unwrap();
    let ids: Vec<_> = paths.iter().map(|p| id(p)).collect();
    let serial = || {
        let start = std::time::Instant::now();
        let values: Vec<_> = ids.iter().map(|id| writers::scan(*id).is_clear()).collect();
        (values, start.elapsed().as_micros())
    };
    let batch = || {
        let start = std::time::Instant::now();
        let set = writers::snapshot(ids.iter().copied());
        let values: Vec<_> = ids.iter().map(|id| set.is_clear(*id)).collect();
        (values, start.elapsed().as_micros())
    };
    for trial in 0..3 {
        let ((serial, serial_us), (together, batch_us)) = if trial % 2 == 0 {
            (serial(), batch())
        } else {
            let together = batch();
            (serial(), together)
        };
        assert_eq!(serial, together);
        println!("trial={trial} candidates=256 serial_us={serial_us} batch_us={batch_us}");
    }
}
