//! The golden files of `fixtures/formats/` (ADR 0048, 13 §13.6). Each
//! roster, status report and clock file reads as its directory says, a known
//! one reads back the same once this release renders it, and each commit case
//! of `commits/` gets its verdict under its parent's roster. The files of
//! `unknown/` are written as a newer release could write them: this release
//! must neither accept nor refuse what holds them, only wait.
//!
//! `PIGEON_WRITE_FIXTURES=1 cargo test -p pigeond --test formats` writes the
//! generated files that are missing (known rosters, reports and clock files,
//! signed commit objects); it never rewrites one, since an older release's
//! file must stay as that release wrote it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use pigeon_core::authority::{self, Entry, ROSTER, Step, Tree, Verdict};
use pigeon_core::roster::{Device, DeviceId, Folder, Person, PersonId, Role, Roster, Rule};
use pigeon_core::sig;
use pigeond::report_file::{self, Custody, Disk, Queue, Scrub, Status, Versions};
use pigeond::roster_file::{self, RosterDoc};
use pigeond::{clock_file, util};
use sha2::{Digest, Sha256};
use toml_edit::{Item, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/formats")
}

fn writing() -> bool {
    std::env::var_os("PIGEON_WRITE_FIXTURES").is_some()
}

/// Writes `path` when writing and it is missing.
fn generate(path: &Path, bytes: &[u8]) {
    if writing() && !path.exists() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}

/// The files of `dir`, sorted, with their text.
fn files(dir: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file())
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(&p).unwrap())
        })
        .collect();
    out.sort();
    assert!(!out.is_empty(), "{} holds no fixture", dir.display());
    out
}

fn key(i: u8) -> [u8; 32] {
    [i; 32]
}

fn dev(i: u8) -> DeviceId {
    sig::device_id(&key(i))
}

fn pid(s: &str) -> PersonId {
    PersonId(s.to_owned())
}

/// Ana (an admin, a laptop and a recovery sheet), Ben (a laptop) and Cleo
/// (a custodian): devices 1, 5, 2 and 3. `Ana` is owned by Ana; `Shared`
/// is add-only, Cleo its steward.
fn group() -> Roster {
    let device = |p: &str, name: &str, role, uuid: Option<&str>, maxsize| Device {
        person: pid(p),
        name: name.to_owned(),
        role,
        annex_uuid: uuid.map(str::to_owned),
        maxsize,
    };
    let person = |name: &str, admin| Person {
        name: name.to_owned(),
        admin,
    };
    Roster {
        rules: 1,
        group: "fixture".into(),
        k: 2,
        retention_days: 30,
        telemetry_device: None,
        relay: None,
        persons: [
            (pid("ana"), person("Ana", true)),
            (pid("ben"), person("Ben", false)),
            (pid("cleo"), person("Cleo", false)),
        ]
        .into(),
        devices: [
            (
                dev(1),
                device(
                    "ana",
                    "Ana's laptop",
                    Role::Laptop,
                    Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e01"),
                    None,
                ),
            ),
            (
                dev(2),
                device(
                    "ben",
                    "Ben's laptop",
                    Role::Laptop,
                    Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e02"),
                    None,
                ),
            ),
            (
                dev(3),
                device(
                    "cleo",
                    "Cleo's box",
                    Role::Custodian,
                    Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e03"),
                    Some(500_000_000_000),
                ),
            ),
            (
                dev(5),
                device("ana", "Ana's recovery sheet", Role::Recovery, None, None),
            ),
        ]
        .into(),
        removed: [(
            dev(9),
            Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e09".to_owned()),
        )]
        .into(),
        folders: vec![
            Folder {
                path: "Ana".into(),
                rule: Rule::Owned(pid("ana")),
            },
            Folder {
                path: "Shared".into(),
                rule: Rule::AddOnly([pid("cleo")].into()),
            },
        ],
    }
}

/// The rosters this release writes, by file name.
fn known_rosters() -> Vec<(&'static str, Roster)> {
    let g = group();
    let mut phone = g.clone();
    phone.devices.insert(
        dev(4),
        Device {
            person: pid("ben"),
            name: "Ben's phone".into(),
            role: Role::Laptop,
            annex_uuid: Some("4f1c0a52-1b7e-4c4e-9a51-0d3b6a1f2e04".into()),
            maxsize: None,
        },
    );
    let mut k3 = g.clone();
    k3.k = 3;
    let mut relay = g.clone();
    relay.relay = Some("https://relay.example.org".into());
    let mut rules2 = g.clone();
    rules2.rules = 2;
    let mut minimal = g.clone();
    minimal.persons.retain(|p, _| p.0 == "ana");
    minimal.devices.retain(|d, _| *d == dev(1));
    minimal.removed.clear();
    minimal.folders.clear();
    minimal.k = 1;
    vec![
        ("group.toml", g),
        ("group-ben-phone.toml", phone),
        ("group-k3.toml", k3),
        ("group-relay.toml", relay),
        ("group-rules-2.toml", rules2),
        ("minimal.toml", minimal),
    ]
}

fn read_roster(text: &str) -> &'static str {
    match roster_file::parse(text) {
        Ok(_) => "known",
        Err(authority::Unread::Unknown(_)) => "unknown",
        Err(authority::Unread::Malformed(_)) => "malformed",
    }
}

#[test]
fn rosters_read_as_their_directory_says() {
    let dir = root().join("roster");
    for (name, r) in known_rosters() {
        let text = roster_file::render(&RosterDoc { roster: r }).unwrap();
        generate(&dir.join("known").join(name), text.as_bytes());
    }
    for class in ["known", "unknown", "malformed"] {
        for (name, text) in files(&dir.join(class)) {
            assert_eq!(read_roster(&text), class, "roster/{class}/{name}");
            if let Ok(doc) = roster_file::parse(&text) {
                let again = roster_file::render(&doc).unwrap();
                assert_eq!(roster_file::parse(&again), Ok(doc), "roster/{class}/{name}");
            }
        }
    }
}

fn report(device: u8, custodian: bool) -> Status {
    Status {
        format: report_file::FORMAT,
        device: dev(device).0,
        at: report_file::datetime(1_791_892_920),
        main: Some("5d41402abc4b2a76b9719d911017c592ae0b4c2e".into()),
        proposals: vec![],
        checking: false,
        lab: false,
        versions: Versions {
            pigeon: "0.1.0".into(),
            git_annex: "10.20261006".into(),
            git: "2.47.0".into(),
        },
        disk: Disk {
            used: 812_000_000,
            free: 41_000_000_000,
            maxsize: if custodian { 500_000_000_000 } else { 0 },
            reserve: 2_000_000_000,
            full: false,
        },
        custody: custodian.then(|| Custody {
            repository: "ok".into(),
        }),
        scrub: custodian.then(|| Scrub {
            last_pass_completed: report_file::datetime(1_791_806_520),
            keys_failed: 0,
        }),
        purge: vec![],
        acknowledgements: Vec::new(),
        queue: Queue {
            bytes_to_send: if custodian { 0 } else { 3_500_000 },
            admits: "all".into(),
            ..Queue::default()
        },
    }
}

fn read_report(text: &str) -> &'static str {
    match report_file::parse(text) {
        report_file::Read::Known(_) => "known",
        report_file::Read::Unknown => "unknown",
        report_file::Read::Malformed => "malformed",
    }
}

#[test]
fn status_reports_read_as_their_directory_says() {
    let dir = root().join("status");
    for (name, s) in [
        ("laptop.toml", report(2, false)),
        ("custodian.toml", report(3, true)),
    ] {
        generate(
            &dir.join("known").join(name),
            report_file::render(&s).unwrap().as_bytes(),
        );
    }
    for class in ["known", "unknown", "malformed"] {
        for (name, text) in files(&dir.join(class)) {
            assert_eq!(read_report(&text), class, "status/{class}/{name}");
            if let report_file::Read::Known(s) = report_file::parse(&text) {
                let again = report_file::render(&s).unwrap();
                assert_eq!(report_file::parse(&again), report_file::Read::Known(s));
            }
        }
    }
}

#[test]
fn clock_files_read_as_their_directory_says() {
    let dir = root().join("clock");
    let c = clock_file::Clock {
        format: clock_file::FORMAT,
        clock: 1207,
        heard: [(dev(2).0, 1_791_892_920), (dev(3).0, 1_791_892_861)].into(),
    };
    let text = serde_json::to_string_pretty(&c).unwrap() + "\n";
    generate(&dir.join("known/clock.json"), text.as_bytes());
    for class in ["known", "unknown", "missing"] {
        for (name, text) in files(&dir.join(class)) {
            let got = match clock_file::parse(&text) {
                clock_file::Read::Known(_) => "known",
                clock_file::Read::Unknown => "unknown",
                clock_file::Read::Missing => "missing",
            };
            assert_eq!(got, class, "clock/{class}/{name}");
        }
    }
}

fn oid(content: &[u8]) -> String {
    util::hex(&Sha256::digest(content))[..40].to_owned()
}

/// A tree entry of a case: a blob's text, or a table with `mode` and
/// `content`, or `roster`, a roster fixture's path.
fn entry(item: &Item, blobs: &mut BTreeMap<String, Vec<u8>>) -> Entry {
    let (mode, content) = match item {
        Item::Value(Value::String(s)) => ("100644".to_owned(), s.value().clone()),
        _ => {
            let t = item.as_table_like().expect("an entry is text or a table");
            let field = |k: &str| t.get(k).and_then(Item::as_str).map(str::to_owned);
            let content = match field("roster") {
                Some(path) => fs::read_to_string(root().join("roster").join(path)).unwrap(),
                None => field("content").expect("content"),
            };
            (field("mode").unwrap_or_else(|| "100644".into()), content)
        }
    };
    let id = oid(content.as_bytes());
    blobs.insert(id.clone(), content.into_bytes());
    Entry {
        mode,
        kind: "blob".into(),
        oid: id,
    }
}

fn tree(t: Option<&Item>, into: &mut Tree, blobs: &mut BTreeMap<String, Vec<u8>>) {
    let Some(t) = t.and_then(Item::as_table_like) else {
        return;
    };
    for (path, item) in t.iter() {
        if item
            .as_table_like()
            .and_then(|x| x.get("deleted"))
            .is_some()
        {
            into.remove(path);
        } else {
            into.insert(path.to_owned(), entry(item, blobs));
        }
    }
}

/// The commit object of a case: signed by `signer` (0: unsigned), then
/// altered when `tamper`.
fn commit(case: &toml_edit::DocumentMut) -> Vec<u8> {
    let message = case["message"].as_str().unwrap();
    let body = format!(
        "tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
         parent 5d41402abc4b2a76b9719d911017c592ae0b4c2e\n\
         author pigeon <pigeon@localhost> 1791892920 +0000\n\
         committer pigeon <pigeon@localhost> 1791892920 +0000\n\n{message}\n"
    );
    let signer = case["signer"].as_integer().unwrap();
    let mut raw = if signer == 0 {
        body.into_bytes()
    } else {
        sig::sign(body.as_bytes(), &key(u8::try_from(signer).unwrap())).unwrap()
    };
    if case.get("tamper").and_then(Item::as_bool) == Some(true) {
        let n = raw.len();
        raw[n - 2] ^= 1;
    }
    raw
}

fn verdict(v: &Verdict) -> String {
    match v {
        Verdict::Accepted(..) => "accepted".into(),
        Verdict::Refused(r) => format!("refused:{}", authority::code(r)),
        Verdict::Unknown(_) => "unknown".into(),
    }
}

#[test]
fn commit_cases_get_their_verdicts() {
    let dir = root().join("commits");
    let mut seen = 0;
    for (name, text) in files(&dir) {
        let Some(stem) = name.strip_suffix(".toml") else {
            continue;
        };
        seen += 1;
        let case: toml_edit::DocumentMut = text.parse().unwrap();
        let path = dir.join(format!("{stem}.commit"));
        generate(&path, &commit(&case));
        let raw = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let parent = case["roster"].as_str().unwrap();
        let roster_text = fs::read_to_string(root().join("roster").join(parent)).unwrap();
        let roster = roster_file::parse(&roster_text).unwrap().roster;
        let mut blobs = BTreeMap::new();
        let mut before = Tree::new();
        let pigeon = toml_edit::value(roster_text.clone());
        before.insert(ROSTER.to_owned(), entry(&pigeon, &mut blobs));
        tree(case.get("before"), &mut before, &mut blobs);
        let mut after = before.clone();
        tree(case.get("after"), &mut after, &mut blobs);
        let new_roster = authority::delta(&before, &after)
            .contains(ROSTER)
            .then(|| after.get(ROSTER))
            .flatten()
            .map(|e| {
                roster_file::parse(&String::from_utf8_lossy(&blobs[&e.oid])).map(|d| d.roster)
            });
        let v = authority::judge(&Step {
            raw: &raw,
            roster: &roster,
            before: &before,
            after: &after,
            writers: &BTreeMap::new(),
            blobs: &blobs,
            new_roster: new_roster.as_ref(),
            decisions: &BTreeMap::new(),
            purges: &BTreeMap::new(),
            witnesses: &BTreeMap::new(),
        });
        let want = if authority::RULES >= 2 {
            case.get("verdict_rules_2")
                .and_then(Item::as_str)
                .unwrap_or_else(|| case["verdict"].as_str().unwrap())
        } else {
            case["verdict"].as_str().unwrap()
        };
        assert_eq!(verdict(&v), want, "commits/{name}: {v:?}");
    }
    assert!(seen > 0, "no commit case");
}
