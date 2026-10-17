//! A resource-paused real restore must release the worktree lock, preserve
//! later captured saves and recheck a replaced group before installation.
//! The fixture blocks native get at production resource admission, then
//! resumes it. It runs no daemon rounds or mid-transfer SIGSTOP trial.
#![allow(clippy::unwrap_used, reason = "native restore interleaving fixture")]

use super::*;
use pigeon_git::work::{Class, Control};
use std::sync::{
    Arc, OnceLock, RwLock,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Default)]
struct Paused {
    allow: AtomicBool,
    seen: AtomicBool,
    waiting: tokio::sync::Notify,
}
impl Control for Paused {
    fn running(&self, class: Class) -> bool {
        if class != Class::Transfer || self.allow.load(Ordering::SeqCst) {
            return true;
        }
        if !self.seen.swap(true, Ordering::SeqCst) {
            self.waiting.notify_one();
        }
        false
    }
}

async fn fixture() -> Option<(tempfile::TempDir, Arc<Daemon>, Arc<Group>, String)> {
    let temp = tempfile::tempdir().unwrap();
    let wt = temp.path().join("files");
    std::fs::create_dir_all(wt.join("Docs")).unwrap();
    let env = pigeon_git::GitEnv::default();
    let git = crate::git::Git::new(&wt, env.clone());
    if git.run(&["annex", "version", "--raw"]).await.is_err() {
        assert!(std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none());
        return None;
    }
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "Restore fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "restore@pigeon"])
        .await
        .unwrap();
    git.run(&["annex", "init", "restore"]).await.unwrap();
    let node = crate::net::Node::bind(&crate::net::NetConfig {
        secret_key: [43; 32],
        bind: "127.0.0.1:0".parse().unwrap(),
        mdns: false,
        reach: crate::net::Reach::Local,
    })
    .await
    .unwrap();
    let dirs = pigeon_sys::paths::Dirs {
        config: temp.path().join("config"),
        state: temp.path().join("state"),
        cache: temp.path().join("cache"),
        runtime: temp.path().join("run"),
    };
    let device = node.device_id();
    let tracer = crate::trace::Tracer::new(dirs.traces(), &device);
    let mut pending = Group::open("pending", &wt, &env).await.unwrap();
    // Distinct fixture commit dates select a folder snapshot deterministically;
    // they are not evidence of any elapsed retention or resource interval.
    pending.git.repo.env.extra = vec![
        ("GIT_AUTHOR_DATE".into(), "946684800 +0000".into()),
        ("GIT_COMMITTER_DATE".into(), "946684800 +0000".into()),
    ];
    crate::settings::check(&pending).await;
    let file = wt.join("Docs/notes.bin");
    std::fs::write(&file, b"original saved version\0\xff\n").unwrap();
    crate::capture::capture(&pending, &device, &tracer)
        .await
        .unwrap();
    let old = git.keys("main").await.unwrap()["Docs/notes.bin"].clone();
    pending.git.repo.env.extra = vec![
        ("GIT_AUTHOR_DATE".into(), "946684802 +0000".into()),
        ("GIT_COMMITTER_DATE".into(), "946684802 +0000".into()),
    ];
    std::fs::write(&file, b"latest saved version\0\xfe\n").unwrap();
    crate::capture::capture(&pending, &device, &tracer)
        .await
        .unwrap();
    let id = git.root("main").await.unwrap();
    let g = Arc::new(Group::open(&id, &wt, &env).await.unwrap());
    let d = Arc::new(Daemon {
        admissions: crate::admission_round::Store::open(dirs.state.join("admission.toml")),
        dirs,
        config: crate::config::Config::default(),
        device,
        env,
        groups: RwLock::new(vec![g.clone()]),
        node,
        net: OnceLock::new(),
        web: OnceLock::new(),
        wake: Arc::new(tokio::sync::Notify::new()),
        debug: crate::debug::Permissions::default(),
        tracer,
    });
    Some((temp, d, g, old))
}

#[derive(Clone, Copy)]
enum Change {
    Save,
    Remove,
    None,
}

async fn interleave(folder: bool, change: Change) {
    let Some((temp, d, g, old)) = fixture().await else {
        return;
    };
    let wt = g.worktree.as_ref().unwrap();
    let file = wt.join("Docs/notes.bin");
    let key = old;
    let requested: &[u8] = b"original saved version\0\xff\n";
    g.git
        .run(&["annex", "drop", "--force", "--key", &key])
        .await
        .unwrap();
    assert!(crate::custody::present(&g, &key).await.is_none());
    let pause = Arc::new(Paused::default());
    g.git.repo.set_work_control(pause.clone());
    let request = {
        let d = d.clone();
        let path = if folder {
            wt.join("Docs")
        } else {
            file.clone()
        };
        tokio::spawn(async move {
            restore(
                &d,
                &path,
                (!folder).then_some(1),
                folder.then_some("2000-01-01T00:00:01Z"),
            )
            .await
        })
    };
    tokio::time::timeout(Duration::from_secs(10), pause.waiting.notified())
        .await
        .unwrap();
    // This observes the actual get waiting for resource admission, without
    // a guessed sleep or an artificially short transfer deadline.
    let busy = g.busy.try_lock();
    assert!(
        busy.is_ok(),
        "a restore transfer must leave the worktree available"
    );
    let busy = busy.unwrap();
    let private = b"a newer private save while restore waits\0\xfd\n";
    if !matches!(change, Change::None) {
        std::fs::write(&file, private).unwrap();
        crate::capture::capture(&g, &d.device, &d.tracer)
            .await
            .unwrap();
    }
    let index = g.git.run(&["write-tree"]).await.unwrap();
    if matches!(change, Change::Remove) {
        d.groups.write().unwrap().clear();
    }
    drop(busy);
    if !matches!(change, Change::Remove) {
        let status = tokio::time::timeout(
            Duration::from_secs(10),
            crate::api::handle(
                &d,
                pigeon_proto::Request::Status {
                    path: Some(file.to_string_lossy().into_owned()),
                    not_shared: false,
                },
            ),
        )
        .await
        .unwrap();
        assert!(
            status.iter().any(|r| matches!(r, Response::Status(_))),
            "{status:?}"
        );
    }
    // Supply the exact saved object through an independent native driver;
    // it has no controller and does not alter the captured worktree/index.
    let supplied = temp.path().join("available-copy.bin");
    std::fs::write(&supplied, requested).unwrap();
    let supply = pigeon_git::Repo::new(wt, d.env.clone());
    supply
        .run(
            &["annex", "setkey", &key, supplied.to_str().unwrap()],
            None,
            pigeon_git::DEFAULT_TIMEOUT,
        )
        .await
        .unwrap();
    pause.allow.store(true, Ordering::SeqCst);
    let answer = tokio::time::timeout(Duration::from_secs(15), request)
        .await
        .unwrap()
        .unwrap();
    if matches!(change, Change::None) {
        let message = if folder {
            "restore-folder-result"
        } else {
            "restore-result"
        };
        assert!(
            answer
                .iter()
                .any(|r| matches!(r, Response::Said {message: m} if m.id == message)),
            "{answer:?}"
        );
        assert_eq!(std::fs::read(&file).unwrap(), requested);
    } else {
        let code = if matches!(change, Change::Remove) {
            "not-in-group"
        } else if folder {
            "restore-folder-incomplete"
        } else {
            "restore-in-use"
        };
        assert!(
            answer
                .iter()
                .any(|r| matches!(r, Response::Problem(p) if p.code == code)),
            "{answer:?}"
        );
        assert_eq!(std::fs::read(&file).unwrap(), private);
        assert_eq!(g.git.run(&["write-tree"]).await.unwrap(), index);
    }
    assert!(crate::custody::present(&g, &key).await.is_some());
    d.node.close().await;
}

#[tokio::test]
async fn paused_file_restore_allows_capture_and_keeps_its_original_baseline() {
    interleave(false, Change::Save).await;
}

#[tokio::test]
async fn paused_folder_restore_allows_capture_and_keeps_its_original_baselines() {
    interleave(true, Change::Save).await;
}

#[tokio::test]
async fn paused_restore_rechecks_a_group_removed_before_the_contents_arrive() {
    interleave(false, Change::Remove).await;
}

#[tokio::test]
async fn a_paused_file_restore_completes_when_its_baseline_is_unchanged() {
    interleave(false, Change::None).await;
}

#[tokio::test]
async fn a_paused_folder_restore_completes_when_its_baselines_are_unchanged() {
    interleave(true, Change::None).await;
}
