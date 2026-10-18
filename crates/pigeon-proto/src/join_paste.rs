//! Find a wrapped invitation in one pass over its candidate words (09 §9.6).
//! Decode the base32 run once, parse one JSON value, then check word ends
//! for canonical base32 padding bits and JSON's permitted trailing space.

use crate::join::{INVITATION_VERSION, Invitation};

fn digit(byte: u8) -> Option<u8> {
    match byte.to_ascii_uppercase() {
        b'A'..=b'Z' => Some(byte.to_ascii_uppercase() - b'A'),
        b'2'..=b'7' => Some(byte - b'2' + 26),
        _ => None,
    }
}

pub(crate) fn find(body: &str) -> Option<String> {
    let mut joined = String::new();
    let mut boundaries = Vec::new();
    for word in body.split_whitespace() {
        if !word.bytes().all(|byte| digit(byte).is_some()) {
            break;
        }
        joined.push_str(word);
        boundaries.push(joined.len());
    }
    let mut bytes = Vec::with_capacity(joined.len() / 8 * 5);
    let mut pending = 0_u16;
    let mut bits = 0_u8;
    for byte in joined.bytes() {
        pending = (pending << 5) | u16::from(digit(byte)?);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            bytes.push((pending >> bits) as u8);
        }
        pending &= (1 << bits) - 1;
    }
    let mut json = serde_json::Deserializer::from_slice(&bytes).into_iter::<Invitation>();
    let invitation = json.next()?.ok()?;
    if invitation.v != INVITATION_VERSION {
        return None;
    }
    let end = json.byte_offset();
    let limit = end
        + bytes[end..]
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
            .count();
    let end = boundaries.into_iter().rev().find(|end| {
        // The unpadded alphabet admits only these lengths. The remainder
        // bits must be zero; do not accept aliases the ordinary decoder
        // would refuse. Division first avoids length multiplication overflow.
        let remainder = end % 8;
        if !matches!(remainder, 0 | 2 | 4 | 5 | 7) {
            return false;
        }
        let decoded = end / 8 * 5 + remainder * 5 / 8;
        if decoded < json.byte_offset() || decoded > limit {
            return false;
        }
        let unused = remainder * 5 % 8;
        digit(joined.as_bytes()[end - 1])
            .is_some_and(|last| u16::from(last) & ((1 << unused) - 1) == 0)
    })?;
    joined.truncate(end);
    Some(format!("pgn2-{joined}"))
}

#[cfg(test)]
#[path = "join_paste_tests.rs"]
mod tests;
