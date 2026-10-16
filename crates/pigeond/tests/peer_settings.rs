//! A managed roster peer recovers from git-annex's sticky ignore state.
//! Only that peer's configuration is repaired; unrelated remotes stay alone.
#![allow(clippy::unwrap_used, reason = "integration fixtures")]

use pigeon_git::GitEnv;
use pigeond::{git::Git, settings::ensure_remote};

#[tokio::test]
async fn an_admitted_peer_is_reenabled_after_an_initial_probe_ignored_it() {
    let root = tempfile::tempdir().unwrap();
    let git = Git::new(root.path(), GitEnv::default());
    git.run(&["init", "-q", "--bare"]).await.unwrap();
    let device = "a".repeat(64);
    let uuid = "00000000-0000-0000-0000-000000000001";
    ensure_remote(&git, &device, uuid).await.unwrap();
    git.run(&["config", "remote.unrelated.annex-ignore", "true"])
        .await
        .unwrap();
    // Both the retained CI failure (ignore alone) and the automatic
    // initialization state must recover on subsequent configuration rounds.
    for automatic in [false, true] {
        let ignored = format!("remote.{device}.annex-ignore");
        let auto = format!("remote.{device}.annex-ignore-auto");
        git.run(&["config", &ignored, "true"]).await.unwrap();
        if automatic {
            git.run(&["config", &auto, "true"]).await.unwrap();
        }
        ensure_remote(&git, &device, uuid).await.unwrap();
        assert_eq!(git.config(&ignored).await.as_deref(), Some("false"));
        assert_eq!(git.config(&auto).await.as_deref(), Some("false"));
        assert_eq!(
            git.config("remote.unrelated.annex-ignore").await.as_deref(),
            Some("true")
        );
        assert_eq!(
            git.config(&format!("remote.{device}.annex-uuid"))
                .await
                .as_deref(),
            Some(uuid)
        );
    }
}
