//! The catalogue merge against its oracle (05 §5.2): every commit that
//! `models/catalogue/catalogue.py --mode new` reaches on every schedule of
//! its cases, judged again by [`Engine`], must get the oracle's verdict, and
//! an accepted one the oracle's tree (law `tree`) and open conflicts.
//! `PIGEON_CATALOGUE_ORACLE=DIR` names a `--dump` already written; else the
//! test runs `python3` to write one; `PIGEON_CATALOGUE_CASES=a,b` limits it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

use pigeon_core::catalogue::{Engine, Meta, Outcome};
use pigeon_core::names::{Stamp, fold};
use serde_json::Value;

/// 2026-10-13 00:00 UTC, the oracle's day.
const DAY: i64 = 1_791_849_600;

/// The oracle's folder rules: `team` add-only with three stewards, `alice`
/// owned by Alice, the root add-only with the admins as stewards.
fn may(person: &str, path: &str, added: bool) -> bool {
    let folded = fold(path);
    let parts: Vec<&str> = folded.split('/').collect();
    for i in (1..parts.len()).rev() {
        match parts[..i].join("/").as_str() {
            "team" => return added || ["Alice", "Bob", "Paul"].contains(&person),
            "alice" => return person == "Alice",
            _ => {}
        }
    }
    added || person == "Alice"
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect()
}

fn oracle() -> PathBuf {
    if let Ok(dir) = std::env::var("PIGEON_CATALOGUE_ORACLE") {
        return PathBuf::from(dir);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("pigeon-catalogue-oracle-{}", std::process::id()));
    let mut cmd = Command::new("python3");
    cmd.arg(root.join("models/catalogue/catalogue.py"))
        .args(["--mode", "new", "--dump"])
        .arg(&dir);
    if let Ok(cases) = std::env::var("PIGEON_CATALOGUE_CASES") {
        for c in cases.split(',') {
            cmd.args(["--case", c]);
        }
    }
    let status = cmd.status().expect("python3 runs the oracle");
    assert!(status.success(), "the oracle passes its own laws");
    dir
}

fn replay(name: &str, text: &str) -> usize {
    let mut engine: Engine<String> = Engine::new();
    let mut n = 0;
    for line in text.lines() {
        let rec: Value = serde_json::from_str(line).unwrap();
        let id = rec["id"].as_str().unwrap().to_owned();
        let tree: BTreeMap<String, String> = rec["tree"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(p, k)| (p.clone(), k.as_str().unwrap().to_owned()))
            .collect();
        let meta = Meta {
            id: id.clone(),
            parents: strings(&rec["parents"]),
            device: rec["device"].as_str().unwrap().to_owned(),
            who: rec["who"].as_str().unwrap().to_owned(),
            person: pigeon_core::roster::PersonId(rec["who"].as_str().unwrap().to_owned()),
            renames: rec["renames"]
                .as_array()
                .map(|rs| {
                    rs.iter()
                        .map(|r| {
                            let pair = strings(r);
                            (pair[0].clone(), pair[1].clone())
                        })
                        .collect()
                })
                .unwrap_or_default(),
            when: Stamp {
                utc: DAY + rec["when"].as_i64().unwrap() * 60,
                offset: 0,
            },
            resolves: strings(&rec["resolve"]),
        };
        let person = rec["person"].as_str().unwrap().to_owned();
        let outcome = engine.record(&meta, &tree, &|p, added| may(&person, p, added));
        let accepted = matches!(outcome, Outcome::Clean | Outcome::Contribution);
        let contribution = outcome == Outcome::Contribution;
        assert_eq!(
            (accepted, contribution),
            (
                rec["accepted"].as_bool().unwrap(),
                rec["contribution"].as_bool().unwrap()
            ),
            "{name}: commit {id} ({outcome:?})"
        );
        if accepted {
            let ids = [id.clone()];
            let view = engine.catalogue(&ids);
            assert_eq!(view.tree, tree, "{name}: tree of {id}");
            let open: BTreeSet<String> = strings(&rec["open"]).into_iter().collect();
            assert_eq!(engine.open(&ids), open, "{name}: open conflicts of {id}");
        }
        n += 1;
    }
    n
}

#[test]
fn every_commit_the_oracle_reaches_is_judged_alike() {
    let dir = oracle();
    let mut total = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).unwrap();
        total += replay(&name, &text);
    }
    assert!(total > 0, "the oracle wrote commits");
}
