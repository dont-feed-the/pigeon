//! Laws of the authority rule for one accepted parent (04 §4.3, §4.4,
//! §4.6, §4.7; pigeon-core `authority`), against an oracle written from
//! 04 §4.3's tables, and of `roster::diff`.
//!
//! The group: p1 an admin, p2 the owner of `F`, p3 a member and the steward
//! of `S`; devices 1-3 are theirs, device 4 is outside the roster, device 5
//! is p1's recovery key. Other paths are add-only with the admins as
//! stewards. The base's files were written by persons drawn at random: one
//! who alone wrote an add-only file may change it (ADR 0070). A rename's
//! trailer gives back its two paths (ADR 0072).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{flag, index, int, pick};
use hegel::TestCase;
use pigeon_core::authority::{self, Entry, Refusal, Step, Tree, Unread, Verdict, Writers};
use pigeon_core::names;
use pigeon_core::roster::{
    self, Device, DeviceId, Edit, Field, Folder, Person, PersonId, Role, Roster, Rule, Value,
};
use pigeon_core::sig::{self, SigError};

fn key(i: usize) -> [u8; 32] {
    [i as u8; 32]
}

fn dev(i: usize) -> DeviceId {
    sig::device_id(&key(i))
}

fn pid(i: usize) -> PersonId {
    PersonId(format!("p{i}"))
}

fn group() -> Roster {
    let laptop = |p: usize| Device {
        person: pid(p),
        name: format!("laptop {p}"),
        role: Role::Laptop,
        annex_uuid: Some(format!("uuid-{p}")),
        maxsize: None,
    };
    let mut devices: BTreeMap<DeviceId, Device> = (1..=3).map(|i| (dev(i), laptop(i))).collect();
    devices.insert(
        dev(5),
        Device {
            person: pid(1),
            name: "paper".into(),
            role: Role::Recovery,
            annex_uuid: None,
            maxsize: None,
        },
    );
    Roster {
        rules: 1,
        group: "g".into(),
        k: 2,
        retention_days: 30,
        telemetry_device: None,
        relay: None,
        persons: (1..=3)
            .map(|i| {
                (
                    pid(i),
                    Person {
                        name: format!("person {i}"),
                        admin: i == 1,
                    },
                )
            })
            .collect(),
        devices,
        removed: BTreeMap::new(),
        folders: vec![
            Folder {
                path: "F".into(),
                rule: Rule::Owned(pid(2)),
            },
            Folder {
                path: "S".into(),
                rule: Rule::AddOnly([pid(3)].into()),
            },
        ],
    }
}

const POINTER: &str = "/annex/objects/SHA256E-s3--2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae.txt\n";

fn entry(oid: &str) -> Entry {
    Entry {
        mode: "100644".into(),
        kind: "blob".into(),
        oid: oid.into(),
    }
}

fn base() -> Tree {
    let mut t: Tree = [".gitattributes", ".pigeon/roster.toml", "F/a", "S/b", "r"]
        .iter()
        .map(|p| ((*p).to_owned(), entry(&format!("old-{p}"))))
        .collect();
    t.insert(old_record(), entry("old-record"));
    t
}

/// A drop record of person 2's laptop, already on `main`.
fn old_record() -> String {
    authority::drop_record_path(&dev(2), "0")
}

fn raw(force: bool) -> Vec<u8> {
    let mut s = String::from(
        "tree 1111111111111111111111111111111111111111\n\
         parent 2222222222222222222222222222222222222222\n\
         author pigeon <pigeon@localhost> 5 +0000\n\
         committer pigeon <pigeon@localhost> 5 +0000\n\npigeon: capture\n",
    );
    if force {
        s.push_str("\nPigeon-Force: fixing a typo\n");
    }
    s.into_bytes()
}

/// Who wrote each file of the base: persons drawn at random.
fn drawn(tc: &TestCase) -> Writers {
    ["F/a", "S/b", "r"]
        .iter()
        .map(|p| {
            let who = (1..=3).filter(|_| flag(tc)).map(pid).collect();
            (names::fold(names::unit(p)), who)
        })
        .collect()
}

/// The oracle: 04 §4.3's tables for one change by device `d`, the base's
/// files written by `w`.
fn table(d: usize, path: &str, added: bool, force: bool, w: &Writers) -> bool {
    let (person, admin, recovery) = match d {
        1 => (1, true, false),
        5 => (1, false, true),
        p => (p, false, false),
    };
    if let Some(rest) = path.strip_prefix(".pigeon/records/") {
        let own = rest == format!("drop-{}-1.toml", dev(d).0);
        return admin || (!recovery && added && own);
    }
    if path.starts_with(".pigeon/") {
        return admin;
    }
    if recovery {
        return false;
    }
    let sole = w
        .get(&names::fold(path))
        .is_some_and(|s| !s.is_empty() && s.iter().all(|p| *p == pid(person)));
    let folders = if path.starts_with("F/") {
        person == 2
    } else if path.starts_with("S/") {
        added || person == 3 || sole
    } else {
        added || admin || sole
    };
    folders || (force && admin)
}

fn judge(
    signer: usize,
    tree: &Tree,
    force: bool,
    blobs: &BTreeMap<String, Vec<u8>>,
    writers: &Writers,
) -> Verdict {
    let r = group();
    let before = base();
    let signed = sig::sign(&raw(force), &key(signer)).unwrap();
    authority::judge(&Step {
        raw: &signed,
        roster: &r,
        before: &before,
        after: tree,
        writers,
        blobs,
        new_roster: None,
        decisions: &BTreeMap::new(),
    })
}

#[hegel::test(test_cases = 1000)]
fn a_change_is_accepted_iff_the_tables_allow_it(tc: TestCase) {
    let signer = int(&tc, 1, 5) as usize;
    let force = flag(&tc);
    // Drop records: the signer's own, another device's, an old one, and a
    // malformed name (08 §8.7).
    let own = authority::drop_record_path(&dev(signer), "1");
    let other = authority::drop_record_path(&dev(if signer == 2 { 3 } else { 2 }), "1");
    let (old, odd) = (
        old_record(),
        format!(".pigeon/records/drop-{}-.toml", dev(signer).0),
    );
    let paths = [
        "F/a",
        "F/new",
        "S/b",
        "S/new",
        "r",
        "new",
        "deep/new",
        ".pigeon/other",
        ".gitattributes",
        own.as_str(),
        other.as_str(),
        old.as_str(),
        odd.as_str(),
    ];
    let w = drawn(&tc);
    let mut tree = base();
    let mut blobs = BTreeMap::new();
    let mut expect = true;
    let mut changed = BTreeSet::new();
    for _ in 0..int(&tc, 1, 3) {
        let p: &str = pick(&tc, &paths);
        if !changed.insert(p) {
            continue;
        }
        let added = !tree.contains_key(p);
        if !added && flag(&tc) {
            tree.remove(p);
        } else {
            let oid = format!("new-{p}");
            tree.insert(p.to_owned(), entry(&oid));
            blobs.insert(oid, POINTER.as_bytes().to_vec());
        }
        expect &= table(signer, p, added, force, &w);
    }
    let verdict = judge(signer, &tree, force, &blobs, &w);
    match verdict {
        Verdict::Accepted(s, None) => {
            assert!(expect, "accepted against the tables");
            assert_eq!(s, dev(signer));
        }
        Verdict::Refused(Refusal::UnknownSigner(s)) => assert_eq!((s, signer), (dev(4), 4)),
        Verdict::Refused(Refusal::NotAllowed(_)) => assert!(!expect && signer != 4),
        other => panic!("{other:?}"),
    }
}

#[hegel::test(test_cases = 1000)]
fn a_proposals_deciders_are_those_who_may_adopt_it(tc: TestCase) {
    let paths = ["F/a", "F/new", "S/b", "S/new", "r", "new", "deep/new"];
    let w = drawn(&tc);
    let mut tree = base();
    let mut blobs = BTreeMap::new();
    let mut changes = Vec::new();
    for _ in 0..int(&tc, 1, 3) {
        let p: &str = pick(&tc, &paths);
        if changes.iter().any(|(q, _)| *q == p) {
            continue;
        }
        let added = !tree.contains_key(p);
        if !added && flag(&tc) {
            tree.remove(p);
        } else {
            let oid = format!("new-{p}");
            tree.insert(p.to_owned(), entry(&oid));
            blobs.insert(oid, POINTER.as_bytes().to_vec());
        }
        changes.push((p, added));
    }
    let (deciders, force) = authority::deciders(&group(), &w, &changes);
    let may = |i: usize| changes.iter().all(|(p, a)| table(i, p, *a, false, &w));
    let expect: BTreeSet<PersonId> = (1..=3).filter(|i| may(*i)).map(pid).collect();
    if force {
        assert!(expect.is_empty(), "{changes:?}");
        assert_eq!(deciders, [pid(1)].into(), "the admins decide");
    } else {
        assert_eq!(deciders, expect, "{changes:?}");
    }
    // `DecidersCanAdopt`: each decider's laptop may sign the adoption, with
    // Force when nobody may make it; nobody else may (04 §4.5).
    for i in 1..=3 {
        let accepted = matches!(judge(i, &tree, force, &blobs, &w), Verdict::Accepted(..));
        assert_eq!(
            accepted,
            deciders.contains(&pid(i)),
            "person {i}: {changes:?}"
        );
    }
}

#[hegel::test(test_cases = 1000)]
fn capture_splits_a_batch_into_an_accepted_commit_and_proposals(tc: TestCase) {
    let signer = int(&tc, 1, 3) as usize;
    let paths = ["F/a", "F/new", "S/b", "S/new", "r", "new", "deep/new"];
    let w = drawn(&tc);
    let before = base();
    let mut changes: BTreeMap<String, Option<Entry>> = BTreeMap::new();
    for _ in 0..int(&tc, 1, 5) {
        let p: &str = pick(&tc, &paths);
        let gone = before.contains_key(p) && flag(&tc);
        changes.insert(p.to_owned(), (!gone).then(|| entry(&format!("new-{p}"))));
    }
    let blobs: BTreeMap<String, Vec<u8>> = changes
        .values()
        .flatten()
        .map(|e| (e.oid.clone(), POINTER.as_bytes().to_vec()))
        .collect();
    let applied = |part: &BTreeMap<String, Option<Entry>>| {
        let mut t = before.clone();
        for (p, e) in part {
            match e {
                Some(e) => t.insert(p.clone(), e.clone()),
                None => t.remove(p),
            };
        }
        t
    };
    let s = authority::split(&group(), &dev(signer), &before, &w, &changes);
    // Nothing is lost, nothing is in two places.
    let mut all: Vec<&String> = s.allowed.keys().collect();
    all.extend(s.proposed.values().flat_map(BTreeMap::keys));
    all.sort_unstable();
    assert_eq!(all, changes.keys().collect::<Vec<_>>());
    // The commit on `main` is accepted; each proposal is refused for folder
    // rules alone, and its deciders are those of each of its changes.
    let v = judge(signer, &applied(&s.allowed), false, &blobs, &w);
    assert!(matches!(v, Verdict::Accepted(..)), "{v:?}");
    for (deciders, part) in &s.proposed {
        assert!(!part.is_empty());
        let v = judge(signer, &applied(part), false, &blobs, &w);
        assert!(
            matches!(v, Verdict::Refused(Refusal::NotAllowed(_))),
            "{v:?}"
        );
        let each: Vec<(&str, bool)> = part
            .keys()
            .map(|p| (p.as_str(), !before.contains_key(p)))
            .collect();
        assert_eq!(&authority::deciders(&group(), &w, &each), deciders);
        for one in &each {
            assert_eq!(&authority::deciders(&group(), &w, &[*one]), deciders);
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn a_decision_is_accepted_iff_its_signer_may_make_it(tc: TestCase) {
    let signer = int(&tc, 1, 5) as usize;
    let proposal = "c0ffee".to_owned();
    let named = if flag(&tc) {
        proposal.clone()
    } else {
        "other".to_owned()
    };
    let path = authority::decision_path(&named);
    let verdict = if flag(&tc) {
        authority::Decided::Rejected
    } else {
        authority::Decided::Withdrawn
    };
    let deciders: BTreeSet<PersonId> = (1..=3).filter(|_| flag(&tc)).map(pid).collect();
    let author = pid(int(&tc, 1, 3) as usize);
    let known = flag(&tc);
    let again = flag(&tc);
    let mut before = base();
    if again {
        before.insert(path.clone(), entry("old-decision"));
    }
    let mut after = before.clone();
    after.insert(path.clone(), entry("decision"));
    let d = authority::Decision {
        proposal,
        verdict,
        about: known.then(|| ((deciders.clone(), false), author.clone())),
    };
    let decisions = [(path.clone(), Ok(d))].into();
    let signed = sig::sign(&raw(false), &key(signer)).unwrap();
    let r = group();
    let v = authority::judge(&Step {
        raw: &signed,
        roster: &r,
        before: &before,
        after: &after,
        writers: &Writers::new(),
        blobs: &BTreeMap::new(),
        new_roster: None,
        decisions: &decisions,
    });
    let person = pid(if signer == 5 { 1 } else { signer });
    let may = match verdict {
        authority::Decided::Rejected => deciders.contains(&person),
        authority::Decided::Withdrawn => person == author,
    };
    match v {
        Verdict::Refused(Refusal::UnknownSigner(_)) => assert_eq!(signer, 4),
        Verdict::Refused(Refusal::NotAllowed(p)) => {
            assert_eq!(p, path);
            assert!(signer == 5 || again || (named == "c0ffee" && known && !may));
        }
        Verdict::Refused(Refusal::Malformed(p, "decision")) => {
            assert!(p == path && named != "c0ffee" && signer != 5 && !again);
        }
        Verdict::Unknown("decision") => assert!(!known && named == "c0ffee" && signer != 5),
        Verdict::Accepted(..) => {
            assert!(may && known && !again && signer != 5 && named == "c0ffee");
        }
        other => panic!("{other:?}"),
    }
}

#[hegel::test(test_cases = 300)]
fn only_files_and_pointers_are_well_formed(tc: TestCase) {
    let p: &str = pick(&tc, &["r", "new", "S/new"]);
    let (mode, kind) = pick(
        &tc,
        &[
            ("100644", "blob"),
            ("100755", "blob"),
            ("120000", "blob"),
            ("160000", "commit"),
        ],
    );
    let content: &str = pick(
        &tc,
        &[POINTER, "hello\n", "/annex/objects/SHA1-s3--abc\n", ""],
    );
    let big = flag(&tc);
    let mut tree = base();
    tree.insert(
        p.to_owned(),
        Entry {
            mode: mode.into(),
            kind: kind.into(),
            oid: "x".into(),
        },
    );
    let blobs: BTreeMap<String, Vec<u8>> = if big {
        BTreeMap::new()
    } else {
        [("x".to_owned(), content.as_bytes().to_vec())].into()
    };
    let good = kind == "blob" && mode != "120000" && !big && content == POINTER;
    match judge(1, &tree, false, &blobs, &Writers::new()) {
        Verdict::Accepted(..) => assert!(good),
        Verdict::Refused(Refusal::Malformed(q, _)) => assert!(!good && q == p),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_only_author_of_an_add_only_file_may_change_it() {
    let blobs = [("n".to_owned(), POINTER.as_bytes().to_vec())].into();
    let mut tree = base();
    tree.insert("r".into(), entry("n"));
    tree.remove("S/b");
    let ok = |signer, who: &[usize]| {
        let set: BTreeSet<PersonId> = who.iter().map(|i| pid(*i)).collect();
        let w: Writers = [("r".to_owned(), set.clone()), ("s/b".to_owned(), set)].into();
        matches!(
            judge(signer, &tree, false, &blobs, &w),
            Verdict::Accepted(..)
        )
    };
    assert!(ok(2, &[2]), "p2 alone wrote both");
    assert!(!ok(2, &[2, 1]), "someone else changed them since");
    assert!(!ok(2, &[3]), "p3's files");
    assert!(!ok(2, &[]), "nobody known");
    assert!(!ok(3, &[1]), "p3 stewards S, not the root");
    let mut owned = base();
    owned.insert("F/a".into(), entry("n"));
    let w: Writers = [("f/a".to_owned(), [pid(3)].into())].into();
    let v = judge(3, &owned, false, &blobs, &w);
    assert!(
        matches!(v, Verdict::Refused(Refusal::NotAllowed(_))),
        "an owned folder stays its owner's: {v:?}"
    );
}

#[test]
fn unsigned_and_forged_commits_are_refused() {
    let r = group();
    let before = base();
    let mut after = base();
    after.insert("new".into(), entry("n"));
    let blobs = [("n".to_owned(), POINTER.as_bytes().to_vec())].into();
    let step = |raw: &[u8]| {
        authority::judge(&Step {
            raw,
            roster: &r,
            before: &before,
            after: &after,
            writers: &Writers::new(),
            blobs: &blobs,
            new_roster: None,
            decisions: &BTreeMap::new(),
        })
    };
    assert_eq!(
        step(&raw(false)),
        Verdict::Refused(Refusal::Signature(SigError::Unsigned))
    );
    let mut forged = sig::sign(&raw(false), &key(1)).unwrap();
    let n = forged.len();
    forged[n - 3] ^= 1;
    assert_eq!(
        step(&forged),
        Verdict::Refused(Refusal::Signature(SigError::Mismatch))
    );
}

/// A roster edit drawn among those 04 §4.7 lets someone make.
fn edit(tc: &TestCase, r: &Roster, fresh: &mut usize) -> Edit {
    let persons: Vec<PersonId> = r.persons.keys().cloned().collect();
    let devices: Vec<DeviceId> = r.devices.keys().cloned().collect();
    let p = pick(tc, &persons);
    match int(tc, 0, 10) {
        0 => Edit::Set {
            field: Field::K,
            from: Value::Number(u64::from(r.k)),
            to: Value::Number(int(tc, 1, 3) as u64),
        },
        1 => Edit::Set {
            field: Field::PersonAdmin(p.clone()),
            from: Value::Flag(r.persons[&p].admin),
            to: Value::Flag(flag(tc)),
        },
        2 if !devices.is_empty() => {
            let d = pick(tc, &devices);
            Edit::Set {
                field: Field::DeviceName(d.clone()),
                from: Value::Text(r.devices[&d].name.clone()),
                to: Value::Text(format!("name {}", int(tc, 0, 9))),
            }
        }
        3 | 4 => {
            *fresh += 1;
            Edit::AddDevice(
                dev(10 + *fresh),
                Device {
                    person: p,
                    name: format!("fresh {fresh}"),
                    role: Role::Custodian,
                    annex_uuid: Some(format!("fresh-{fresh}")),
                    maxsize: flag(tc).then_some(1 << 30),
                },
            )
        }
        5 if !devices.is_empty() => Edit::RemoveDevice(pick(tc, &devices)),
        6 => {
            *fresh += 1;
            Edit::AddPerson(
                PersonId(format!("q{fresh}")),
                Person {
                    name: format!("q {fresh}"),
                    admin: flag(tc),
                },
            )
        }
        7 => Edit::RemovePerson(p),
        8 => Edit::AddFolder(Folder {
            path: format!("D{}", int(tc, 0, 2)),
            rule: Rule::Owned(p),
        }),
        9 => Edit::Set {
            field: Field::Relay,
            from: r.get(&Field::Relay).unwrap_or(Value::Unset),
            // The last is no relay URL: the change is invalid (04 §4.2).
            to: match int(tc, 0, 3) {
                0 => Value::Unset,
                1 => Value::Text("https://relay.example.org".to_owned()),
                2 => Value::Text("http://192.168.1.2:3340".to_owned()),
                _ => Value::Text("relay.example.org".to_owned()),
            },
        },
        _ => Edit::RemoveFolder(format!("D{}", int(tc, 0, 2))),
    }
}

/// A roster reached from the group by edits that apply.
fn walk(tc: &TestCase, from: &Roster, steps: usize, fresh: &mut usize) -> Roster {
    let mut r = from.clone();
    for _ in 0..steps {
        let change: Vec<Edit> = (0..int(tc, 1, 2)).map(|_| edit(tc, &r, fresh)).collect();
        let a = roster::apply(&r, &change);
        if a.skipped.is_none() {
            r = a.roster;
        }
    }
    r
}

#[hegel::test(test_cases = 1000)]
fn diff_is_a_right_inverse_of_apply(tc: TestCase) {
    let mut fresh = 0;
    let old = walk(&tc, &group(), index(&tc, 4), &mut fresh);
    let new = walk(&tc, &old, index(&tc, 5), &mut fresh);
    if let Some(change) = roster::diff(&old, &new) {
        let a = roster::apply(&old, &change);
        assert_eq!(a.skipped, None, "{change:?}");
        assert_eq!(a.roster, new, "{change:?}");
    }
    assert_eq!(roster::diff(&new, &new), Some(Vec::new()));
    // A change of one edit, as pigeond writes them, is always said.
    let one = [edit(&tc, &old, &mut fresh)];
    let a = roster::apply(&old, &one);
    if a.skipped.is_none() {
        let change = roster::diff(&old, &a.roster).expect("one edit is a change");
        assert_eq!(roster::apply(&old, &change).roster, a.roster, "{one:?}");
    }
}

#[hegel::test(test_cases = 1000)]
fn a_roster_change_is_accepted_iff_its_signer_may_make_it(tc: TestCase) {
    let mut fresh = 0;
    let r = group();
    let signer = int(&tc, 1, 5) as usize;
    let change: Vec<Edit> = (0..int(&tc, 1, 2))
        .map(|_| edit(&tc, &r, &mut fresh))
        .collect();
    let applied = roster::apply(&r, &change);
    let new = applied.roster.clone();
    let mut after = base();
    after.insert(".pigeon/roster.toml".into(), entry("roster-new"));
    let before = base();
    let signed = sig::sign(&raw(false), &key(signer)).unwrap();
    let parsed = Ok(new.clone());
    let verdict = authority::judge(&Step {
        raw: &signed,
        roster: &r,
        before: &before,
        after: &after,
        writers: &Writers::new(),
        blobs: &BTreeMap::new(),
        new_roster: Some(&parsed),
        decisions: &BTreeMap::new(),
    });
    let Some(delta) = roster::diff(&r, &new) else {
        // A device added and removed in one change: edits cannot say it.
        assert!(
            matches!(
                verdict,
                Verdict::Refused(Refusal::RosterNotAChange | Refusal::UnknownSigner(_))
            ),
            "{verdict:?} for {change:?}"
        );
        return;
    };
    let may = roster::may_sign(&r, &dev(signer), &delta);
    match verdict {
        Verdict::Accepted(_, Some(n)) => {
            assert!(may && delta == roster::diff(&r, &n).unwrap());
            assert_eq!(n, new);
        }
        Verdict::Accepted(_, None) => panic!("a roster change without R(c)"),
        Verdict::Refused(Refusal::UnknownSigner(_)) => assert_eq!(signer, 4),
        Verdict::Refused(Refusal::NotAllowed(p)) => {
            assert!(!may && p == ".pigeon/roster.toml");
        }
        other => panic!("{other:?} for {change:?}"),
    }
}

#[test]
fn an_invalid_or_unreadable_roster_is_refused_and_an_unknown_one_waits() {
    let r = group();
    let mut after = base();
    after.insert(".pigeon/roster.toml".into(), entry("roster-new"));
    let before = base();
    let signed = sig::sign(&raw(false), &key(1)).unwrap();
    let mut no_admin = r.clone();
    no_admin.persons.get_mut(&pid(1)).unwrap().admin = false;
    for (parsed, want) in [
        (Err(Unread::Malformed("bad toml".to_owned())), "unreadable"),
        (Err(Unread::Unknown("format 2".to_owned())), "unknown"),
        (Ok(no_admin), "invalid"),
    ] {
        let v = authority::judge(&Step {
            raw: &signed,
            roster: &r,
            before: &before,
            after: &after,
            writers: &Writers::new(),
            blobs: &BTreeMap::new(),
            new_roster: Some(&parsed),
            decisions: &BTreeMap::new(),
        });
        let got = match v {
            Verdict::Refused(Refusal::RosterUnreadable(_)) => "unreadable",
            Verdict::Refused(Refusal::RosterInvalid(_)) => "invalid",
            Verdict::Unknown("format") => "unknown",
            other => panic!("{other:?}"),
        };
        assert_eq!(got, want);
    }
}

#[test]
fn trailers_and_author_times_are_read_from_the_commit() {
    let raw = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
        author A <a@x> 1791892920 -0130\n\
        committer A <a@x> 1791892999 +0200\n\n\
        pigeon: keep both\n\nPigeon-Resolved: Shared/a.txt\nPigeon-Resolved:  \n";
    assert_eq!(authority::resolved(raw), vec!["Shared/a.txt".to_owned()]);
    assert_eq!(authority::author_time(raw), Some((1_791_892_920, -90)));
    assert!(!authority::forced(raw));
    let early = b"tree x\nauthor A <a@x> 12 +0000\n\nPigeon-Resolved: a\n\nlater text\n";
    assert!(
        authority::resolved(early).is_empty(),
        "only the last paragraph holds trailers"
    );
    assert_eq!(
        authority::author_time(b"tree x\nauthor A <a@x> 12 +2\n\n"),
        None
    );
}

/// A path of letters, spaces, quotes, backslashes and slashes.
fn odd_path(tc: &TestCase) -> String {
    let chars = ['a', 'é', ' ', '"', '\\', '/', '.', '(', '\''];
    let n = usize::try_from(int(tc, 1, 12)).unwrap();
    (0..n).map(|_| pick(tc, &chars)).collect()
}

#[hegel::test(test_cases = 1000)]
fn a_rename_trailer_names_its_paths(tc: TestCase) {
    let (old, new) = (odd_path(&tc), odd_path(&tc));
    let raw = format!(
        "tree x\nauthor A <a@x> 12 +0000\n\npigeon: capture\n\n{}\n{}\n",
        authority::renamed_trailer(&old, &new),
        authority::renamed_trailer(&new, "b.txt"),
    );
    let got = authority::renamed(raw.as_bytes());
    assert_eq!(
        got,
        vec![(old.clone(), new.clone()), (new, "b.txt".to_owned())]
    );
    let bad = format!("tree x\n\nPigeon-Renamed: \"{old}\n");
    assert!(authority::renamed(bad.as_bytes()).is_empty() || old.contains('"'));
}

/// Selecting and clearing a collector follows the same atomic field and
/// authority rules as other admin-only group settings (ADR 0025).
#[hegel::test(test_cases = 1000)]
fn telemetry_selection_round_trips_and_only_admins_authorize_it(tc: TestCase) {
    let mut old = group();
    old.telemetry_device = flag(&tc).then(|| dev(int(&tc, 1, 3) as usize));
    let mut new = old.clone();
    new.telemetry_device = flag(&tc).then(|| dev(int(&tc, 1, 3) as usize));
    let edits = roster::diff(&old, &new).unwrap();
    let applied = roster::apply(&old, &edits);
    assert_eq!(applied.roster, new);
    assert!(applied.skipped.is_none());
    assert!(roster::may_sign(&old, &dev(1), &edits));
    if !edits.is_empty() {
        assert!(!roster::may_sign(&old, &dev(2), &edits));
        assert!(!roster::may_sign(&old, &dev(5), &edits));
    }
}
