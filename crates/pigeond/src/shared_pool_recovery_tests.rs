//! Interrupted pool publication must complete directory sync on retry. Removing
//! private aliases never removes an append-only pool object or another reader.
#![allow(clippy::unwrap_used, reason = "tests")]

use super::*;

fn fixture(root: &Path) -> (PathBuf, String) {
    let source = root.join("source");
    let bytes = b"retained immutable bytes";
    std::fs::write(&source, bytes).unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o444)).unwrap();
    let key = format!(
        "SHA256E-s{}--{}.txt",
        bytes.len(),
        sha256_file(&source).unwrap()
    );
    (source, key)
}

#[test]
fn retry_finishes_directory_sync_after_a_published_attempt_failed() {
    let temp = tempfile::tempdir().unwrap();
    let (source, key) = fixture(temp.path());
    let store = temp.path().join("pool");
    let group = "a".repeat(40);
    let target = path(&store, &group, &key).unwrap();
    let fail_sync = |_: &Path| Err(io::Error::other("simulated directory sync failure"));
    assert!(retain_with_sync(&store, &group, &key, &source, true, fail_sync).is_err());
    assert!(
        target.exists(),
        "The failed attempt already renamed its object"
    );
    verify(&target, &key).unwrap();
    // A retry seeing the target must still propagate the same sync failure.
    assert!(retain_with_sync(&store, &group, &key, &source, true, fail_sync).is_err());
    let calls = std::cell::Cell::new(0);
    let result = retain_with_sync(&store, &group, &key, &source, true, |directory| {
        assert_eq!(directory, target.parent().unwrap());
        calls.set(calls.get() + 1);
        pigeon_sys::fs::fsync_dir(directory).map_err(io::Error::other)
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(same_inode(&result, &source));
}

#[test]
fn private_drops_and_abandoned_staging_preserve_pool_pins() {
    let temp = tempfile::tempdir().unwrap();
    let (source, key) = fixture(temp.path());
    let store = temp.path().join("pool");
    let group = "a".repeat(40);
    let target = path(&store, &group, &key).unwrap();
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    let orphan = target.with_extension("interrupted");
    std::fs::write(&orphan, b"incomplete unverified stage").unwrap();
    let retained = retain(&store, &group, &key, &source, true).unwrap();
    let desktop = temp.path().join("desktop");
    let phone = temp.path().join("phone");
    relink(&retained, &desktop).unwrap();
    relink(&retained, &phone).unwrap();
    let mut reader = std::fs::File::open(&desktop).unwrap();
    std::fs::remove_file(&source).unwrap();
    std::fs::remove_file(&desktop).unwrap();
    std::fs::remove_file(&phone).unwrap();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut bytes).unwrap();
    assert_eq!(bytes, b"retained immutable bytes");
    verify(&retained, &key).unwrap();
    assert_eq!(
        std::fs::read(&orphan).unwrap(),
        b"incomplete unverified stage"
    );
    // Reusing the verified pool does not require the removed source or clients.
    assert_eq!(
        retain(&store, &group, &key, &source, true).unwrap(),
        retained
    );
}
