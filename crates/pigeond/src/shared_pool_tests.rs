//! Pool invariants: mutable saves stay isolated, immutable custody objects can
//! share inodes, concurrent writers converge, and corrupt objects fail closed.
#![allow(clippy::unwrap_used, reason = "tests")]

use super::*;

fn source(root: &Path, name: &str, bytes: &[u8], immutable: bool) -> (PathBuf, String) {
    let path = root.join(name);
    std::fs::write(&path, bytes).unwrap();
    if immutable {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    }
    let key = format!(
        "SHA256E-s{}--{}.txt",
        bytes.len(),
        sha256_file(&path).unwrap()
    );
    (path, key)
}

#[test]
fn immutable_custody_bytes_are_pinned_without_a_second_inode() {
    let temp = tempfile::tempdir().unwrap();
    let (source, key) = source(temp.path(), "custody", b"retained", true);
    let target = retain(
        &temp.path().join("pool"),
        &"a".repeat(40),
        &key,
        &source,
        true,
    )
    .unwrap();
    assert!(same_inode(&source, &target));
    std::fs::remove_file(&source).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"retained");
    verify(&target, &key).unwrap();
}

#[test]
fn mutable_laptop_inodes_are_snapshotted_and_identical_content_is_shared() {
    let temp = tempfile::tempdir().unwrap();
    let (first, key) = source(temp.path(), "laptop", b"original", false);
    let store = temp.path().join("pool");
    let group = "a".repeat(40);
    let target = retain(&store, &group, &key, &first, true).unwrap();
    assert!(!same_inode(&first, &target));
    std::fs::write(&first, b"changed in place").unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
    let (other, _) = source(temp.path(), "client", b"original", true);
    let identical = retain(&store, &group, &key.replace(".txt", ".bin"), &other, true).unwrap();
    assert_eq!(identical, target);
    let reference = temp.path().join("reference");
    std::fs::write(&reference, b"original").unwrap();
    relink(&target, &reference).unwrap();
    assert!(same_inode(&target, &reference));
    let different_group = retain(&store, &"b".repeat(40), &key, &other, true).unwrap();
    assert_ne!(different_group, target);
}

#[test]
fn registered_custodian_receives_reuse_existing_client_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let git_dir = temp.path().join("source.git");
    std::fs::create_dir_all(git_dir.join("pigeon")).unwrap();
    let store = temp.path().join("pool");
    register(&git_dir, &store).unwrap();
    register(&git_dir, &store).unwrap();
    assert_eq!(
        registration(&git_dir).unwrap(),
        Some(store.canonicalize().unwrap())
    );
    assert!(register(&git_dir, &temp.path().join("other-pool")).is_err());
    let (client, key) = source(temp.path(), "client", b"new version", true);
    let group = "a".repeat(40);
    let canonical = retain(&store, &group, &key, &client, true).unwrap();
    let (incoming, _) = source(temp.path(), "custodian", b"new version", true);
    assert!(!same_inode(&incoming, &canonical));
    received(&git_dir, &group, &key, &incoming).unwrap();
    assert!(same_inode(&incoming, &canonical));
    assert!(same_inode(&client, &canonical));
    std::fs::write(
        git_dir.join("pigeon/shared-provider.json"),
        br#"{"format":99,"store":"/private"}"#,
    )
    .unwrap();
    assert!(registration(&git_dir).is_err());
}

#[test]
fn concurrent_pool_writers_converge_and_corruption_is_never_blessed() {
    let temp = tempfile::tempdir().unwrap();
    let (source, key) = source(temp.path(), "content", b"one immutable version", true);
    let store = temp.path().join("pool");
    let group = "a".repeat(40);
    let paths = std::thread::scope(|scope| {
        let tasks: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| retain(&store, &group, &key, &source, true).unwrap()))
            .collect();
        tasks
            .into_iter()
            .map(|task| task.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(
        paths
            .iter()
            .all(|p| p == &paths[0] && same_inode(p, &source))
    );
    std::fs::set_permissions(&paths[0], std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::write(&paths[0], b"corruption").unwrap();
    assert!(retain(&store, &group, &key, &source, true).is_err());
    assert_eq!(std::fs::read(&paths[0]).unwrap(), b"corruption");
    assert!(path(&store, "../other", &key).is_err());
    assert!(path(&store, &group, "../../outside").is_err());
}

#[test]
fn erasure_releases_only_the_qualified_group_name_and_allows_a_fresh_birth() {
    let temp = tempfile::tempdir().unwrap();
    let (source, key) = source(temp.path(), "retained", b"same bytes", true);
    let store = temp.path().join("pool");
    let group = "a".repeat(40);
    let other = "b".repeat(40);
    let target = retain(&store, &group, &key, &source, true).unwrap();
    let independent = retain(&store, &other, &key, &source, true).unwrap();
    let reader = temp.path().join("active-reader");
    std::fs::hard_link(&target, &reader).unwrap();
    let locks = target.with_extension("lock");
    let inode = locks.metadata().unwrap().ino();
    release(&store, &group, &key).unwrap();
    release(&store, &group, &key).unwrap();
    assert!(!target.exists());
    assert_eq!(locks.metadata().unwrap().ino(), inode);
    assert_eq!(std::fs::read(&independent).unwrap(), b"same bytes");
    assert_eq!(std::fs::read(&reader).unwrap(), b"same bytes");
    // No sticky tombstone may defeat a later uncovered restore/reference.
    assert_eq!(retain(&store, &group, &key, &source, true).unwrap(), target);
    verify(&target, &key).unwrap();
}
