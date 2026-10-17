//! Actual corrupt-object scrub: the real pinned git-annex moves bad bytes
//! aside, the durable pass reports one failed key, and a healthy object
//! and the laptop's independent source remain exact.

use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn scrub_checks_real_objects_at_idle_priority_and_keeps_a_durable_result() {
    let Some(l) = laptop_fixture(true).await else {
        return;
    };
    let remote = l.dir.path().join("custodian.git");
    let rg = Git::new(&remote, GitEnv::default());
    l.git
        .run(&["remote", "add", &dev(3).0, remote.to_str().unwrap()])
        .await
        .unwrap();
    write(&l, "a.txt", b"first healthy original\n");
    write(&l, "b.txt", b"second healthy original\n");
    let main = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let keys = l.git.keys(&main).await.unwrap();
    for key in keys.values() {
        l.git
            .run(&["annex", "copy", "--to", &dev(3).0, "--key", key])
            .await
            .unwrap();
    }
    rg.run(&[
        "fetch",
        l.wt.to_str().unwrap(),
        "refs/heads/main:refs/heads/main",
    ])
    .await
    .unwrap();
    let backup = Group::open(&l.g.id, &remote, &GitEnv::default())
        .await
        .unwrap();
    pigeond::settings::check(&backup).await;
    let corrupt = pigeond::custody::present(&backup, &keys["a.txt"])
        .await
        .unwrap();
    let healthy = pigeond::custody::present(&backup, &keys["b.txt"])
        .await
        .unwrap();
    // Replace this backup's inode, so local hard-link optimizations cannot
    // corrupt the laptop's independent fixture along with the backup.
    let parent = corrupt.parent().unwrap();
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let replacement = parent.join("corruption-fixture");
    std::fs::write(&replacement, b"wrong but same size xx\n").unwrap();
    std::fs::rename(replacement, &corrupt).unwrap();
    let local = pigeond::custody::present(&l.g, &keys["a.txt"])
        .await
        .unwrap();
    assert_eq!(std::fs::read(local).unwrap(), b"first healthy original\n");
    pigeond::scrub::slice(&backup, pigeond::util::now())
        .await
        .unwrap();
    assert!(!corrupt.exists());
    assert_eq!(
        std::fs::read(&healthy).unwrap(),
        b"second healthy original\n"
    );
    let report = pigeond::report::build(&backup, &dev(3).0).await;
    let scrub = report.scrub.unwrap();
    assert_eq!(scrub.keys_failed, 1);
    assert!(scrub.last_pass_completed.to_string().ends_with('Z'));
    let reopened = Group::open(&l.g.id, &remote, &GitEnv::default())
        .await
        .unwrap();
    assert_eq!(
        pigeond::report::build(&reopened, &dev(3).0).await.scrub,
        Some(scrub)
    );
    // A pass completed just now is not restarted merely by another slice.
    let ledger = std::fs::read(backup.root().join("scrub.toml")).unwrap();
    pigeond::scrub::slice(&backup, pigeond::util::now())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(backup.root().join("scrub.toml")).unwrap(),
        ledger
    );
    // Unknown or torn maintenance state is preserved, never reset to a
    // reassuring new pass. Existing completed evidence remains separate.
    let path = backup.root().join("scrub.toml");
    for unknown in ["format = 2\n", "format = [torn"] {
        std::fs::write(&path, unknown).unwrap();
        assert!(
            pigeond::scrub::slice(&backup, pigeond::util::now())
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), unknown);
    }
}
