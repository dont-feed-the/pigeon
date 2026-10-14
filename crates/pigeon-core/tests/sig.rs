//! Laws of commit signatures (ADR 0007, 04 §4.4; pigeon-core `sig`): a
//! signed commit verifies to its signer, cutting the signature off gives
//! the commit back, and no change of one byte forges a signature.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::{flag, index, int};
use hegel::TestCase;
use pigeon_core::sig::{self, SigError};

/// A commit object as git writes one, with drawn parents, times and message.
pub fn commit(tc: &TestCase) -> Vec<u8> {
    let mut s = format!("tree {:040x}\n", int(tc, 0, 1 << 40));
    for _ in 0..index(tc, 3) {
        s.push_str(&format!("parent {:040x}\n", int(tc, 0, 1 << 40)));
    }
    let t = int(tc, 0, 2_000_000_000);
    s.push_str(&format!("author pigeon <pigeon@localhost> {t} +0000\n"));
    s.push_str(&format!("committer pigeon <pigeon@localhost> {t} +0000\n"));
    if flag(tc) {
        s.push_str("\npigeon: capture\n");
        if flag(tc) {
            s.push_str("\nPigeon-Force: a reason\n");
        }
    }
    s.into_bytes()
}

fn secret(tc: &TestCase) -> [u8; 32] {
    let mut k = [0u8; 32];
    for b in &mut k {
        *b = int(tc, 0, 255) as u8;
    }
    k
}

#[hegel::test(test_cases = 200)]
fn a_signed_commit_verifies_to_its_signer(tc: TestCase) {
    let (raw, key) = (commit(&tc), secret(&tc));
    let signed = sig::sign(&raw, &key).unwrap();
    assert_eq!(sig::verify(&signed), Ok(sig::device_id(&key)));
    assert_eq!(
        sig::split(&signed).0,
        raw,
        "the signed bytes are the commit"
    );
    assert_eq!(sig::verify(&raw), Err(SigError::Unsigned));
    assert_eq!(sig::sign(&signed, &key), Err(SigError::AlreadySigned));
}

#[hegel::test(test_cases = 300)]
fn no_change_of_one_byte_forges_a_signature(tc: TestCase) {
    let (raw, key) = (commit(&tc), secret(&tc));
    let signed = sig::sign(&raw, &key).unwrap();
    let mut bad = signed.clone();
    let i = index(&tc, bad.len());
    bad[i] ^= int(&tc, 1, 255) as u8;
    // A change in the armour that decodes to the same signature (a
    // non-canonical base64 bit) still verifies, but only on the same bytes.
    if let Ok(who) = sig::verify(&bad) {
        assert_eq!(who, sig::device_id(&key));
        assert_eq!(sig::split(&bad).0, raw, "byte {i} changed the signed bytes");
    }
}

#[hegel::test(test_cases = 100)]
fn a_signature_does_not_move_to_another_commit(tc: TestCase) {
    let (a, b, key) = (commit(&tc), commit(&tc), secret(&tc));
    let sa = sig::sign(&a, &key).unwrap();
    let (_, sigs) = sig::split(&sa);
    let header: String = sigs[0]
        .split_inclusive('\n')
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                format!("gpgsig {l}")
            } else {
                format!(" {l}")
            }
        })
        .collect();
    let end = b
        .windows(2)
        .position(|w| w == b"\n\n")
        .map_or(b.len(), |i| i + 1);
    let mut moved = b[..end].to_vec();
    moved.extend_from_slice(header.as_bytes());
    moved.extend_from_slice(&b[end..]);
    let verdict = sig::verify(&moved);
    assert_eq!(verdict.is_ok(), a == b, "{verdict:?}");
}

#[test]
fn two_signatures_and_other_namespaces_are_refused() {
    let raw = b"tree 0000000000000000000000000000000000000000\nauthor a <a> 0 +0000\ncommitter a <a> 0 +0000\n\nm\n".to_vec();
    let key = [7u8; 32];
    let signed = sig::sign(&raw, &key).unwrap();
    let (_, sigs) = sig::split(&signed);
    let end = signed.windows(2).position(|w| w == b"\n\n").unwrap() + 1;
    let mut twice = signed[..end].to_vec();
    for (i, l) in sigs[0].split_inclusive('\n').enumerate() {
        twice.extend_from_slice(if i == 0 { b"gpgsig " } else { b" " });
        twice.extend_from_slice(l.as_bytes());
    }
    twice.extend_from_slice(&signed[end..]);
    assert_eq!(sig::verify(&twice), Err(SigError::TwoSignatures));
    let garbled = String::from_utf8(signed.clone())
        .unwrap()
        .replace("SSH SIGNATURE", "PGP SIGNATURE");
    assert_eq!(sig::verify(garbled.as_bytes()), Err(SigError::Unreadable));
}
