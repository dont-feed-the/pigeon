//! Immutable DAG reachability, 1000 generated cases: excluding references
//! at or after a birth agrees with looking backwards from every later tip.

use super::Index;
use hegel::{TestCase, generators as gs};

#[hegel::test(test_cases = 1000)]
fn ancestry_and_descendants_are_the_same_relation_in_opposite_directions(tc: TestCase) {
    let count = 1 + tc.draw(gs::integers::<u8>().max_value(31));
    let mut index = Index::default();
    for child in 0..count {
        let parents = (0..child)
            .filter(|_| tc.draw(gs::booleans()))
            .map(|parent| parent.to_string())
            .collect();
        index.parents.insert(child.to_string(), parents);
    }
    let ancestor = tc
        .draw(gs::integers::<u8>().max_value(count - 1))
        .to_string();
    let tip = tc
        .draw(gs::integers::<u8>().max_value(count - 1))
        .to_string();
    assert_eq!(
        index
            .ancestors(&tip)
            .is_some_and(|set| set.contains(&ancestor)),
        index.descendants(&ancestor).contains(&tip)
    );
}
