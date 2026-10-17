//! Read-only observations under a content fence need real local byte proof,
//! not transfer authorization or merely a retained pointer. Corruption and
//! an absent annex object must never become a stored-content observation.

use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn a_closed_content_gate_still_observes_only_hash_verified_local_objects() {
    let Some(l) = laptop().await else { return };
    let bytes = b"capture remains durable while transfer metadata waits\n";
    write(&l, "observed.txt", bytes);
    let main = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let key = l.git.keys(&main).await.unwrap()["observed.txt"].clone();
    let digest = pigeond::net::sync::key_sha256(&key).unwrap();
    let object = pigeond::custody::present(&l.g, &key).await.unwrap();
    let index = l.git.index().await.unwrap();
    l.g.git.purge_gate.suspend();
    assert!(pigeond::retention::keys(&l.g).await.is_err());
    let observation = pigeond::trace_state::snapshot(&l.g, "observed.txt", 0)
        .await
        .unwrap();
    assert!(
        observation["signed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some(digest))
    );
    assert!(l.g.git.purge_gate.policy().waiting);
    assert_eq!(l.git.index().await.unwrap(), index);
    assert_eq!(
        l.git.rev("refs/heads/main").await.as_deref(),
        Some(main.as_str())
    );
    std::fs::set_permissions(&object, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(&object, vec![b'x'; bytes.len()]).unwrap();
    // git-annex may itself refuse the damaged object. Either its refusal
    // or the observer's hash error is safe; neither can count these bytes.
    if let Ok(observation) = pigeond::trace_state::snapshot(&l.g, "observed.txt", 0).await {
        assert!(
            !observation["signed"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some(digest))
        );
    }
    std::fs::set_permissions(
        object.parent().unwrap(),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    std::fs::remove_file(&object).unwrap();
    let observation = pigeond::trace_state::snapshot(&l.g, "observed.txt", 0)
        .await
        .unwrap();
    assert!(
        !observation["signed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some(digest))
    );
    assert_eq!(std::fs::read(l.wt.join("observed.txt")).unwrap(), bytes);
    assert!(l.g.git.purge_gate.policy().waiting);
}
