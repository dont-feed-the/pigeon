//! Only canonical key/key objects may be compacted. Annex lock files,
//! retention timestamps and temporary siblings must retain their own inode.
#![allow(clippy::unwrap_used, reason = "tests")]

use super::*;
use sha2::Digest;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[tokio::test]
async fn local_adoption_waits_to_publish_without_blocking_existing_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let env = pigeon_git::GitEnv::default();
    let git = crate::git::Git::new(temp.path(), env.clone());
    git.run(&["init", "--bare", "-q"]).await.unwrap();
    let mut group = Group::open(&"a".repeat(40), temp.path(), &env)
        .await
        .unwrap();
    let store = temp.path().join("separate-pool");
    let key = format!("SHA256E-s3--{:064x}.txt", 1);
    for shared in [false, true] {
        group.shared = shared.then(|| crate::config::BridgeStorage {
            store: store.clone(),
            source_socket: temp.path().join("not-listening.sock"),
        });
        group.git.purge_gate.arm();
        for waiting in [true, false] {
            if !waiting {
                assert!(group.git.purge_gate.publish(
                    group.git.purge_gate.revision().unwrap(),
                    pigeon_core::filter::ContentPolicy {
                        waiting: false,
                        released: [key.clone()].into(),
                    },
                ));
            }
            assert!(!content_allowed(&group, &key));
            adopt(&group, &key).await.unwrap();
            assert!(hydrate(&group, &key).await.is_err());
            assert!(provider_hydrate(&group, &key).await.is_err());
            assert!(!store.exists(), "fenced adoption published a pool name");
        }
        group.git.purge_gate.suspend();
    }
}

#[tokio::test]
async fn compaction_preserves_annex_locks_timestamps_and_temporary_files() {
    for seeded in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let bytes = b"retained bytes";
        let hash = crate::util::hex(&sha2::Sha256::digest(bytes));
        let key = format!("SHA256E-s{}--{hash}.txt", bytes.len());
        let directory = temp.path().join("annex/objects");
        let parent = directory.join("ab/cd").join(&key);
        std::fs::create_dir_all(&parent).unwrap();
        let object = parent.join(&key);
        std::fs::write(&object, bytes).unwrap();
        std::fs::set_permissions(&object, std::fs::Permissions::from_mode(0o444)).unwrap();
        let extras: Vec<_> = [
            (".lck", b"".as_slice()),
            (".rtm", b"12345".as_slice()),
            (".partial", bytes.as_slice()),
        ]
        .into_iter()
        .map(|(suffix, body)| {
            let path = parent.join(format!("{key}{suffix}"));
            std::fs::write(&path, body).unwrap();
            let inode = path.metadata().unwrap().ino();
            (path, body, inode)
        })
        .collect();
        let store = temp.path().join("pool");
        let group = "a".repeat(40);
        if seeded {
            let seed = temp.path().join("seed");
            std::fs::write(&seed, bytes).unwrap();
            pool::retain(&store, &group, &key, &seed, false).unwrap();
        }
        compact_objects(
            directory,
            store.clone(),
            group.clone(),
            false,
            Default::default(),
        )
        .await
        .unwrap();
        let target = pool::path(&store, &group, &key).unwrap();
        assert!(pool::same_inode(&object, &target));
        for (path, body, inode) in extras {
            assert_eq!(
                path.metadata().unwrap().ino(),
                inode,
                "{} was relinked",
                path.display()
            );
            assert_eq!(std::fs::read(&path).unwrap(), body);
        }
    }
}

#[tokio::test]
async fn compaction_cannot_republish_released_or_unverified_pool_bytes() {
    for waiting in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let bytes = b"qualified bytes";
        let hash = crate::util::hex(&sha2::Sha256::digest(bytes));
        let key = format!("SHA256E-s{}--{hash}.txt", bytes.len());
        let directory = temp.path().join("annex/objects");
        let parent = directory.join("ab/cd").join(&key);
        std::fs::create_dir_all(&parent).unwrap();
        let object = parent.join(&key);
        std::fs::write(&object, bytes).unwrap();
        let gate = std::sync::Arc::new(crate::purge_gate::Gate::default());
        gate.arm();
        if !waiting {
            assert!(gate.publish(
                gate.revision().unwrap(),
                pigeon_core::filter::ContentPolicy {
                    waiting: false,
                    released: [key.clone()].into()
                }
            ));
        }
        let store = temp.path().join("pool");
        let group = "a".repeat(40);
        compact_objects(directory, store.clone(), group.clone(), false, gate)
            .await
            .unwrap();
        assert!(!pool::path(&store, &group, &key).unwrap().exists());
        assert_eq!(std::fs::read(&object).unwrap(), bytes);
    }
}
