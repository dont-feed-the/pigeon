//! Recovery reachability leaves cached peer refs to background fetching.
//! A local ref lock does not mean the backup is offline (09 §9.9).
#![allow(clippy::unwrap_used, reason = "isolated real-Git probe regressions")]

use crate::git::Git;

async fn fixture() -> (tempfile::TempDir, Git, String, String) {
    let temp = tempfile::tempdir().unwrap();
    let mut repos = Vec::new();
    let mut commits = Vec::new();
    for name in ["local", "peer"] {
        let path = temp.path().join(name);
        std::fs::create_dir(&path).unwrap();
        let git = Git::new(path, pigeon_git::GitEnv::default());
        git.run(&["init", "--bare", "-q", "-b", "main"])
            .await
            .unwrap();
        git.run(&["config", "user.name", "Probe fixture"])
            .await
            .unwrap();
        git.run(&["config", "user.email", "probe@example.invalid"])
            .await
            .unwrap();
        let tree = git.run_in(&["mktree"], b"").await.unwrap();
        let commit = git
            .run_in(&["commit-tree", tree.trim()], name.as_bytes())
            .await
            .unwrap()
            .trim()
            .to_owned();
        git.update_ref("refs/heads/main", &commit, None)
            .await
            .unwrap();
        repos.push(git);
        commits.push(commit);
    }
    let local = repos.remove(0);
    let peer = repos.remove(0);
    local
        .run(&["remote", "add", "peer", peer.repo.dir.to_str().unwrap()])
        .await
        .unwrap();
    local
        .run(&[
            "config",
            "remote.peer.fetch",
            &crate::settings::peer_refspec("peer"),
        ])
        .await
        .unwrap();
    let cached = "refs/pigeon/peers/peer/main".to_owned();
    local.update_ref(&cached, &commits[0], None).await.unwrap();
    (temp, local, cached, commits.remove(0))
}

#[tokio::test]
async fn online_peer_is_reachable_while_its_cached_ref_is_locked() {
    let (_temp, git, cached, old) = fixture().await;
    let lock = git.repo.dir.join(format!("{cached}.lock"));
    std::fs::write(&lock, b"background fetch owns this lock").unwrap();
    let result = super::probe(&git, "peer").await;
    assert!(result.is_ok(), "online peer was refused: {result:?}");
    assert_eq!(
        std::fs::read(lock).unwrap(),
        b"background fetch owns this lock"
    );
    assert_eq!(git.rev(&cached).await.as_deref(), Some(old.as_str()));
    assert!(!git.repo.dir.join("FETCH_HEAD").exists());
}

#[tokio::test]
async fn online_probe_does_not_move_a_cached_ref_even_without_a_lock() {
    let (_temp, git, cached, old) = fixture().await;
    super::probe(&git, "peer").await.unwrap();
    assert_eq!(git.rev(&cached).await.as_deref(), Some(old.as_str()));
    assert!(!git.repo.dir.join("FETCH_HEAD").exists());
}

#[tokio::test]
async fn absent_peer_is_not_reported_reachable() {
    let (temp, git, _, _) = fixture().await;
    std::fs::remove_dir_all(temp.path().join("peer")).unwrap();
    assert!(super::probe(&git, "peer").await.is_err());
}
