//! Catalogue unit grouping preserves file/package content and folded-slot
//! ordering (05 §5.1, 07 §7.9), including arbitrary nonportable input trees.
use std::collections::BTreeMap;

use hegel::{TestCase, generators as gs};
use pigeon_core::catalogue::{Content, Tree, Version, expand, units};
use pigeon_core::names::{fold, unit};

// The pre-optimization grouping is an extensional reference: keep its
// literal-unit sort before folded-slot collection, including collisions.
fn original(tree: &Tree<u64>) -> BTreeMap<String, Version<u64>> {
    let mut groups: BTreeMap<&str, BTreeMap<String, u64>> = BTreeMap::new();
    for (path, key) in tree {
        let path_unit = unit(path);
        groups.entry(path_unit).or_default().insert(
            path.get(path_unit.len() + 1..)
                .unwrap_or_default()
                .to_owned(),
            *key,
        );
    }
    groups
        .into_iter()
        .map(|(path, mut files)| {
            let content = match files.remove("") {
                Some(key) if files.is_empty() => Content::File(key),
                _ => Content::Package(files),
            };
            (
                fold(path),
                Version {
                    path: path.to_owned(),
                    content,
                },
            )
        })
        .collect()
}

const SPELLINGS: [&str; 12] = [
    "a", "A", "Straße", "STRASSE", "é", "e\u{301}", "R.pages", "r.PAGES", "x", "x.pages", "", "a.b",
];

#[hegel::test(test_cases = 1000)]
fn grouping_preserves_the_original_function_on_arbitrary_trees(tc: TestCase) {
    let count = tc.draw(gs::integers::<u8>().max_value(80));
    let mut tree = Tree::new();
    for _ in 0..count {
        let depth = tc.draw(gs::integers::<u8>().min_value(1).max_value(5));
        let path = (0..depth)
            .map(|_| {
                let index = tc.draw(gs::integers::<u8>().max_value(11));
                SPELLINGS[usize::from(index)]
            })
            .collect::<Vec<_>>()
            .join("/");
        tree.insert(path, tc.draw(gs::integers::<u64>()));
    }
    assert_eq!(units(&tree), original(&tree));
}

#[hegel::test(test_cases = 1000)]
fn grouping_then_expanding_disjoint_units_keeps_every_leaf(tc: TestCase) {
    let count = tc.draw(gs::integers::<u8>().max_value(80));
    let mut tree = Tree::new();
    for index in 0..count {
        let package = tc.draw(gs::integers::<u8>().max_value(1)) == 1;
        let count = if package {
            tc.draw(gs::integers::<u8>().min_value(1).max_value(8))
        } else {
            1
        };
        for leaf in 0..count {
            let path = if package {
                format!("team/unit-{index}.pages/nested/leaf-{leaf}.bin")
            } else {
                format!("team/file-{index}.bin")
            };
            tree.insert(path, tc.draw(gs::integers::<u64>()));
        }
    }
    let grouped = units(&tree);
    assert_eq!(grouped.len(), usize::from(count));
    assert_eq!(
        expand(grouped.values().map(|v| (v.path.as_str(), &v.content))),
        tree
    );
}

#[test]
fn a_package_root_leaf_is_removed_when_children_exist() {
    let tree = Tree::from([
        ("R.pages".to_owned(), 1),
        ("R.pages/data/x".to_owned(), 2),
        ("R.pages/N.pages/y".to_owned(), 3),
    ]);
    assert_eq!(
        units(&tree),
        BTreeMap::from([(
            "r.pages".to_owned(),
            Version {
                path: "R.pages".to_owned(),
                content: Content::Package(BTreeMap::from([
                    ("data/x".to_owned(), 2),
                    ("N.pages/y".to_owned(), 3),
                ]))
            },
        )])
    );
    assert_eq!(
        units(&Tree::from([("R.pages".to_owned(), 1)]))["r.pages"].content,
        Content::File(1)
    );
}

#[test]
fn folded_collision_winners_follow_literal_unit_order() {
    // STRASSE precedes Straße, and uppercase package precedes lowercase.
    let tree = Tree::from([
        ("STRASSE".to_owned(), 1),
        ("Straße".to_owned(), 2),
        ("R.pages/a".to_owned(), 3),
        ("r.PAGES/z".to_owned(), 4),
    ]);
    let grouped = units(&tree);
    assert_eq!(grouped["strasse"].path, "Straße");
    assert_eq!(grouped["strasse"].content, Content::File(2));
    assert_eq!(grouped["r.pages"].path, "r.PAGES");
    assert_eq!(
        grouped["r.pages"].content,
        Content::Package(BTreeMap::from([("z".to_owned(), 4)]))
    );
}
