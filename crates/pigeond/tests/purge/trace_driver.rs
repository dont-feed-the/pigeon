//! Fixed-membership purge interleavings over actual signed histories and
//! three real annex stores. File remotes control metadata partitions;
//! zero-day retention checks ordering, not an elapsed month. Reports use
//! the production builder and codec, signed with the fixture devices' keys.

use super::*;
use pigeond::purge_file::PurgeDoc;
use serde_json::{Value, json};

#[path = "trace_observe.rs"]
mod observe;

const IDS: [&str; 3] = ["l1", "c1", "c2"];
const NUMBERS: [u8; 3] = [1, 3, 4];
const BYTES: &[u8] = b"partitioned erasure trace\x00\xff\r\n";

struct World {
    laptop: Laptop,
    backups: Vec<Group>,
    key: String,
    old: String,
    decision: String,
    request: String,
    restored: Option<String>,
    cancelled: Option<String>,
    events: Vec<Value>,
}

impl World {
    fn group(&self, i: usize) -> &Group {
        if i == 0 {
            &self.laptop.g
        } else {
            &self.backups[i - 1]
        }
    }

    async fn new() -> Option<Self> {
        let l = laptop_fixture_options(2, 0).await?;
        let upgraded = purge_upgrade::upgrade(&l.g).await;
        let pointer = version(&l, "history.txt", BYTES, true).await;
        let mut tree = l.git.tree(&upgraded).await.unwrap();
        tree.insert("Shared/history.txt".into(), pointer);
        let old =
            purge_content::signed(&l, &tree, &[&upgraded], pigeond::util::now() - 10, 1).await;
        adopt(&l.g, &old).await;
        let content = l.git.keys(&old).await.unwrap()["Shared/history.txt"].clone();
        let mut backups = Vec::new();
        for n in [3, 4] {
            let path = l.dir.path().join(if n == 3 {
                "custodian.git".into()
            } else {
                format!("custodian-{n}.git")
            });
            let git = Git::new(&path, GitEnv::default());
            git.run(&[
                "fetch",
                "-q",
                l.wt.to_str().unwrap(),
                "+refs/heads/main:refs/heads/main",
                &pigeond::purge_epoch::refspec("fixture"),
            ])
            .await
            .unwrap();
            let g = Group::open(&l.g.id, &path, &GitEnv::default())
                .await
                .unwrap();
            merge::judge_history(&g, std::slice::from_ref(&old))
                .await
                .unwrap();
            assert!(g.lock().engine.accepted(&old));
            // Fixture population precedes the observed run. No setkey is
            // used after Init: every later disappearance is a real purge.
            let file = l.dir.path().join(format!("initial-{n}"));
            std::fs::write(&file, BYTES).unwrap();
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["annex", "setkey", &content, file.to_str().unwrap()])
                .output()
                .unwrap();
            assert!(status.status.success(), "{:?}", status);
            backups.push(g);
        }
        let doc = PurgeDoc {
            format: 1,
            kind: "purge".into(),
            history: true,
            paths: vec!["Shared/history.txt".into()],
            base: old.clone(),
            keys: vec![],
            proposals: vec![],
            requested_by: "ana".into(),
            retention_end: pigeond::report_file::datetime(pigeond::util::now() - 1),
        };
        let request = ".pigeon/decisions/purge-trace.toml".to_owned();
        tree.remove("Shared/history.txt");
        tree.insert(
            request.clone(),
            blob(&l.g, pigeond::purge_file::render(&doc).unwrap().as_bytes()).await,
        );
        let decision = purge_content::signed(&l, &tree, &[&old], pigeond::util::now() - 2, 1).await;
        // The decision object is prepared but not referenced at Init.
        let mut world = Self {
            laptop: l,
            backups,
            key: content,
            old,
            decision,
            request,
            restored: None,
            cancelled: None,
            events: vec![],
        };
        world.observe("Init", 0, 0).await;
        Some(world)
    }

    async fn decide(&mut self) {
        adopt(self.group(0), &self.decision).await;
        self.observe("Decide", 0, 0).await;
    }

    async fn history(&mut self, from: usize, to: usize) {
        let source = self.group(from).git.repo.dir.to_str().unwrap();
        let head = self.group(from).git.rev("refs/heads/main").await.unwrap();
        self.group(to)
            .git
            .run(&[
                "fetch",
                "-q",
                source,
                "+refs/heads/main:refs/pigeon/peers/trace",
            ])
            .await
            .unwrap();
        adopt(self.group(to), &head).await;
        self.observe("SyncHistory", from, to).await;
    }

    async fn seal(&mut self, i: usize) {
        let g = self.group(i);
        let _busy = g.busy.lock().await;
        pigeond::purge::prepare(g).await;
        pigeond::purge::round(g, &dev(NUMBERS[i]).0, false).await;
        assert_eq!(g.lock().purge_seals.len(), 1);
        publish(g, NUMBERS[i]).await;
        drop(_busy);
        self.observe("Seal", i, i).await;
    }

    async fn reports(&mut self, from: usize, to: usize) {
        let source = self.group(from).git.repo.dir.to_str().unwrap();
        self.group(to)
            .git
            .run(&[
                "fetch",
                "-q",
                source,
                "+refs/pigeon/status/*:refs/pigeon/status/*",
                "+refs/pigeon/purge-seals/*:refs/pigeon/purge-seals/*",
            ])
            .await
            .unwrap();
        self.observe("SyncReports", from, to).await;
    }

    async fn drop_copy(&mut self, i: usize, expected: bool) {
        let g = self.group(i);
        let before = pigeond::custody::present(g, &self.key).await.is_some();
        let _busy = g.busy.lock().await;
        pigeond::purge::prepare(g).await;
        pigeond::purge::round(g, &dev(NUMBERS[i]).0, true).await;
        assert_eq!(
            pigeond::custody::present(g, &self.key).await.is_none(),
            expected
        );
        drop(_busy);
        self.observe(
            if expected && before {
                "Drop"
            } else {
                "Stutter"
            },
            i,
            i,
        )
        .await;
    }

    async fn place(&mut self, i: usize, expected: usize) {
        let source = self.group(0);
        source.git.run(&["annex", "merge"]).await.unwrap();
        let g = self.group(i);
        g.git
            .run(&[
                "config",
                "remote.trace-laptop.url",
                source.git.repo.dir.to_str().unwrap(),
            ])
            .await
            .unwrap();
        let uuid = source.git.config("annex.uuid").await.unwrap();
        g.git
            .run(&["config", "remote.trace-laptop.annex-uuid", &uuid])
            .await
            .unwrap();
        g.git
            .run(&[
                "fetch",
                "-q",
                "trace-laptop",
                "+refs/heads/git-annex:refs/remotes/trace-laptop/git-annex",
            ])
            .await
            .unwrap();
        g.git.run(&["annex", "merge"]).await.unwrap();
        pigeond::purge::prepare(g).await;
        pigeond::purge::round(g, &dev(NUMBERS[i]).0, false).await;
        let made = pigeond::custody::place(g, &dev(NUMBERS[i]).0).await;
        let shortfall = g.lock().shortfall.clone();
        let retained = pigeond::retention::keys(g).await;
        let holders = pigeond::custody::holders(&g.git, &self.key).await;
        assert_eq!(
            made,
            expected,
            "room={}, shortfall={shortfall:?}, retained={retained:?}, holders={holders:?}",
            pigeond::admission::room_of(g)
        );
        assert_eq!(
            pigeond::custody::present(g, &self.key).await.is_some(),
            expected > 0
        );
        self.observe(if made > 0 { "Fetch" } else { "Stutter" }, 0, i)
            .await;
    }

    async fn restore(&mut self) {
        let g = self.group(0);
        let parent = g.git.rev("refs/heads/main").await.unwrap();
        let mut tree = g.git.tree(&parent).await.unwrap();
        tree.insert(
            "Shared/restored.txt".into(),
            g.git.tree(&self.old).await.unwrap()["Shared/history.txt"].clone(),
        );
        let restored =
            purge_content::signed(&self.laptop, &tree, &[&parent], pigeond::util::now(), 1).await;
        adopt(g, &restored).await;
        self.restored = Some(restored);
        self.observe("Restore", 0, 0).await;
    }

    async fn cancel(&mut self) {
        let g = self.group(0);
        let parent = g.git.rev("refs/heads/main").await.unwrap();
        let mut tree = g.git.tree(&parent).await.unwrap();
        tree.remove(&self.request);
        let cancelled =
            purge_content::signed(&self.laptop, &tree, &[&parent], pigeond::util::now(), 1).await;
        adopt(g, &cancelled).await;
        self.cancelled = Some(cancelled);
        self.observe("Cancel", 0, 0).await;
    }

    fn save(&self, case: &str) {
        if let Some(out) = std::env::var_os("PIGEON_PURGE_TRACE_DIR") {
            let path = PathBuf::from(out).join(case);
            std::fs::create_dir_all(&path).unwrap();
            let text: String = self.events.iter().map(|e| format!("{e}\n")).collect();
            std::fs::write(path.join("trace.ndjson"), text).unwrap();
        }
    }
}

async fn adopt(g: &Group, head: &str) {
    merge::judge_history(g, &[head.into()]).await.unwrap();
    assert!(
        g.lock().engine.accepted(head),
        "unaccepted {head}: {:?}",
        g.lock().verdicts.get(head)
    );
    let old = g.git.rev("refs/heads/main").await;
    g.git
        .update_ref("refs/heads/main", head, old.as_deref())
        .await
        .unwrap();
    g.reload_roster().await;
}

async fn blob(g: &Group, bytes: &[u8]) -> Entry {
    let oid = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], bytes)
        .await
        .unwrap();
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: oid.trim().into(),
    }
}

async fn publish(g: &Group, signer: u8) {
    let report = pigeond::report::build(g, &dev(signer).0).await;
    assert_eq!(report.purge.len(), 1);
    let tree = [(
        "status.toml".into(),
        blob(g, pigeond::report_file::render(&report).unwrap().as_bytes()).await,
    )]
    .into();
    let root = merge::write_tree(&g.git, &tree).await.unwrap();
    let raw = format!(
        "tree {root}\nauthor pigeon <pigeon@localhost> {} +0000\ncommitter pigeon <pigeon@localhost> {} +0000\n\npigeon: observed report\n",
        pigeond::util::now(),
        pigeond::util::now()
    );
    let raw = sig::sign(raw.as_bytes(), &key(signer)).unwrap();
    let oid = g
        .git
        .run_in(&["hash-object", "-t", "commit", "-w", "--stdin"], &raw)
        .await
        .unwrap();
    let name = format!("refs/pigeon/status/{}", dev(signer).0);
    let old = g.git.rev(&name).await;
    g.git
        .update_ref(&name, oid.trim(), old.as_deref())
        .await
        .unwrap();
}

#[tokio::test]
async fn purge_trace_restore_reaches_a_backup_before_its_seal() {
    let Some(mut w) = World::new().await else {
        return;
    };
    w.decide().await;
    w.history(0, 1).await;
    w.history(0, 2).await;
    w.seal(1).await;
    w.restore().await;
    w.history(0, 2).await;
    w.seal(2).await;
    // Reports arrive before the restoring history. A full signed seal
    // cannot authorize a drop without adopting the reference it saw.
    w.reports(2, 1).await;
    w.drop_copy(1, false).await;
    w.history(2, 1).await;
    w.reports(1, 2).await;
    w.drop_copy(1, false).await;
    w.drop_copy(2, false).await;
    w.save("timely-restore");
}

#[tokio::test]
async fn purge_trace_late_restore_after_both_backup_copies_drop() {
    let Some(mut w) = World::new().await else {
        return;
    };
    w.decide().await;
    w.history(0, 1).await;
    w.history(0, 2).await;
    w.seal(1).await;
    w.seal(2).await;
    w.reports(1, 2).await;
    w.reports(2, 1).await;
    w.drop_copy(1, true).await;
    w.drop_copy(2, true).await;
    w.place(1, 0).await;
    // The partitioned laptop still has the exact bytes. Its late restore
    // is a fresh signed reference, not permission to revive old history.
    w.restore().await;
    w.history(0, 1).await;
    w.history(0, 2).await;
    w.drop_copy(1, true).await;
    w.drop_copy(2, true).await;
    assert!(!w.group(1).git.purge_gate.policy().released.contains(&w.key));
    assert!(!w.group(2).git.purge_gate.policy().released.contains(&w.key));
    w.place(1, 1).await;
    w.place(2, 1).await;
    w.save("late-restore");
}

#[tokio::test]
async fn purge_trace_cancel_reaches_an_unsealed_backup() {
    let Some(mut w) = World::new().await else {
        return;
    };
    w.decide().await;
    w.history(0, 1).await;
    w.history(0, 2).await;
    w.seal(1).await;
    w.cancel().await;
    w.history(0, 2).await;
    w.drop_copy(2, false).await;
    w.history(0, 1).await;
    w.drop_copy(1, false).await;
    w.save("timely-cancel");
}

#[tokio::test]
async fn purge_trace_a_late_restore_with_no_copy_remains_recorded_and_alerts() {
    let Some(mut w) = World::new().await else {
        return;
    };
    w.decide().await;
    w.history(0, 1).await;
    w.history(0, 2).await;
    w.seal(1).await;
    w.seal(2).await;
    w.reports(1, 2).await;
    w.reports(2, 1).await;
    w.drop_copy(1, true).await;
    w.drop_copy(2, true).await;
    w.reports(2, 0).await;
    w.drop_copy(0, true).await;
    w.restore().await;
    w.history(0, 1).await;
    w.place(1, 0).await;
    let head = w.group(0).git.rev("refs/heads/main").await.unwrap();
    for restart in [false, true] {
        let reopened;
        let g = if restart {
            reopened = Group::open(&w.laptop.g.id, &w.laptop.wt, &GitEnv::default())
                .await
                .unwrap();
            &reopened
        } else {
            w.group(0)
        };
        pigeond::purge::prepare(g).await;
        pigeond::purge::round(g, &dev(1).0, false).await;
        let report = pigeond::status::report(g, &dev(1).0, "", false).await;
        assert_eq!(report.level, pigeon_proto::api::Level::NeedsYou);
        assert_eq!(
            report.state,
            pigeon_proto::api::State::Missing,
            "restart={restart}; headline={}; files={:?}",
            report.headline.id,
            report
                .versions
                .iter()
                .map(|v| (&v.path, &v.state, v.reason.as_ref().map(|m| &m.id)))
                .collect::<Vec<_>>()
        );
        assert_eq!(report.headline.id, "headline-erased-restoration-missing");
        assert_eq!(g.git.rev("refs/heads/main").await.as_ref(), Some(&head));
        assert!(!g.git.purge_gate.policy().released.contains(&w.key));
    }
    let private = w.laptop.wt.join("private.txt");
    std::fs::write(&private, b"a separate private save\n").unwrap();
    pigeond::change::stage_file(
        &w.laptop.git,
        Some(&w.laptop.wt),
        "staged.txt",
        b"a staged private save\n",
    )
    .await
    .unwrap();
    let report = pigeond::status::report(w.group(0), &dev(1).0, "", false).await;
    assert!(
        report
            .versions
            .iter()
            .any(|v| v.path == "staged.txt" && v.state == pigeon_proto::api::State::Local)
    );
    assert!(
        report
            .versions
            .iter()
            .any(|v| v.path == "private.txt" && v.state == pigeon_proto::api::State::Local)
    );
    assert!(report.versions.iter().any(|v| {
        v.path == "Shared/restored.txt"
            && v.state == pigeon_proto::api::State::Missing
            && v.reason
                .as_ref()
                .is_some_and(|m| m.id == "why-erased-restoration-missing")
    }));
    assert_eq!(
        std::fs::read(&private).unwrap(),
        b"a separate private save\n"
    );
    w.observe("Stutter", 0, 0).await;
    w.save("late-missing");
}

#[tokio::test]
async fn missing_erased_contents_restored_from_a_refounded_archive_alert_the_decider() {
    let Some(mut w) = World::new().await else {
        return;
    };
    w.decide().await;
    w.history(0, 1).await;
    w.history(0, 2).await;
    w.seal(1).await;
    w.seal(2).await;
    w.reports(1, 2).await;
    w.reports(2, 1).await;
    w.drop_copy(1, true).await;
    w.drop_copy(2, true).await;
    w.reports(2, 0).await;
    w.drop_copy(0, true).await;
    assert!(pigeond::purge_epoch::close(w.group(0)).await.unwrap());
    let old_main = w.group(0).git.rev("refs/heads/main").await.unwrap();
    let tree = pigeond::refound::new_tree(w.group(0), &old_main, &dev(1))
        .await
        .unwrap();
    let root = merge::write_tree(&w.laptop.git, &tree).await.unwrap();
    let message = format!(
        "pigeon: refound\n\nPigeon-Refound: {} {old_main}",
        w.laptop.g.id
    );
    let new = w
        .laptop
        .git
        .commit_tree(&root, &[], &message)
        .await
        .unwrap();
    let archive = pigeond::refound::archive_ref(&w.laptop.g.id);
    w.laptop
        .git
        .update_ref(&archive, &old_main, None)
        .await
        .unwrap();
    w.laptop
        .git
        .update_ref("refs/heads/main", &new, Some(&old_main))
        .await
        .unwrap();
    let g = Group::open(&new, &w.laptop.wt, &GitEnv::default())
        .await
        .unwrap();
    merge::judge_history(&g, std::slice::from_ref(&new))
        .await
        .unwrap();
    assert!(g.lock().engine.accepted(&new));
    pigeond::purge::prepare(&g).await;
    pigeond::purge::round(&g, &dev(1).0, false).await;
    assert!(!g.git.purge_gate.policy().waiting);
    assert!(g.git.purge_gate.policy().released.contains(&w.key));
    let mut tree = g.git.tree(&new).await.unwrap();
    tree.insert(
        "Recovered/history.txt".into(),
        g.git.tree(&w.old).await.unwrap()["Shared/history.txt"].clone(),
    );
    let restored = purge_content::signed(&w.laptop, &tree, &[&new], pigeond::util::now(), 1).await;
    adopt(&g, &restored).await;
    for restart in [false, true] {
        let reopened;
        let group = if restart {
            reopened = Group::open(&new, &w.laptop.wt, &GitEnv::default())
                .await
                .unwrap();
            &reopened
        } else {
            &g
        };
        pigeond::purge::prepare(group).await;
        pigeond::purge::round(group, &dev(1).0, false).await;
        assert!(!group.git.purge_gate.policy().waiting);
        assert!(!group.git.purge_gate.policy().released.contains(&w.key));
        let report = pigeond::status::report(group, &dev(1).0, "", false).await;
        assert_eq!(
            report.level,
            pigeon_proto::api::Level::NeedsYou,
            "restart={restart}, headline={}",
            report.headline.id
        );
        assert_eq!(report.headline.id, "headline-erased-restoration-missing");
        assert!(report.versions.iter().any(|v| {
            v.path == "Recovered/history.txt"
                && v.state == pigeon_proto::api::State::Missing
                && v.reason
                    .as_ref()
                    .is_some_and(|m| m.id == "why-erased-restoration-missing")
        }));
    }
    g.git.run(&["update-ref", "-d", &archive]).await.unwrap();
    pigeond::purge::round(&g, &dev(1).0, false).await;
    assert!(g.git.purge_gate.policy().waiting);
    let report = pigeond::status::report(&g, &dev(1).0, "", false).await;
    assert!(report.versions.iter().all(|v| {
        v.reason
            .as_ref()
            .is_none_or(|m| m.id != "why-erased-restoration-missing")
    }));
}
