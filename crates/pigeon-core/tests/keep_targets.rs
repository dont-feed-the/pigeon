//! Local folder-choice laws, each checked with 1000 generated cases.
use hegel::{TestCase, generators as gs};
use pigeon_core::keep_targets::Targets;

#[hegel::test(test_cases = 1000)]
fn a_child_choice_overrides_its_parent_and_future_children(tc: TestCase) {
    let parent = tc.draw(gs::booleans());
    let root = format!("Photos-{}", tc.draw(gs::integers::<u32>()));
    let child = format!("{root}/2019");
    let mut targets = Targets::default();
    targets.set(&root, parent);
    targets.set(&child, !parent);
    assert_eq!(targets.is_freed(&format!("{child}/new.jpg")), !parent);
    assert_eq!(targets.is_freed(&format!("{root}/2020/new.jpg")), parent);
    assert!(!targets.is_freed(&format!("{root}-copy/new.jpg")));
}

#[hegel::test(test_cases = 1000)]
fn the_last_parent_request_replaces_older_descendant_choices(tc: TestCase) {
    let freed = tc.draw(gs::booleans());
    let root = format!("Photos-{}", tc.draw(gs::integers::<u32>()));
    let child = format!("{root}/2019");
    let mut targets = Targets::default();
    targets.set(&child, !freed);
    targets.set(&root, freed);
    assert_eq!(targets.is_freed(&format!("{child}/new.jpg")), freed);
    assert_eq!(targets.0.len(), 1);
    let once = targets.clone();
    targets.set(&root, freed);
    assert_eq!(targets, once);
}

#[hegel::test(test_cases = 1000)]
fn unrelated_requests_commute_and_root_requests_cover_everything(tc: TestCase) {
    let a = tc.draw(gs::booleans());
    let b = tc.draw(gs::booleans());
    let notes = format!("Notes-{}", tc.draw(gs::integers::<u32>()));
    let mut left = Targets::default();
    left.set("Photos", a);
    left.set(&notes, b);
    let mut right = Targets::default();
    right.set(&notes, b);
    right.set("Photos", a);
    assert_eq!(left, right);
    left.set("", a);
    assert_eq!(left.is_freed(&format!("{notes}/draft.txt")), a);
    assert_eq!(left.is_freed("root.txt"), a);
}
