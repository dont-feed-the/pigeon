//! A real native location is insufficient when bytes contradict the key's size.
#![allow(clippy::unwrap_used, reason = "private native copy-identity fixture")]

use super::{AnnexSyncer, Repository, Syncer};
use sha2::{Digest, Sha256};

#[tokio::test]
async fn native_copy_barrier_refuses_correct_digest_with_inconsistent_key_size() {
    let root = tempfile::tempdir().unwrap();
    let git = crate::git::Git::new(root.path(), pigeon_git::GitEnv::default());
    if git.run(&["annex", "version", "--raw"]).await.is_err() {
        assert!(std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none());
        return;
    }
    git.run(&["init", "-q"]).await.unwrap();
    git.run(&["config", "user.name", "copy identity fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "fixture@example.invalid"])
        .await
        .unwrap();
    git.run(&["annex", "init", "copy identity"]).await.unwrap();
    let bytes = b"independent copy identity bytes\n";
    let digest = crate::util::hex(&Sha256::digest(bytes));
    let syncer = AnnexSyncer::new(Default::default(), git.repo.env.clone());
    let mut repository = Repository {
        group: "copy-identity".into(),
        git_dir: root.path().join(".git"),
        worktree: Some(root.path().into()),
        role: pigeon_core::filter::Role::Custodian,
        auth_token: None,
        checking: false,
    };
    // This ordinary custodian has no shared-provider registration. That
    // optional pool's own verifier cannot supply the network barrier's check.
    assert!(
        crate::shared_pool::registration(&repository.git_dir)
            .unwrap()
            .is_none()
    );
    for size in [bytes.len() + 1, bytes.len()] {
        let key = format!("SHA256E-s{size}--{digest}");
        let relative = git
            .run(&[
                "annex",
                "examinekey",
                &key,
                "--format=${hashdirmixed}${key}/${key}",
            ])
            .await
            .unwrap();
        let object = repository
            .git_dir
            .join("annex/objects")
            .join(relative.trim());
        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, bytes).unwrap();
        // Native git-annex reports this actual object even for the wrong
        // declared size. Independently verify the bytes used by the fixture.
        let location = git.run(&["annex", "contentlocation", &key]).await.unwrap();
        assert_eq!(root.path().join(location.trim()), object);
        assert_eq!(std::fs::read(&object).unwrap(), bytes);
        for role in [
            pigeon_core::filter::Role::Custodian,
            pigeon_core::filter::Role::Laptop,
        ] {
            repository.role = role;
            assert_eq!(
                syncer.sync(&repository, &key, true).await.unwrap(),
                size == bytes.len(),
                "a hash-checked copy must match the complete content identity"
            );
        }
    }
}
