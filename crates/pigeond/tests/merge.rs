//! Accept's judge over merges (04 §4.4, 05 §5.2) on a custodian's bare
//! repository: two laptops edit one file from the genesis; `merge` judges
//! both sides, writes the merge whose tree is the catalogue of both, signed
//! by the custodian, and judges it clean; a commit signed outside the
//! roster is refused, and so are a non-portable name and a merge of roster
//! forks that does not hold their fold; forks merge by the fold (04 §4.8).
//! A rename merged with a concurrent edit gives the edit the new name (ADR
//! 0072).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]

use std::collections::BTreeMap;

use pigeon_core::authority::{Entry, ROSTER, Tree};
use pigeon_core::roster::{Device, DeviceId, Folder, Person, PersonId, Role, Roster, Rule};
use pigeon_core::sig;
use pigeon_git::GitEnv;
use pigeond::git::Git;
use pigeond::group::Group;
use pigeond::roster_file::{self, RosterDoc};
use pigeond::{merge, verify};

#[path = "merge/fetch_progress.rs"]
mod fetch_progress;

const T0: i64 = 1_791_892_800;

fn key(i: u8) -> [u8; 32] {
    [i; 32]
}

fn dev(i: u8) -> DeviceId {
    sig::device_id(&key(i))
}

fn roster(k: u32) -> Roster {
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
        group: "merge".into(),
        invitation: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        k,
        retention_days: 30,
        telemetry_device: None,
        relay: None,
        persons: [
            (PersonId("ana".into()), person("Ana", true)),
            (PersonId("ben".into()), person("Ben", false)),
        ]
        .into(),
        devices: [
            (dev(1), device("ana", Role::Laptop, 1)),
            (dev(2), device("ben", Role::Laptop, 2)),
            (dev(3), device("ana", Role::Custodian, 3)),
        ]
        .into(),
        removed: BTreeMap::new(),
        folders: vec![Folder {
            path: "Shared".into(),
            rule: Rule::AddOnly([PersonId("ana".into()), PersonId("ben".into())].into()),
        }],
    }
}

async fn blob(git: &Git, bytes: &[u8]) -> Entry {
    let oid = git
        .run_in(&["hash-object", "-w", "--stdin"], bytes)
        .await
        .unwrap();
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: oid.trim().to_owned(),
    }
}

async fn pointer(git: &Git, n: u8) -> Entry {
    let hex = pigeond::util::hex(&[n; 32]);
    blob(
        git,
        format!("/annex/objects/SHA256E-s1--{hex}.txt\n").as_bytes(),
    )
    .await
}

/// A commit of `tree` on `parents`, at `T0 + at` seconds, signed by `signer`.
async fn commit(git: &Git, tree: &Tree, parents: &[&str], at: i64, signer: u8) -> String {
    commit_with(git, tree, parents, (at, signer), "pigeon: capture").await
}

/// The same, with `message`.
async fn commit_with(
    git: &Git,
    tree: &Tree,
    parents: &[&str],
    (at, signer): (i64, u8),
    message: &str,
) -> String {
    let root = merge::write_tree(git, tree).await.unwrap();
    let mut body = format!("tree {root}\n");
    for p in parents {
        body.push_str(&format!("parent {p}\n"));
    }
    let t = T0 + at;
    body.push_str(&format!(
        "author pigeon <pigeon@localhost> {t} +0200\ncommitter pigeon <pigeon@localhost> {t} +0200\n\n{message}\n"
    ));
    let raw = sig::sign(body.as_bytes(), &key(signer)).unwrap();
    let args = ["hash-object", "-t", "commit", "-w", "--stdin"];
    git.run_in(&args, &raw).await.unwrap().trim().to_owned()
}

struct World {
    _dir: tempfile::TempDir,
    git: Git,
    g: Group,
    genesis: String,
    base: Tree,
}

async fn world() -> World {
    world_with_roster(roster(2)).await
}

async fn world_with_roster(roster: Roster) -> World {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo.git");
    let env = GitEnv {
        path_first: vec![],
        extra: vec![],
    };
    let status = std::process::Command::new("git")
        .args(["init", "-q", "--bare", "-b", "main"])
        .arg(&repo)
        .status()
        .unwrap();
    assert!(status.success());
    let git = Git::new(&repo, env.clone());
    git.run(&["config", "user.name", "pigeon"]).await.unwrap();
    git.run(&["config", "user.email", "pigeon@localhost"])
        .await
        .unwrap();
    let text = roster_file::render(&RosterDoc { roster }).unwrap();
    let mut base = Tree::new();
    base.insert(ROSTER.into(), blob(&git, text.as_bytes()).await);
    base.insert("Shared/a.txt".into(), pointer(&git, 0).await);
    let genesis = commit(&git, &base, &[], 0, 1).await;
    git.update_ref("refs/heads/main", &genesis, None)
        .await
        .unwrap();
    let g = Group::open(&genesis, &repo, &env).await.unwrap();
    World {
        _dir: dir,
        git,
        g,
        genesis,
        base,
    }
}

#[tokio::test]
async fn two_laptops_edits_merge_into_one_catalogue() {
    let w = world().await;
    pigeond::git::set_signer(key(3));
    let mut a = w.base.clone();
    a.insert("Shared/a.txt".into(), pointer(&w.git, 1).await);
    let a1 = commit(&w.git, &a, &[&w.genesis], 60, 1).await;
    let mut b = w.base.clone();
    b.insert("Shared/a.txt".into(), pointer(&w.git, 2).await);
    b.insert("Shared/b.txt".into(), pointer(&w.git, 3).await);
    let b1 = commit(&w.git, &b, &[&w.genesis], 120, 2).await;
    let judged = verify::judge(&w.g, Some(&a1), &b1).await.unwrap();
    assert_eq!(judged.accepted.as_deref(), Some(b1.as_str()), "{judged:?}");
    assert!(merge::brings(&w.g, &a1, &b1).await.unwrap());
    let m = merge::merge(&w.g, &a1, &b1)
        .await
        .unwrap()
        .expect("a clean merge");
    let tree = w.git.tree(&m).await.unwrap();
    let paths: Vec<&str> = tree.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        [
            ".pigeon/roster.toml",
            "Shared/a (Ben, 2026-10-13 14h02).txt",
            "Shared/a.txt",
            "Shared/b.txt"
        ]
    );
    assert_eq!(
        tree["Shared/a.txt"], a["Shared/a.txt"],
        "the earlier save keeps the name"
    );
    assert_eq!(
        tree["Shared/a (Ben, 2026-10-13 14h02).txt"],
        b["Shared/a.txt"]
    );
    assert!(
        !merge::brings(&w.g, &m, &a1).await.unwrap(),
        "nothing new on a side"
    );
    let open = w.g.lock().engine.open(std::slice::from_ref(&m));
    assert_eq!(open, ["shared/a.txt".to_owned()].into());
    // Ben edits his copy on top of the merge: the conflict closes.
    let mut c = tree.clone();
    c.insert(
        "Shared/a (Ben, 2026-10-13 14h02).txt".into(),
        pointer(&w.git, 4).await,
    );
    let c1 = commit(&w.git, &c, &[&m], 180, 2).await;
    let judged = verify::judge(&w.g, Some(&m), &c1).await.unwrap();
    assert_eq!(judged.accepted.as_deref(), Some(c1.as_str()));
    assert!(w.g.lock().engine.open(std::slice::from_ref(&c1)).is_empty());
}

#[tokio::test]
async fn refused_and_waiting_commits_are_named() {
    let w = world().await;
    let mut a = w.base.clone();
    a.insert("Shared/x.txt".into(), pointer(&w.git, 1).await);
    let stranger = commit(&w.git, &a, &[&w.genesis], 60, 7).await;
    let judged = verify::judge(&w.g, Some(&w.genesis), &stranger)
        .await
        .unwrap();
    let (c, v) = judged.stopped.unwrap();
    assert_eq!(c, stranger);
    assert_eq!(verify::code(&v), "commit-unknown-signer");
    let mut bad = w.base.clone();
    bad.insert("Shared/CON.txt".into(), pointer(&w.git, 2).await);
    let named = commit(&w.git, &bad, &[&w.genesis], 70, 1).await;
    let judged = verify::judge(&w.g, Some(&w.genesis), &named).await.unwrap();
    assert_eq!(verify::code(&judged.stopped.unwrap().1), "commit-malformed");
    // Two sides whose rosters differ: a merge holding another roster than
    // their fold is refused (04 §4.8).
    let mut r3 = w.base.clone();
    let text = roster_file::render(&RosterDoc { roster: roster(3) }).unwrap();
    r3.insert(ROSTER.into(), blob(&w.git, text.as_bytes()).await);
    let k3 = commit(&w.git, &r3, &[&w.genesis], 80, 1).await;
    let mut e = w.base.clone();
    e.insert("Shared/e.txt".into(), pointer(&w.git, 5).await);
    let e1 = commit(&w.git, &e, &[&w.genesis], 90, 2).await;
    let mut both = e.clone();
    both.insert("Shared/e.txt".into(), pointer(&w.git, 5).await);
    let fork = commit(&w.git, &both, &[&k3, &e1], 100, 1).await;
    let judged = verify::judge(&w.g, Some(&w.genesis), &fork).await.unwrap();
    let (c, v) = judged.stopped.unwrap();
    assert_eq!((c, verify::code(&v)), (fork.clone(), "commit-malformed"));
    assert!(w.g.lock().engine.accepted(&k3) && w.g.lock().engine.accepted(&e1));
    // The merge holding the fold is accepted.
    let mut folded = r3.clone();
    folded.insert("Shared/e.txt".into(), pointer(&w.git, 5).await);
    let good = commit(&w.git, &folded, &[&k3, &e1], 110, 2).await;
    let judged = verify::judge(&w.g, Some(&w.genesis), &good).await.unwrap();
    assert_eq!(
        judged.accepted.as_deref(),
        Some(good.as_str()),
        "{judged:?}"
    );
}

/// Ana raises K on one side while Ben adds his second laptop on the other
/// (04 §4.8, ADR 0051): the custodian merges the forks, and the merge's
/// roster is their fold, both changes applied, judged as a clean merge;
/// the next change is judged under that roster.
#[tokio::test]
async fn roster_forks_merge_by_the_fold() {
    let w = world().await;
    pigeond::git::set_signer(key(3));
    let with = |r: &Roster| roster_file::render(&RosterDoc { roster: r.clone() }).unwrap();
    let mut a = w.base.clone();
    a.insert(
        ROSTER.into(),
        blob(&w.git, with(&roster(3)).as_bytes()).await,
    );
    let k3 = commit(&w.git, &a, &[&w.genesis], 60, 1).await;
    let mut more = roster(2);
    more.devices.insert(
        dev(4),
        Device {
            person: PersonId("ben".into()),
            name: "ben's device 4".into(),
            role: Role::Laptop,
            annex_uuid: Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e04".into()),
            maxsize: None,
        },
    );
    let mut b = w.base.clone();
    b.insert(ROSTER.into(), blob(&w.git, with(&more).as_bytes()).await);
    let d4 = commit(&w.git, &b, &[&w.genesis], 120, 2).await;
    let judged = verify::judge(&w.g, Some(&k3), &d4).await.unwrap();
    assert_eq!(judged.accepted.as_deref(), Some(d4.as_str()), "{judged:?}");
    let m = merge::merge(&w.g, &k3, &d4)
        .await
        .unwrap()
        .expect("forks always merge");
    let tree = w.git.tree(&m).await.unwrap();
    let text = w.git.object(&tree[ROSTER].oid).await.unwrap();
    let r = roster_file::parse(&String::from_utf8_lossy(&text))
        .unwrap()
        .roster;
    assert_eq!(r.k, 3, "Ana's change is in the fold");
    assert!(
        r.devices.contains_key(&dev(4)),
        "Ben's change is in the fold"
    );
    // Ben's new laptop signs under the merged roster.
    let mut c = tree.clone();
    c.insert("Shared/c.txt".into(), pointer(&w.git, 6).await);
    let c1 = commit(&w.git, &c, &[&m], 180, 4).await;
    let judged = verify::judge(&w.g, Some(&m), &c1).await.unwrap();
    assert_eq!(judged.accepted.as_deref(), Some(c1.as_str()), "{judged:?}");
}

#[tokio::test]
async fn a_rename_and_a_concurrent_edit_merge_into_the_renamed_file() {
    let w = world().await;
    pigeond::git::set_signer(key(3));
    let mut a = w.base.clone();
    a.insert("Shared/a.txt".into(), pointer(&w.git, 1).await);
    let a1 = commit(&w.git, &a, &[&w.genesis], 60, 1).await;
    // Ana renames a.txt to b.txt; Ben, who saw a.txt, edits it.
    let mut r = a.clone();
    let v = r.remove("Shared/a.txt").unwrap();
    r.insert("Shared/b.txt".into(), v);
    let renamed = [("Shared/a.txt".to_owned(), "Shared/b.txt".to_owned())];
    let message = pigeond::renames::message(&renamed);
    let a2 = commit_with(&w.git, &r, &[&a1], (120, 1), &message).await;
    let mut e = a.clone();
    let edit = pointer(&w.git, 2).await;
    e.insert("Shared/a.txt".into(), edit.clone());
    let b2 = commit(&w.git, &e, &[&a1], 180, 2).await;
    let judged = verify::judge(&w.g, Some(&a2), &b2).await.unwrap();
    assert_eq!(judged.accepted.as_deref(), Some(b2.as_str()), "{judged:?}");
    let m = merge::merge(&w.g, &a2, &b2)
        .await
        .unwrap()
        .expect("a clean merge");
    let tree = w.git.tree(&m).await.unwrap();
    let paths: Vec<&str> = tree.keys().map(String::as_str).collect();
    assert_eq!(paths, [".pigeon/roster.toml", "Shared/b.txt"]);
    assert_eq!(tree["Shared/b.txt"], edit, "the edit follows the rename");
    assert!(w.g.lock().engine.open(std::slice::from_ref(&m)).is_empty());
    // Without the trailer, the rename is a deletion and an add: both stay.
    let a3 = commit(&w.git, &r, &[&a1], 240, 1).await;
    let judged = verify::judge(&w.g, Some(&a3), &b2).await.unwrap();
    assert_eq!(judged.accepted.as_deref(), Some(b2.as_str()), "{judged:?}");
    let m = merge::merge(&w.g, &a3, &b2).await.unwrap().unwrap();
    let tree = w.git.tree(&m).await.unwrap();
    assert_eq!(
        (&tree["Shared/a.txt"], &tree["Shared/b.txt"]),
        (&edit, &a["Shared/a.txt"])
    );
}

#[tokio::test]
async fn joining_checks_authority_before_adopting_any_history() {
    let w = world().await;
    let valid = commit(&w.git, &w.base, &[&w.genesis], 1, 1).await;
    verify::bootstrap(&w.g, &w.genesis, &valid, &w.genesis, &dev(2).0)
        .await
        .unwrap();
    let unsigned = w
        .git
        .run_in(
            &[
                "commit-tree",
                &merge::write_tree(&w.git, &w.base).await.unwrap(),
                "-p",
                &w.genesis,
            ],
            b"unsigned history\n",
        )
        .await
        .unwrap()
        .trim()
        .to_owned();
    let stranger = commit(&w.git, &w.base, &[&w.genesis], 2, 7).await;
    let raw = String::from_utf8(w.git.object(&valid).await.unwrap()).unwrap();
    let forged = raw.replace("author pigeon", "author forged");
    let forged = w
        .git
        .run_in(
            &["hash-object", "-t", "commit", "-w", "--stdin"],
            forged.as_bytes(),
        )
        .await
        .unwrap()
        .trim()
        .to_owned();
    let mut bad_tree = w.base.clone();
    bad_tree.insert("Shared/CON.txt".into(), pointer(&w.git, 2).await);
    let bad_path = commit(&w.git, &bad_tree, &[&w.genesis], 3, 1).await;
    for tip in [&unsigned, &stranger, &forged, &bad_path] {
        assert_eq!(
            verify::bootstrap(&w.g, &w.genesis, tip, &w.genesis, &dev(2).0).await,
            Err("join-history-unverified".into())
        );
        assert_eq!(
            w.git.rev("refs/heads/main").await.as_deref(),
            Some(w.genesis.as_str())
        );
    }
    assert_eq!(
        verify::bootstrap(&w.g, &valid, &valid, &w.genesis, &dev(2).0).await,
        Err("invite-unknown".into())
    );
    let mut removed = roster(2);
    let device = removed.devices.remove(&dev(2)).unwrap();
    removed.removed.insert(dev(2), device.annex_uuid);
    let mut tree = w.base.clone();
    let text = roster_file::render(&RosterDoc { roster: removed }).unwrap();
    tree.insert(ROSTER.into(), blob(&w.git, text.as_bytes()).await);
    let revoked = commit(&w.git, &tree, &[&w.genesis], 4, 1).await;
    assert_eq!(
        verify::bootstrap(&w.g, &w.genesis, &revoked, &w.genesis, &dev(2).0).await,
        Err("invite-not-allowed".into())
    );
}

#[tokio::test]
async fn a_decision_waits_for_its_signed_proposal_and_can_be_judged_again() {
    use pigeon_core::authority::{Decided, Verdict, decision_path};
    use pigeond::decision_file::{DecisionDoc, render};
    let mut roster = roster(2);
    roster.folders[0].rule = Rule::AddOnly([PersonId("ana".into())].into());
    let w = world_with_roster(roster).await;
    let mut proposed = w.base.clone();
    proposed.insert("Shared/a.txt".into(), pointer(&w.git, 2).await);
    let proposal = commit(&w.git, &proposed, &[&w.genesis], 30, 2).await;
    let mut tree = w.base.clone();
    tree.insert(
        decision_path(&proposal),
        blob(
            &w.git,
            render(&DecisionDoc {
                proposal: proposal.clone(),
                verdict: Decided::Rejected,
                note: None,
            })
            .as_bytes(),
        )
        .await,
    );
    let decision = commit_with(
        &w.git,
        &tree,
        &[&w.genesis],
        (60, 1),
        "pigeon: decline a suggestion",
    )
    .await;
    let raw_proposal = w.git.object(&proposal).await.unwrap();
    let loose =
        w.g.git_dir
            .join("objects")
            .join(&proposal[..2])
            .join(&proposal[2..]);
    std::fs::remove_file(&loose).unwrap();
    let first = verify::judge(&w.g, None, &decision).await.unwrap();
    assert_eq!(first.accepted.as_deref(), Some(w.genesis.as_str()));
    assert!(matches!(
        first.stopped,
        Some((_, Verdict::Unknown("decision")))
    ));
    assert!(w.g.lock().engine.outcome(&decision).is_none());
    let restored = w
        .git
        .run_in(
            &["hash-object", "-t", "commit", "-w", "--stdin"],
            &raw_proposal,
        )
        .await
        .unwrap();
    assert_eq!(restored.trim(), proposal);
    // No live proposal ref: signed object proof also works for archives.
    let second = verify::judge(&w.g, None, &decision).await.unwrap();
    assert_eq!(second.accepted.as_deref(), Some(decision.as_str()));
    assert!(second.stopped.is_none());
    assert!(!w.g.lock().verdicts.contains_key(&decision));
}

#[tokio::test]
async fn a_decision_cannot_treat_an_authorized_or_unknown_signer_commit_as_a_proposal() {
    use pigeon_core::authority::{Decided, decision_path};
    use pigeond::decision_file::{DecisionDoc, render};
    for signer in [1, 99] {
        let mut r = roster(2);
        r.folders[0].rule = Rule::AddOnly([PersonId("ana".into())].into());
        let w = world_with_roster(r).await;
        let mut proposed = w.base.clone();
        proposed.insert("Shared/a.txt".into(), pointer(&w.git, 7).await);
        let proposal = commit(&w.git, &proposed, &[&w.genesis], 30, signer).await;
        let mut tree = w.base.clone();
        let doc = render(&DecisionDoc {
            proposal: proposal.clone(),
            verdict: Decided::Rejected,
            note: None,
        });
        tree.insert(decision_path(&proposal), blob(&w.git, doc.as_bytes()).await);
        let decision = commit(&w.git, &tree, &[&w.genesis], 60, 1).await;
        let judged = verify::judge(&w.g, None, &decision).await.unwrap();
        assert_ne!(judged.accepted.as_deref(), Some(decision.as_str()));
        assert!(!w.g.lock().engine.accepted(&decision));
    }
}

#[tokio::test]
async fn peer_metadata_cannot_be_admitted_as_an_ordinary_file_suggestion() {
    let w = world().await;
    let mut tree = w.base.clone();
    tree.insert(
        ".pigeon/unrecognized.txt".into(),
        blob(&w.git, b"unapproved metadata").await,
    );
    let proposal = commit(&w.git, &tree, &[&w.genesis], 30, 2).await;
    let incoming = format!(
        "refs/pigeon/peers/fixture/proposals/{}/{proposal}",
        dev(2).0
    );
    w.git.update_ref(&incoming, &proposal, None).await.unwrap();
    assert_eq!(pigeond::proposal::take(&w.g, "fixture").await, 0);
    assert!(pigeond::proposal::kept(&w.git).await.is_empty());
}

#[tokio::test]
async fn a_purge_request_is_admin_only_and_late_approval_extends_only_its_wait() {
    use pigeond::purge_file::{PurgeDoc, render};
    let mut w = world().await;
    let head = purge_upgrade::upgrade(&w.g).await;
    w.base = w.git.tree(&head).await.unwrap();
    let path = ".pigeon/decisions/purge-fixture.toml";
    let doc = PurgeDoc {
        format: 1,
        kind: "purge".into(),
        history: true,
        paths: vec!["Shared/".into()],
        base: head.clone(),
        keys: vec![],
        proposals: vec![],
        requested_by: "ben".into(),
        retention_end: pigeond::report_file::datetime(T0 + 30 + 30 * 86_400),
    };
    let mut tree = w.base.clone();
    tree.insert(
        path.into(),
        blob(&w.git, render(&doc).unwrap().as_bytes()).await,
    );
    let proposal = commit(&w.git, &tree, &[&head], 30, 2).await;
    let incoming = format!(
        "refs/pigeon/peers/fixture/proposals/{}/{proposal}",
        dev(2).0
    );
    w.git.update_ref(&incoming, &proposal, None).await.unwrap();
    assert_eq!(pigeond::proposal::take(&w.g, "fixture").await, 1);
    let pending = pigeond::decide::pending(&w.g).await.unwrap();
    assert_eq!(pending[0].deciders.0, [PersonId("ana".into())].into());
    let mut approved = doc.clone();
    approved.retention_end = pigeond::report_file::datetime(T0 + 1_000 + 30 * 86_400);
    tree.insert(
        path.into(),
        blob(&w.git, render(&approved).unwrap().as_bytes()).await,
    );
    let accepted = commit(&w.git, &tree, &[&head, &proposal], 1_000, 1).await;
    let result = verify::judge(&w.g, None, &accepted).await.unwrap();
    assert_eq!(
        result.accepted.as_deref(),
        Some(accepted.as_str()),
        "{result:?}"
    );
    // Extending the wait confers no right to extend the erased scope.
    approved.paths.push("Unrequested/".into());
    tree.insert(
        path.into(),
        blob(&w.git, render(&approved).unwrap().as_bytes()).await,
    );
    let broadened = commit(&w.git, &tree, &[&head, &proposal], 1_000, 1).await;
    let result = verify::judge(&w.g, None, &broadened).await.unwrap();
    assert_ne!(result.accepted.as_deref(), Some(broadened.as_str()));
    // Original P4 still applies to a direct request and to an approval.
    approved = doc;
    approved.retention_end = pigeond::report_file::datetime(T0 + 1_000 + 30 * 86_400 - 1);
    tree.insert(
        path.into(),
        blob(&w.git, render(&approved).unwrap().as_bytes()).await,
    );
    let shortened = commit(&w.git, &tree, &[&head, &proposal], 1_000, 1).await;
    let result = verify::judge(&w.g, None, &shortened).await.unwrap();
    assert_ne!(result.accepted.as_deref(), Some(shortened.as_str()));
}

#[tokio::test]
async fn cancelling_and_readding_the_same_purge_creates_a_new_identity() {
    use pigeond::purge_file::{PurgeDoc, render};
    let mut w = world().await;
    let head = purge_upgrade::upgrade(&w.g).await;
    w.base = w.git.tree(&head).await.unwrap();
    let path = ".pigeon/decisions/purge-identity.toml";
    let doc = PurgeDoc {
        format: 1,
        kind: "purge".into(),
        history: true,
        paths: vec!["Shared/".into()],
        base: head.clone(),
        keys: vec![],
        proposals: vec![],
        requested_by: "ana".into(),
        retention_end: pigeond::report_file::datetime(T0 + 30 * 86_400 + 1_000),
    };
    let mut tree = w.base.clone();
    tree.insert(
        path.into(),
        blob(&w.git, render(&doc).unwrap().as_bytes()).await,
    );
    let first = commit(&w.git, &tree, &[&head], 30, 1).await;
    let active = pigeond::purge_state::active(&w.g, &first).await.unwrap();
    let original_id = active.first_key_value().unwrap().0.clone();
    assert_eq!(active[&original_id].introduced, first);
    assert_eq!(active[&original_id].retention_seconds, 30 * 86_400);
    let cancelled = commit(&w.git, &w.base, &[&first], 60, 1).await;
    assert!(
        pigeond::purge_state::active(&w.g, &cancelled)
            .await
            .unwrap()
            .is_empty()
    );
    let readded = commit(&w.git, &tree, &[&cancelled], 90, 1).await;
    let active = pigeond::purge_state::active(&w.g, &readded).await.unwrap();
    assert_ne!(active.first_key_value().unwrap().0, &original_id);
    assert_eq!(active.first_key_value().unwrap().1.introduced, readded);
}

#[tokio::test]
async fn a_signed_seal_survives_replaced_reports_and_contradictions_stop_release() {
    use pigeond::purge_file::{PurgeDoc, render};
    let mut w = world().await;
    let head = purge_upgrade::upgrade(&w.g).await;
    w.base = w.git.tree(&head).await.unwrap();
    let path = ".pigeon/decisions/purge-seal.toml";
    let doc = PurgeDoc {
        format: 1,
        kind: "purge".into(),
        history: true,
        paths: vec!["Shared/".into()],
        base: head.clone(),
        keys: vec![],
        proposals: vec![],
        requested_by: "ana".into(),
        retention_end: pigeond::report_file::datetime(T0 - 30 * 86_400 + 1_000),
    };
    let mut tree = w.base.clone();
    tree.insert(
        path.into(),
        blob(&w.git, render(&doc).unwrap().as_bytes()).await,
    );
    // Historical signed timestamps exercise admission, not a real month
    // of waiting; a skewed parent clock cannot shorten receiver uptime.
    let first = commit(&w.git, &tree, &[&head], -60 * 86_400 + 30, 1).await;
    let active = pigeond::purge_state::active(&w.g, &first).await.unwrap();
    let id = active.first_key_value().unwrap().0.clone();
    let mut report = pigeond::report::build(&w.g, &dev(3).0).await;
    report.at = pigeond::report_file::datetime(doc.end().unwrap() + 40);
    report.custody = Some(pigeond::report_file::Custody {
        repository: "ok".into(),
    });
    report.purge.push(pigeond::report_file::Purge {
        decision: id.clone(),
        main: first,
        held: vec![],
    });
    let mut report_tree = Tree::new();
    report_tree.insert(
        "status.toml".into(),
        blob(
            &w.git,
            pigeond::report_file::render(&report).unwrap().as_bytes(),
        )
        .await,
    );
    let status_ref = format!("refs/pigeon/status/{}", dev(3).0);
    let mut early = report.clone();
    early.at = pigeond::report_file::datetime(doc.end().unwrap() - 1);
    let early_tree = [(
        "status.toml".into(),
        blob(
            &w.git,
            pigeond::report_file::render(&early).unwrap().as_bytes(),
        )
        .await,
    )]
    .into();
    let early = commit(&w.git, &early_tree, &[], 39, 3).await;
    w.git.update_ref(&status_ref, &early, None).await.unwrap();
    assert!(
        pigeond::purge_proofs::collect(&w.g, &active)
            .await
            .unwrap()
            .is_empty(),
        "a provably early signed seal cannot count"
    );
    let witness = commit(&w.git, &report_tree, &[], 40, 3).await;
    w.git
        .update_ref(&status_ref, &witness, Some(&early))
        .await
        .unwrap();
    let proofs = pigeond::purge_proofs::collect(&w.g, &active).await.unwrap();
    assert_eq!(proofs.len(), 1);
    let proof_ref = format!("refs/pigeon/purge-seals/{id}/{}", dev(3).0);
    assert_eq!(
        w.git.rev(&proof_ref).await.as_deref(),
        Some(witness.as_str())
    );
    // A completion witness uses all ORIGINAL raw signed reports and an
    // accepted head containing every sealer's history, never today's roster.
    let epoch = pigeond::purge_epoch::Doc {
        format: 1,
        kind: "purge-epoch".into(),
        decision: id.clone(),
        main: report.purge[0].main.clone(),
        reports: [(dev(3).0.clone(), witness.clone())].into(),
    };
    let verified = pigeond::purge_epoch::verify(&w.g, &epoch, std::slice::from_ref(&epoch.main))
        .await
        .unwrap();
    assert_eq!(verified.epoch.custodians, [dev(3).0.clone()].into());
    assert_eq!(verified.seals, proofs);
    pigeond::purge_epoch::retain(&w.g, &verified).await.unwrap();
    let rooted = format!("{}{}", pigeond::purge_epoch::PROOFS, witness);
    assert_eq!(
        w.g.git.rev(&rooted).await.as_deref(),
        Some(witness.as_str())
    );
    assert!(
        pigeond::purge_epoch::verify(&w.g, &epoch, std::slice::from_ref(&head))
            .await
            .is_err()
    );
    let mut forged = epoch.clone();
    forged.reports.insert(dev(2).0, witness.clone());
    assert!(
        pigeond::purge_epoch::verify(&w.g, &forged, std::slice::from_ref(&epoch.main))
            .await
            .is_err()
    );
    forged = epoch.clone();
    forged.reports.insert(dev(3).0, head.clone());
    assert!(
        pigeond::purge_epoch::verify(&w.g, &forged, std::slice::from_ref(&epoch.main))
            .await
            .is_err()
    );
    // A restarted device's empty latest report cannot revoke a signed ack.
    let old = report.purge.clone();
    report.purge.clear();
    report_tree.insert(
        "status.toml".into(),
        blob(
            &w.git,
            pigeond::report_file::render(&report).unwrap().as_bytes(),
        )
        .await,
    );
    let empty = commit(&w.git, &report_tree, &[], 50, 3).await;
    w.git
        .update_ref(&status_ref, &empty, Some(&witness))
        .await
        .unwrap();
    assert_eq!(
        pigeond::purge_proofs::collect(&w.g, &active).await.unwrap(),
        proofs
    );
    report.purge = old;
    report.purge[0].held.push("a-different-held-key".into());
    report_tree.insert(
        "status.toml".into(),
        blob(
            &w.git,
            pigeond::report_file::render(&report).unwrap().as_bytes(),
        )
        .await,
    );
    let contradictory = commit(&w.git, &report_tree, &[], 60, 3).await;
    w.git
        .update_ref(&status_ref, &contradictory, Some(&empty))
        .await
        .unwrap();
    assert!(pigeond::purge_proofs::collect(&w.g, &active).await.is_err());
    assert_eq!(
        w.git.rev(&proof_ref).await.as_deref(),
        Some(witness.as_str())
    );
}

#[path = "purge/driver.rs"]
mod purge_driver;

#[path = "purge/admission.rs"]
mod purge_admission;

#[path = "purge/upgrade.rs"]
mod purge_upgrade;

#[path = "purge/suggestion.rs"]
mod purge_suggestion;

#[path = "purge/preview.rs"]
mod erase_preview;
