//! Laws of the network's small parsers (09 §9.1, §9.4; ADR 0012, spec 09 §9.4):
//! device IDs round-trip through their hex, the hash a `SHA256E` key names
//! is read back exactly, and a key from a peer never reaches git-annex as
//! an option or a path.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use hegel::TestCase;
use hegel::generators as gs;
use pigeond::net::sync::{key_sha256, plausible_key};
use pigeond::net::{device_id, parse_device_id};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Law *device ID*: a public key's ID is 64 lowercase hex digits and parses
/// back to the same key; anything else is not an ID.
#[hegel::test(test_cases = 1000)]
fn device_ids_round_trip(tc: TestCase) {
    let seed: Vec<u8> = tc.draw(gs::binary().min_size(32).max_size(32));
    let secret = iroh::SecretKey::from_bytes(&seed.clone().try_into().unwrap());
    let id = device_id(&secret.public());
    assert_eq!(id.len(), 64);
    assert!(
        id.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_eq!(parse_device_id(&id), Some(secret.public()));
    let junk = tc.draw(gs::text().max_size(70));
    if junk.len() != 64 {
        assert_eq!(parse_device_id(&junk), None);
    }
}

/// Law *key hash*: the SHA-256 of a `SHA256E` or `SHA256` key is its hex
/// field, whatever its size and extension; other backends name none.
#[hegel::test(test_cases = 1000)]
fn sha256_keys_name_their_hash(tc: TestCase) {
    let digest = hex(&tc.draw(gs::binary().min_size(32).max_size(32)));
    let size = tc.draw(gs::integers::<u64>());
    let key = if tc.draw(gs::booleans()) {
        format!("SHA256E-s{size}--{digest}.tar.gz")
    } else {
        format!("SHA256-s{size}--{digest}")
    };
    assert_eq!(key_sha256(&key), Some(digest.as_str()));
    assert!(plausible_key(&key));
    assert_eq!(key_sha256(&format!("WORM-s{size}--{digest}")), None);
}

/// Law *plausible key*: a key that starts with `-`, holds a `/`, a space or
/// a control character is never passed to git-annex.
#[hegel::test(test_cases = 1000)]
fn implausible_keys_are_refused(tc: TestCase) {
    let key = tc.draw(gs::text().max_size(40));
    if plausible_key(&key) {
        assert!(!key.is_empty() && !key.starts_with('-'));
        assert!(key.bytes().all(|b| b.is_ascii_graphic() && b != b'/'));
    }
    assert!(!plausible_key(&format!("-{key}")));
    assert!(!plausible_key(&format!("{key}/x")));
}
