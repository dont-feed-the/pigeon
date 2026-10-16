//! Local folder-choice laws, each checked with 1000 generated cases, and
//! the default of other people's folders of a per-person folder.
use hegel::{TestCase, generators as gs};
use pigeon_core::keep_targets::{Targets, others_folders};

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

#[hegel::test(test_cases = 1000)]
fn others_folders_are_freed_unless_a_choice_covers_them(tc: TestCase) {
    let other = "src/laurent".to_owned();
    let mut targets = Targets::default();
    // No choice, a choice of the folder itself, of the folder holding it, of
    // the root, of something inside it, or of something unrelated.
    let scope = match tc.draw(gs::integers::<u8>().max_value(5)) {
        0 => None,
        1 => Some("src/laurent"),
        2 => Some("src"),
        3 => Some(""),
        4 => Some("src/laurent/shots"),
        _ => Some("render"),
    };
    let freed = tc.draw(gs::booleans());
    if let Some(scope) = scope {
        targets.set(scope, freed);
    }
    let effective = targets.clone().with_defaults([other.clone()]);
    let file = "src/laurent/notes.txt";
    let covered = matches!(scope, Some("src/laurent" | "src" | ""));
    assert_eq!(effective.is_freed(file), if covered { freed } else { true });
    if scope == Some("src/laurent/shots") {
        assert_eq!(effective.is_freed("src/laurent/shots/a.png"), freed);
    }
    // The default never touches the person's own choices elsewhere.
    assert_eq!(
        effective.is_freed("render/a.png"),
        targets.is_freed("render/a.png")
    );
    assert_eq!(
        effective.is_freed("src/robin/a.png"),
        targets.is_freed("src/robin/a.png")
    );
}

#[test]
fn only_other_peoples_folders_of_a_per_person_folder_are_defaults() {
    use pigeon_core::roster::{Folder, Person, PersonId, Roster, Rule};
    let person = |name: &str, admin| Person {
        name: name.into(),
        admin,
    };
    let mut roster = Roster {
        rules: 2,
        group: "g".into(),
        invitation: "0".repeat(64),
        k: 1,
        retention_days: 30,
        relay: None,
        telemetry_device: None,
        persons: [
            (PersonId("ana".into()), person("Ana", true)),
            (PersonId("ben".into()), person("Ben", false)),
            (PersonId("cleo".into()), person("Cleo", false)),
        ]
        .into(),
        devices: Default::default(),
        removed: Default::default(),
        folders: vec![
            Folder {
                path: "src".into(),
                rule: Rule::PerPerson,
            },
            Folder {
                path: "Shared".into(),
                rule: Rule::AddOnly(Default::default()),
            },
        ],
    };
    let ana = PersonId("ana".into());
    assert_eq!(others_folders(&roster, &ana), ["src/ben", "src/cleo"]);
    // A folder another rule gives to someone else is not a person's own.
    roster.folders.push(Folder {
        path: "src/cleo".into(),
        rule: Rule::AddOnly(Default::default()),
    });
    assert_eq!(others_folders(&roster, &ana), ["src/ben"]);
    roster.folders.retain(|f| f.rule != Rule::PerPerson);
    assert!(others_folders(&roster, &ana).is_empty());
}
