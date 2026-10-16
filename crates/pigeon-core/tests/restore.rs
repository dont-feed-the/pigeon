//! Snapshot-restore laws (10 §10.1, ADR 0034), 1000 cases each: one
//! moment, inclusive time bounds, exact scope and no invented deletions.
#![allow(clippy::unwrap_used, reason = "property-test fixtures")]
use hegel::{TestCase, generators as gs};
use pigeon_core::restore::{Snapshot, files, select, under};
use std::collections::BTreeMap;

fn history(tc: &TestCase) -> Vec<Snapshot<u32, u32>> {
    (0..tc.draw(gs::integers::<u32>().min_value(1).max_value(15)))
        .rev()
        .map(|i| {
            let files = ["photos/a", "photos/deep/b", "photos-old/a", "other/c"]
                .into_iter()
                .filter(|_| tc.draw(gs::booleans()))
                .map(|path| (path.into(), i))
                .collect();
            Snapshot {
                commit: i,
                at: i64::from(i),
                files,
            }
        })
        .collect()
}

#[hegel::test(test_cases = 1000)]
fn selected_folder_is_one_complete_latest_snapshot(tc: TestCase) {
    let history = history(&tc);
    let selected = select(&history, "photos", None);
    let expected = history
        .iter()
        .position(|s| s.files.keys().any(|p| under(p, "photos")));
    assert_eq!(selected, expected);
    if let Some(i) = selected {
        let restored = files(&history[i], "photos");
        assert!(!restored.is_empty());
        assert!(restored.values().all(|v| *v == history[i].commit));
    }
}

#[hegel::test(test_cases = 1000)]
fn cutoff_is_inclusive_and_never_selects_a_later_save(tc: TestCase) {
    let history = history(&tc);
    let time = tc.draw(gs::integers::<i64>().min_value(-1).max_value(16));
    if let Some(i) = select(&history, "photos", Some(time)) {
        assert!(history[i].at <= time);
        assert!(
            !history[..i]
                .iter()
                .any(|s| s.at <= time && s.files.keys().any(|p| under(p, "photos")))
        );
        assert!(select(&history[i..], "photos", Some(history[i].at)).is_some());
    }
}

#[hegel::test(test_cases = 1000)]
fn sibling_prefixes_and_outside_paths_are_never_restored(tc: TestCase) {
    for snapshot in history(&tc) {
        let restored = files(&snapshot, "photos");
        assert!(!restored.contains_key("photos-old/a"));
        assert!(!restored.contains_key("other/c"));
        assert_eq!(
            restored.len(),
            snapshot
                .files
                .keys()
                .filter(|p| p.starts_with("photos/"))
                .count()
        );
    }
}

#[hegel::test(test_cases = 1000)]
fn applying_restore_keeps_every_unmentioned_newer_file(tc: TestCase) {
    let history = history(&tc);
    let mut working: BTreeMap<String, u32> = [
        ("photos/newer-only".into(), tc.draw(gs::integers::<u32>())),
        ("outside".into(), tc.draw(gs::integers::<u32>())),
    ]
    .into();
    let before = working.clone();
    if let Some(i) = select(&history, "photos", None) {
        working.extend(files(&history[i], "photos"));
    }
    for (path, bytes) in before {
        assert_eq!(working.get(&path), Some(&bytes));
    }
}
