//! Install on a laptop (07 §7.5): `main` moves past the index, with another
//! laptop's versions, and install writes each clean path through the
//! journal without a commit: a changed file, a new one in a new folder, a
//! deletion. A save racing an install becomes a contribution concurrent
//! with the version install did not write, merged into `main`: the earlier
//! keeps the name, the other a conflict name, and the laptop whose save
//! lost the name keeps its file, a divergence in `conflicts.json`, until it
//! holds `main`'s version. Capture commits on `main`'s tree, never on a
//! lagging index's. A save within ten seconds of an install, as an
//! application holding the old content makes, is concurrent with the
//! installed version too (ADR 0071); a later one replaces it. A version
//! whose content is not here waits. A file renamed, alone or with a small
//! edit, is captured with its `Pigeon-Renamed:` trailer (ADR 0072).
//!
//! Needs git-annex on PATH; skipped without, unless
//! `PIGEON_REQUIRE_GIT_ANNEX` is set, as in CI.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
#![allow(clippy::print_stderr, reason = "tests")]

use std::collections::BTreeMap;
use std::path::PathBuf;

use pigeon_core::authority::{Entry, Tree};
use pigeon_core::roster::{Device, DeviceId, Person, PersonId, Role, Roster};
use pigeon_core::sig;
use pigeon_git::GitEnv;
use pigeond::git::Git;
use pigeond::group::Group;
use pigeond::install::Installed;
use pigeond::roster_file::{self, RosterDoc};
use pigeond::trace::Tracer;
use pigeond::{capture, install, merge};

const T0: i64 = 1_791_892_800;

fn key(i: u8) -> [u8; 32] {
    [i; 32]
}

fn dev(i: u8) -> DeviceId {
    sig::device_id(&key(i))
}

fn roster() -> Roster {
    let device = |p: &str, role, n: u8| Device {
        person: PersonId(p.into()),
        name: format!("{p}'s device {n}"),
        role,
        annex_uuid: Some(format!("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e0{n}")),
        maxsize: None,
    };
    let person = |name: &str, admin| Person {
        name: name.into(),
        admin,
    };
    Roster {
        rules: 1,
        group: "install".into(),
        k: 2,
        retention_days: 30,
        relay: None,
        persons: [
            (PersonId("ana".into()), person("Ana", true)),
            (PersonId("ben".into()), person("Ben", true)),
        ]
        .into(),
        devices: [
            (dev(1), device("ana", Role::Laptop, 1)),
            (dev(2), device("ben", Role::Laptop, 2)),
            (dev(3), device("ana", Role::Custodian, 3)),
        ]
        .into(),
        removed: BTreeMap::new(),
        folders: Vec::new(),
    }
}

fn annex_present() -> bool {
    std::process::Command::new("git")
        .args(["annex", "version", "--raw"])
        .output()
        .is_ok_and(|o| o.status.success())
}

struct Laptop {
    dir: tempfile::TempDir,
    wt: PathBuf,
    git: Git,
    g: Group,
    t: Tracer,
}

/// Ana's laptop, created as `pigeon create` makes one, without the network.
async fn laptop() -> Option<Laptop> {
    if !annex_present() {
        assert!(
            std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none(),
            "git-annex is required"
        );
        eprintln!("skipped: no git-annex");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let wt = dir.path().canonicalize().unwrap().join("folder");
    std::fs::create_dir_all(&wt).unwrap();
    let env = GitEnv {
        path_first: vec![],
        extra: vec![],
    };
    let git = Git::new(&wt, env.clone());
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "lap"]).await.unwrap();
    git.run(&["config", "user.email", "lap@pigeon"])
        .await
        .unwrap();
    let pending = Group::open("pending", &wt, &env).await.unwrap();
    pigeond::settings::check(&pending).await;
    git.run(&["annex", "init", "lap"]).await.unwrap();
    pigeond::git::set_signer(key(1));
    let text = roster_file::render(&RosterDoc { roster: roster() }).unwrap();
    let attributes = pigeond::settings::GITATTRIBUTES.as_bytes();
    pigeond::change::stage_file(&git, Some(&wt), ".gitattributes", attributes)
        .await
        .unwrap();
    pigeond::change::stage_file(&git, Some(&wt), ".pigeon/roster.toml", text.as_bytes())
        .await
        .unwrap();
    let genesis = pigeond::change::commit_index(&git, None, "pigeon: create install")
        .await
        .unwrap();
    drop(pending);
    let g = Group::open(&genesis, &wt, &env).await.unwrap();
    let t = Tracer::new(dir.path().join("traces"), &dev(1).0);
    Some(Laptop { dir, wt, git, g, t })
}

/// A version Ben saved: its pointer blob, and its content in this annex
/// when `here`, as a transfer would put it.
async fn version(l: &Laptop, name: &str, bytes: &[u8], here: bool) -> Entry {
    let at = l.dir.path().join("saved").join(format!("{}", bytes.len()));
    std::fs::create_dir_all(&at).unwrap();
    let file = at.join(name);
    std::fs::write(&file, bytes).unwrap();
    let file = file.to_str().unwrap();
    let k = l.git.run(&["annex", "calckey", file]).await.unwrap();
    let k = k.trim();
    if here {
        // Not a subcommand pigeond runs (03 §3.7): the test runs it itself.
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&l.wt)
            .args(["annex", "setkey", k, file])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }
    let pointer = format!("/annex/objects/{k}\n");
    let oid = l
        .git
        .run_in(&["hash-object", "-w", "--stdin"], pointer.as_bytes())
        .await
        .unwrap();
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: oid.trim().to_owned(),
    }
}

/// Ben's commit of `tree` on `parent`, `at` seconds after T0.
async fn bens(l: &Laptop, tree: &Tree, parent: &str, at: i64) -> String {
    let root = merge::write_tree(&l.git, tree).await.unwrap();
    let t = T0 + at;
    let body = format!(
        "tree {root}\nparent {parent}\nauthor pigeon <pigeon@localhost> {t} +0200\ncommitter pigeon <pigeon@localhost> {t} +0200\n\npigeon: capture\n"
    );
    let raw = sig::sign(body.as_bytes(), &key(2)).unwrap();
    let args = ["hash-object", "-t", "commit", "-w", "--stdin"];
    let c = l.git.run_in(&args, &raw).await.unwrap().trim().to_owned();
    let main = l.git.rev("refs/heads/main").await;
    l.git
        .update_ref("refs/heads/main", &c, main.as_deref())
        .await
        .unwrap();
    c
}

fn write(l: &Laptop, path: &str, bytes: &[u8]) {
    let file = l.wt.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, bytes).unwrap();
}

fn read(l: &Laptop, path: &str) -> String {
    std::fs::read_to_string(l.wt.join(path)).unwrap()
}

async fn clean(l: &Laptop) -> String {
    l.git.run(&["status", "--porcelain"]).await.unwrap()
}

#[tokio::test]
async fn install_writes_the_versions_main_gained() {
    let Some(l) = laptop().await else {
        return;
    };
    write(&l, "a.txt", b"one\n");
    write(&l, "b.txt", b"bee\n");
    let c1 = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&c1).await.unwrap();
    tree.insert("a.txt".into(), version(&l, "a.txt", b"two\n", true).await);
    tree.insert(
        "sub/n.txt".into(),
        version(&l, "n.txt", b"new!\n", true).await,
    );
    tree.remove("b.txt");
    let c2 = bens(&l, &tree, &c1, 60).await;
    // The index lags `main`: capture finds nothing of the person's.
    let none = capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(none.commit, None, "{none:?}");
    assert_eq!(
        l.git.rev("refs/heads/main").await.as_deref(),
        Some(c2.as_str())
    );
    let done = install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    let want = Installed {
        written: 2,
        removed: 1,
        receiving: 0,
        deferred: 0,
    };
    assert_eq!(done, want);
    assert_eq!(read(&l, "a.txt"), "two\n");
    assert_eq!(read(&l, "sub/n.txt"), "new!\n");
    assert!(!l.wt.join("b.txt").exists());
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(l.wt.join("a.txt"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o200, 0o200, "the person can save it: {mode:o}");
    assert_eq!(clean(&l).await, "");
    assert_eq!(
        capture::capture(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .commit,
        None
    );
    assert_eq!(
        install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap(),
        Installed::default()
    );
    let journal = std::fs::read(l.g.files.install_journal()).unwrap_or_default();
    let entries =
        pigeon_core::journal::entries(&pigeond::journal_file::read(&l.g.files.install_journal()));
    assert!(
        !journal.is_empty(),
        "old asides stay journaled for their keep period"
    );
    assert!(
        entries.values().all(|e| e.end.is_some()),
        "install ended every entry"
    );
    assert!(entries.values().any(|e| !e.released));
    // Ben deletes the only file of `sub/`: the folder goes with it.
    let mut tree = l.git.tree(&c2).await.unwrap();
    tree.remove("sub/n.txt");
    bens(&l, &tree, &c2, 120).await;
    let done = install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(done.removed, 1, "{done:?}");
    assert!(!l.wt.join("sub").exists());
    assert_eq!(clean(&l).await, "");
}

#[tokio::test]
async fn a_save_racing_an_install_is_kept_both_ways() {
    let Some(l) = laptop().await else {
        return;
    };
    write(&l, "a.txt", b"one\n");
    let c1 = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&c1).await.unwrap();
    let theirs = version(&l, "a.txt", b"two\n", true).await;
    tree.insert("a.txt".into(), theirs.clone());
    let c2 = bens(&l, &tree, &c1, 60).await;
    // Ana saves before install wrote Ben's version: her save is a
    // contribution concurrent with his, merged into `main`.
    write(&l, "a.txt", b"mine\n");
    let m = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    assert!(l.g.lock().diverging.is_empty());
    let parents = l
        .git
        .run(&["rev-parse", &format!("{m}^1"), &format!("{m}^2")])
        .await;
    assert_eq!(parents.unwrap().lines().next(), Some(c2.as_str()));
    let tm = l.git.tree(&m).await.unwrap();
    assert_eq!(
        tm.get("a.txt"),
        Some(&theirs),
        "the earlier save keeps the name"
    );
    let copies: Vec<&String> = tm
        .keys()
        .filter(|q| pigeon_core::names::is_copy_of(q, "a.txt"))
        .collect();
    let [copy] = copies.as_slice() else {
        panic!("one copy: {tm:?}");
    };
    assert!(copy.starts_with("a (Ana, "), "{copy}");
    // Install writes the copy and leaves Ana's file: she lost the name, and
    // the path diverges (`concurrent`) until she chooses.
    let done = install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!((done.written, done.deferred), (1, 1), "{done:?}");
    assert_eq!(read(&l, "a.txt"), "mine\n");
    assert_eq!(read(&l, copy), "mine\n");
    let table = pigeond::conflicts_file::read(&l.g).unwrap();
    let d = &table.paths["a.txt"];
    assert_eq!(d.kind, pigeond::conflicts_file::Kind::Concurrent);
    assert!(
        d.mine.is_some() && d.group.is_some() && d.mine != d.group,
        "{d:?}"
    );
    // Another save is committed on `main`'s tree, not the index's.
    write(&l, "c.txt", b"sea\n");
    let c3 = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let t3 = l.git.tree(&c3).await.unwrap();
    assert_eq!(t3.get("a.txt"), Some(&theirs), "Ben's version stays");
    assert!(t3.contains_key("c.txt") && t3.contains_key(copy.as_str()));
    assert_eq!(
        install::incoming(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .written,
        0
    );
    assert_eq!(read(&l, "a.txt"), "mine\n", "never written");
    // Once the file holds Ben's version, the divergence ends.
    write(&l, "a.txt", b"two\n");
    assert_eq!(
        capture::capture(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .commit,
        None
    );
    install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert!(
        pigeond::conflicts_file::read(&l.g)
            .unwrap()
            .paths
            .is_empty()
    );
    assert_eq!(clean(&l).await, "");
}

#[tokio::test]
async fn a_save_soon_after_an_install_is_kept_beside_it() {
    let Some(l) = laptop().await else {
        return;
    };
    write(&l, "a.txt", b"one\n");
    write(&l, "b.txt", b"bee\n");
    let c1 = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&c1).await.unwrap();
    let theirs = version(&l, "a.txt", b"two\n", true).await;
    tree.insert("a.txt".into(), theirs.clone());
    tree.insert("b.txt".into(), version(&l, "b.txt", b"bee 2\n", true).await);
    bens(&l, &tree, &c1, 60).await;
    let done = install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(done.written, 2, "{done:?}");
    assert_eq!(read(&l, "a.txt"), "two\n");
    // A save of b.txt long after its install replaces the installed version.
    l.g.lock().installed.get_mut("b.txt").unwrap().0 -= 30 * 1_000_000_000;
    write(&l, "b.txt", b"bee\n");
    let c3 = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let parents = l.git.run(&["rev-list", "--parents", "-n1", &c3]).await;
    assert_eq!(parents.unwrap().split_whitespace().count(), 2, "one parent");
    let t3 = l.git.tree(&c3).await.unwrap();
    assert!(
        !t3.keys()
            .any(|q| pigeon_core::names::is_copy_of(q, "b.txt")),
        "{t3:?}"
    );
    // An application that held a.txt's old content saves it at once: the
    // save goes beside Ben's version, which keeps the name.
    write(&l, "a.txt", b"one\n");
    let m = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let first = l.git.run(&["rev-parse", &format!("{m}^")]).await.unwrap();
    assert_eq!(first.trim(), c3, "a commit on main");
    let tm = l.git.tree(&m).await.unwrap();
    assert_eq!(
        tm.get("a.txt"),
        Some(&theirs),
        "the installed version stays"
    );
    let copies: Vec<&String> = tm
        .keys()
        .filter(|q| pigeon_core::names::is_copy_of(q, "a.txt"))
        .collect();
    let [copy] = copies.as_slice() else {
        panic!("one copy: {tm:?}");
    };
    assert!(copy.starts_with("a (Ana, "), "{copy}");
    // Install writes the copy and leaves Ana's file, which diverges as one
    // that lost the name does (07 §7.6).
    install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!(read(&l, copy), "one\n", "the save is kept beside it");
    assert_eq!(read(&l, "a.txt"), "one\n", "Ana's file is not overwritten");
    let table = pigeond::conflicts_file::read(&l.g).unwrap();
    assert_eq!(
        table.paths["a.txt"].kind,
        pigeond::conflicts_file::Kind::Concurrent
    );
    assert!(
        !l.g.lock().installed.contains_key("a.txt"),
        "a record is used once"
    );
    assert_eq!(
        capture::capture(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .commit,
        None,
        "nothing more to capture"
    );
}

#[tokio::test]
async fn a_version_whose_content_is_not_here_waits() {
    let Some(l) = laptop().await else {
        return;
    };
    write(&l, "a.txt", b"one\n");
    let c1 = capture::capture(&l.g, &dev(1).0, &l.t)
        .await
        .unwrap()
        .commit
        .unwrap();
    let mut tree = l.git.tree(&c1).await.unwrap();
    tree.insert("a.txt".into(), version(&l, "a.txt", b"far\n", false).await);
    bens(&l, &tree, &c1, 60).await;
    let done = install::incoming(&l.g, &dev(1).0, &l.t).await.unwrap();
    assert_eq!((done.receiving, done.written), (1, 0), "{done:?}");
    assert_eq!(read(&l, "a.txt"), "one\n");
    assert_eq!(
        capture::capture(&l.g, &dev(1).0, &l.t)
            .await
            .unwrap()
            .commit,
        None
    );
    assert!(l.g.lock().diverging.is_empty());
}

#[tokio::test]
async fn a_renamed_file_is_captured_with_its_trailer() {
    let Some(l) = laptop().await else {
        return;
    };
    let text = b"one\ntwo\nthree\nfour\nfive\nsix\n";
    write(&l, "a.txt", b"apple\n");
    write(&l, "c.txt", text);
    write(&l, "e.txt", b"eel\n");
    capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    // a.txt moves, c.txt moves with an edit, e.txt is replaced by another.
    std::fs::create_dir_all(l.wt.join("sub")).unwrap();
    std::fs::rename(l.wt.join("a.txt"), l.wt.join("sub/b.md")).unwrap();
    std::fs::remove_file(l.wt.join("c.txt")).unwrap();
    write(&l, "d.txt", b"one\ntwo\nthree\nfour\nfive\nseven\n");
    std::fs::remove_file(l.wt.join("e.txt")).unwrap();
    write(&l, "f.txt", b"fig\n");
    let c = capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
    let raw = l.git.object(&c.commit.unwrap()).await.unwrap();
    let got = pigeon_core::authority::renamed(&raw);
    let pair = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    // Same contents pair first, whatever the extension, then similar ones.
    assert_eq!(got, [pair("a.txt", "sub/b.md"), pair("c.txt", "d.txt")]);
}

#[path = "install/storage.rs"]
mod storage;
