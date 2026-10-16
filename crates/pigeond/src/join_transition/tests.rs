//! Replay refuses unknown intents and preserves later work. These use
//! ordinary local Git roots; welcome authority is checked before recording
//! the private journal, separately from these local replay mechanics.
#![allow(clippy::unwrap_used, reason = "tests")]

use super::*;
use crate::git::Git;
use pigeon_git::GitEnv;

struct Fixture {
    _directory: tempfile::TempDir,
    group: Group,
    plan: Plan,
    config: PathBuf,
}

async fn setup() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let git = Git::new(&repo, GitEnv::default());
    git.run(&["init", "--bare", "-q", "-b", "main"])
        .await
        .unwrap();
    git.run(&["config", "user.name", "Fixture"]).await.unwrap();
    git.run(&["config", "user.email", "fixture@example.invalid"])
        .await
        .unwrap();
    let tree = git.run_in(&["mktree"], b"").await.unwrap();
    let old = git
        .run(&["commit-tree", tree.trim(), "-m", "old"])
        .await
        .unwrap()
        .trim()
        .to_owned();
    let new = git
        .run(&["commit-tree", tree.trim(), "-m", "new"])
        .await
        .unwrap()
        .trim()
        .to_owned();
    git.update_ref("refs/heads/main", &old, None).await.unwrap();
    git.run(&["read-tree", &old]).await.unwrap();
    let group = Group::open(&old, &repo, &GitEnv::default()).await.unwrap();
    let config = directory.path().join("config.toml");
    config::add_group(
        &config,
        &GroupEntry {
            id: old.clone(),
            path: repo,
        },
    )
    .unwrap();
    let plan = Plan {
        format: 1,
        old_group: old.clone(),
        old_main: old.clone(),
        archive_main: old,
        group: new.clone(),
        main: new,
        stage: None,
        source: None,
    };
    Fixture {
        _directory: directory,
        group,
        plan,
        config,
    }
}

#[tokio::test]
async fn malformed_future_and_unrelated_intents_do_not_move_main_or_config() {
    let mut f = setup().await;
    let configuration = std::fs::read(&f.config).unwrap();
    for bytes in [b"not json".to_vec(), b"{\"format\":2}".to_vec()] {
        crate::util::write_private(&file(&f.group), &bytes).unwrap();
        assert!(finish(&f.group, &f.config).await.is_err());
    }
    f.plan.format = 2;
    record(&f.group, &f.plan).unwrap();
    assert!(finish(&f.group, &f.config).await.is_err());
    f.plan.format = 1;
    f.plan.old_group = "another-group".into();
    record(&f.group, &f.plan).unwrap();
    assert!(finish(&f.group, &f.config).await.is_err());
    assert_eq!(
        f.group.git.rev("main").await.as_deref(),
        Some(f.group.id.as_str())
    );
    assert_eq!(std::fs::read(&f.config).unwrap(), configuration);
    assert!(pending(&f.group));
}

#[tokio::test]
async fn local_replay_archives_old_proposals_and_is_idempotent() {
    let f = setup().await;
    let proposal = format!("{}fixture", crate::proposal::PREFIX);
    f.group
        .git
        .update_ref(&proposal, &f.plan.old_main, None)
        .await
        .unwrap();
    record(&f.group, &f.plan).unwrap();
    assert!(finish(&f.group, &f.config).await.unwrap());
    assert_eq!(
        f.group.git.rev("main").await.as_deref(),
        Some(f.plan.main.as_str())
    );
    assert!(f.group.git.rev(&proposal).await.is_none());
    let kept = format!(
        "{}{}/proposals/fixture/{}",
        crate::refound::ARCHIVE,
        f.plan.old_group,
        f.plan.old_main
    );
    assert_eq!(
        f.group.git.rev(&kept).await.as_deref(),
        Some(f.plan.old_main.as_str())
    );
    assert_eq!(config::load(&f.config).unwrap().groups[0].id, f.plan.group);
    assert!(!finish(&f.group, &f.config).await.unwrap());
}

#[tokio::test]
async fn a_leftover_completed_intent_does_not_reset_a_later_index() {
    let f = setup().await;
    record(&f.group, &f.plan).unwrap();
    finish(&f.group, &f.config).await.unwrap();
    let tree = f.group.git.run_in(&["mktree"], b"").await.unwrap();
    let later = f
        .group
        .git
        .run(&[
            "commit-tree",
            tree.trim(),
            "-p",
            &f.plan.main,
            "-m",
            "later",
        ])
        .await
        .unwrap();
    f.group
        .git
        .update_ref("refs/heads/main", later.trim(), Some(&f.plan.main))
        .await
        .unwrap();
    let blob = f
        .group
        .git
        .run_in(&["hash-object", "-w", "--stdin"], b"later local index\n")
        .await
        .unwrap();
    f.group
        .git
        .run(&[
            "update-index",
            "--add",
            "--cacheinfo",
            "100644",
            blob.trim(),
            "later.txt",
        ])
        .await
        .unwrap();
    let index = std::fs::read(f.group.git_dir.join("index")).unwrap();
    record(&f.group, &f.plan).unwrap();
    assert!(finish(&f.group, &f.config).await.unwrap());
    assert_eq!(f.group.git.rev("main").await.as_deref(), Some(later.trim()));
    assert_eq!(std::fs::read(f.group.git_dir.join("index")).unwrap(), index);
    assert!(!pending(&f.group));
}
