//! Per-person folders (04 §4.3, rules 2), 1000 cases each: `<folder>/<id>/…`
//! belongs to the person `<id>`; any other path in it, and a more specific
//! rule, are judged as without it; only a roster of rules 2 holds one, and
//! configuring one raises the rules.
#![allow(clippy::unwrap_used, reason = "law fixtures")]
mod common;
use common::roster_sim::{NP, did, genesis, pid};
use hegel::{TestCase, generators as gs};
use pigeon_core::{
    authority::{Writers, allowed, rule_of},
    folder_rules::configure,
    roster::{Folder, Invalid, PersonId, Roster, Rule},
};

fn per_person(at: &str) -> Roster {
    let mut roster = genesis();
    roster.rules = 2;
    roster.folders = vec![Folder {
        path: at.into(),
        rule: Rule::PerPerson,
    }];
    roster
}

fn tail(tc: &TestCase) -> String {
    let depth = tc.draw(gs::integers::<usize>().max_value(3));
    (0..depth)
        .map(|i| format!("d{i}"))
        .chain(std::iter::once("f.txt".to_owned()))
        .collect::<Vec<_>>()
        .join("/")
}

#[hegel::test(test_cases = 1000)]
fn a_persons_folder_is_theirs_alone(tc: TestCase) {
    let roster = per_person("src");
    let owner = tc.draw(gs::integers::<usize>().min_value(1).max_value(NP));
    let other = tc.draw(gs::integers::<usize>().min_value(1).max_value(NP));
    let path = format!("src/{}/{}", pid(owner).0, tail(&tc));
    assert_eq!(rule_of(&roster, &path), Some(Rule::Owned(pid(owner))));
    let added = tc.draw(gs::booleans());
    let none = Writers::new();
    assert_eq!(
        allowed(&roster, &none, &pid(other), &path, added),
        owner == other,
        "{path}"
    );
}

#[hegel::test(test_cases = 1000)]
fn the_rest_of_the_folder_follows_the_rule_above_it(tc: TestCase) {
    let mut roster = per_person("work/src");
    let above = tc.draw(gs::booleans()).then(|| Folder {
        path: "work".into(),
        rule: Rule::AddOnly([pid(3)].into()),
    });
    roster.folders.extend(above.clone());
    let path = if tc.draw(gs::booleans()) {
        format!("work/src/nobody/{}", tail(&tc))
    } else {
        "work/src/notes.txt".to_owned()
    };
    assert_eq!(rule_of(&roster, &path), above.map(|f| f.rule));
}

#[hegel::test(test_cases = 1000)]
fn a_more_specific_rule_wins(tc: TestCase) {
    let mut roster = per_person("src");
    let owner = tc.draw(gs::integers::<usize>().min_value(1).max_value(NP));
    roster.folders.push(Folder {
        path: format!("src/{}/shared", pid(owner).0),
        rule: Rule::AddOnly([pid(1)].into()),
    });
    let path = format!("src/{}/shared/{}", pid(owner).0, tail(&tc));
    assert_eq!(
        rule_of(&roster, &path),
        Some(Rule::AddOnly([pid(1)].into()))
    );
}

#[test]
fn a_folder_named_after_no_current_person_is_nobodys() {
    let mut roster = per_person("src");
    roster.persons.remove(&pid(2));
    roster.devices.retain(|_, d| d.person != pid(2));
    assert_eq!(rule_of(&roster, "src/p2/f.txt"), None);
    // IDs match exactly, as rules 1 match paths.
    assert_eq!(rule_of(&roster, "src/P1/f.txt"), None);
    assert_eq!(
        rule_of(&roster, "src/p1/f.txt"),
        Some(Rule::Owned(PersonId("p1".into())))
    );
}

#[test]
fn only_rules_2_hold_a_per_person_folder() {
    let mut roster = per_person("src");
    assert_eq!(roster.validate(), Ok(()));
    roster.rules = 1;
    assert_eq!(
        roster.validate(),
        Err(Invalid::RuleNeedsRules("src".into()))
    );
}

#[test]
fn configuring_one_raises_the_rules_and_nothing_else() {
    let old = genesis();
    let new = configure(&old, &did(1), "src", Some(Rule::PerPerson)).unwrap();
    assert_eq!(new.rules, 2);
    assert_eq!(new.folders, per_person("src").folders);
    let mut same = new.clone();
    same.rules = old.rules;
    same.folders = old.folders.clone();
    assert_eq!(same, old);
    let again = configure(&new, &did(1), "src", Some(Rule::PerPerson)).unwrap();
    assert_eq!(again, new);
}
