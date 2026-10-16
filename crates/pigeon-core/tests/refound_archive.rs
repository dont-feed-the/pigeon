//! Signed archive-link laws for offline recovery (09 §9.9): exact links
//! round trip; unsigned or changed bytes refuse; only roots with exactly
//! one well-formed trailer can introduce an archive. 1000 cases per law.
#![allow(clippy::unwrap_used, reason = "tests")]

use hegel::{TestCase, generators as gs};
use pigeon_core::{
    refound::{Archive, archive},
    sig,
};

fn input(tc: &TestCase) -> (Vec<u8>, [u8; 32], Archive) {
    let group = format!("{:040x}", tc.draw(gs::integers::<u64>()));
    let main = format!("{:040x}", tc.draw(gs::integers::<u64>()));
    let key = [tc.draw(gs::integers::<u8>()); 32];
    let raw = format!("tree {}\nauthor Ana <a@example.invalid> 1 +0000\ncommitter Ana <a@example.invalid> 1 +0000\n\nrefound\n\nPigeon-Refound: {group} {main}\n", "1".repeat(40)).into_bytes();
    (raw, key, Archive { group, main })
}

#[hegel::test(test_cases = 1000)]
fn a_signed_root_recovers_its_exact_link(tc: TestCase) {
    let (raw, key, expected) = input(&tc);
    assert_eq!(archive(&sig::sign(&raw, &key).unwrap()), Some(expected));
}

#[hegel::test(test_cases = 1000)]
fn unsigned_or_modified_links_refuse(tc: TestCase) {
    let (raw, key, _) = input(&tc);
    assert_eq!(archive(&raw), None);
    let mut signed = sig::sign(&raw, &key).unwrap();
    let at = signed.len() - 2;
    signed[at] = if signed[at] == b'a' { b'b' } else { b'a' };
    assert_eq!(archive(&signed), None);
}

#[hegel::test(test_cases = 1000)]
fn a_non_root_cannot_introduce_an_archive(tc: TestCase) {
    let (raw, key, _) = input(&tc);
    let text = String::from_utf8(raw).unwrap();
    let (tree, rest) = text.split_once('\n').unwrap();
    let non_root = format!(
        "{tree}\nparent {:040x}\n{rest}",
        tc.draw(gs::integers::<u64>())
    );
    assert_eq!(
        archive(&sig::sign(non_root.as_bytes(), &key).unwrap()),
        None
    );
}

#[hegel::test(test_cases = 1000)]
fn malformed_and_ambiguous_signed_links_refuse(tc: TestCase) {
    let (raw, key, _) = input(&tc);
    let text = String::from_utf8(raw).unwrap();
    let malformed = match tc.draw(gs::integers::<u8>().max_value(3)) {
        0 => format!(
            "{text}Pigeon-Refound: {} {}\n",
            "2".repeat(40),
            "3".repeat(40)
        ),
        1 => text.replace("Pigeon-Refound:", "Pigeon-Refound: extra"),
        2 => text.replace("Pigeon-Refound: ", "Pigeon-Refound: invalid-"),
        _ => text
            .lines()
            .filter(|l| !l.starts_with("Pigeon-Refound:"))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    assert_eq!(
        archive(&sig::sign(malformed.as_bytes(), &key).unwrap()),
        None
    );
}
