//! Folder configuration laws (04 §4.3), 1000 cases each: authority,
//! exact scope, idempotence, portable identity and person validity.
#![allow(clippy::unwrap_used, reason = "law fixtures")]
mod common;
use common::roster_sim::{did, genesis, pid};
use hegel::{TestCase, generators as gs};
use pigeon_core::{
    folder_rules::{Refusal, configure},
    roster::{Folder, Role, Rule, apply, diff},
};

fn rule(tc: &TestCase) -> Option<Rule> {
    match tc.draw(gs::integers::<u8>().max_value(2)) {
        0 => None,
        1 => Some(Rule::Owned(pid(
            tc.draw(gs::integers::<usize>().min_value(1).max_value(3))
        ))),
        _ => Some(Rule::AddOnly(
            (1..=3)
                .filter(|_| tc.draw(gs::booleans()))
                .map(pid)
                .collect(),
        )),
    }
}

#[hegel::test(test_cases = 1000)]
fn only_current_admin_computers_can_configure(tc: TestCase) {
    let mut roster = genesis();
    assert_eq!(
        configure(&roster, &did(3), "Photos", rule(&tc)),
        Err(Refusal::Authority)
    );
    assert_eq!(
        configure(&roster, &did(5), "Photos", rule(&tc)),
        Err(Refusal::Authority)
    );
    roster.devices.get_mut(&did(1)).unwrap().role = Role::Recovery;
    roster.devices.get_mut(&did(1)).unwrap().annex_uuid = None;
    assert_eq!(
        configure(&roster, &did(1), "Photos", rule(&tc)),
        Err(Refusal::Authority)
    );
}

#[hegel::test(test_cases = 1000)]
fn a_rule_changes_no_other_roster_state_or_folder(tc: TestCase) {
    let mut old = genesis();
    old.folders = vec![Folder {
        path: "Other".into(),
        rule: Rule::Owned(pid(3)),
    }];
    let new = configure(&old, &did(1), "Photos", rule(&tc)).unwrap();
    assert_eq!(new.folders.first(), old.folders.first());
    let mut without = new.clone();
    without.folders = old.folders.clone();
    assert_eq!(without, old);
    assert_eq!(apply(&old, &diff(&old, &new).unwrap()).roster, new);
}

#[hegel::test(test_cases = 1000)]
fn repeating_a_choice_is_idempotent(tc: TestCase) {
    let choice = rule(&tc);
    let first = configure(&genesis(), &did(1), "Photos", choice.clone()).unwrap();
    assert_eq!(configure(&first, &did(1), "Photos", choice).unwrap(), first);
}

#[hegel::test(test_cases = 1000)]
fn portable_aliases_do_not_move_legacy_scopes_and_reset_inherits(tc: TestCase) {
    let mut old = genesis();
    old.folders.push(Folder {
        path: "Photos".into(),
        rule: Rule::Owned(pid(3)),
    });
    let choice = rule(&tc);
    assert_eq!(
        configure(&old, &did(1), "PHOTOS", choice.clone()),
        Err(Refusal::Path)
    );
    let new = configure(&old, &did(1), "Photos", choice.clone()).unwrap();
    assert_eq!(new.folders.len(), usize::from(choice.is_some()));
    if let Some(choice) = choice {
        assert_eq!(new.folders[0].rule, choice);
    }
    assert!(
        configure(&new, &did(1), "Photos", None)
            .unwrap()
            .folders
            .is_empty()
    );
}

#[hegel::test(test_cases = 1000)]
fn malformed_paths_and_unknown_people_never_become_rules(tc: TestCase) {
    let old = genesis();
    let paths = [
        "",
        "../Photos",
        "/Photos",
        ".pigeon/data",
        ".GIT",
        "bad\\path",
        ".gitattributes",
    ];
    let path = paths[tc.draw(gs::integers::<usize>().max_value(paths.len() - 1))];
    assert_eq!(
        configure(&old, &did(1), path, rule(&tc)),
        Err(Refusal::Path)
    );
    let absent = pid(tc.draw(gs::integers::<usize>().min_value(4).max_value(1000)));
    assert_eq!(
        configure(&old, &did(1), "Photos", Some(Rule::Owned(absent))),
        Err(Refusal::Person)
    );
}
