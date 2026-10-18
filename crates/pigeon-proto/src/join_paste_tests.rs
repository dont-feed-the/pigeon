//! The linear parser preserves the original longest-valid-word-prefix rule.
//! The bounded reference retains the old repeated decoding as an oracle;
//! production inputs are not repeatedly decoded or concatenated.

use crate::join::{INVITATION_VERSION, Invitation, decode, encode, invitation};
use hegel::{TestCase, generators as gs};

fn original(pasted: &str) -> String {
    let Some(start) = pasted.find("pgn2-") else {
        return pasted.trim().to_owned();
    };
    let words: Vec<&str> = pasted[start + 5..].split_whitespace().collect();
    (1..=words.len())
        .rev()
        .map(|n| format!("pgn2-{}", words[..n].concat()))
        .find(|code| decode(code).is_some())
        .unwrap_or_else(|| pasted.trim().to_owned())
}

/// Law: arbitrary wrapping, JSON string escapes, trailing chat and versions
/// keep exactly the original longest complete invitation or trimmed refusal.
#[hegel::test(test_cases = 1000)]
fn wrapped_invitations_and_trailing_messages_preserve_the_original_result(tc: TestCase) {
    let inv = Invitation {
        v: if tc.draw(gs::booleans()) {
            INVITATION_VERSION
        } else {
            0
        },
        group: tc.draw(gs::text().max_size(12)),
        name: tc.draw(gs::text().max_size(12)),
        k: tc.draw(gs::integers::<u32>()),
        admitters: Vec::new(),
        secret: "synthetic".into(),
        relay: None,
        refounded: None,
    };
    let code = encode(&inv);
    let width = usize::from(tc.draw(gs::integers::<u8>().min_value(1).max_value(40)));
    let separator = if tc.draw(gs::booleans()) {
        "\n"
    } else {
        "\u{2003}"
    };
    let body = code.as_bytes()[5..]
        .chunks(width)
        .map(|chunk| String::from_utf8_lossy(chunk))
        .collect::<Vec<_>>()
        .join(separator);
    let suffix = format!(
        "{} {}",
        "a ".repeat(usize::from(tc.draw(gs::integers::<u8>().max_value(20)))),
        tc.draw(gs::text().max_size(50))
    );
    let pasted = format!("before pgn2-{body} {suffix}");
    assert_eq!(invitation(&pasted), original(&pasted));
}

/// Law: arbitrary candidate alphabets, invalid words and suffix bits cannot
/// introduce an invitation the ordinary strict decoder would reject.
#[hegel::test(test_cases = 1000)]
fn arbitrary_candidates_preserve_strict_refusal(tc: TestCase) {
    let text = tc.draw(gs::text().max_size(150));
    let pasted = format!("pgn2-{text}");
    assert_eq!(invitation(&pasted), original(&pasted));
}

#[test]
fn canonical_padding_bits_and_longest_trailing_json_space_are_preserved() {
    use data_encoding::BASE32_NOPAD;
    // Known valid JSON is encoded with explicit trailing JSON whitespace;
    // ordinary decode accepts it, including wrapped canonical word ends.
    let json = br#"{"v":2,"group":"g","name":"n","k":1,"admitters":[],"secret":"synthetic"}"#;
    let mut spaced = b"\r\n".to_vec();
    spaced.extend_from_slice(json);
    spaced.extend_from_slice(b" \n\t\r ");
    let body = BASE32_NOPAD.encode(&spaced).to_ascii_lowercase();
    let code = format!("pgn2-{body}");
    assert!(decode(&code).is_some());
    assert_eq!(invitation(&format!("{code} a a invalid!")), code);
    let mut checked = 0;
    for spaces in 0..5 {
        let mut data = json.to_vec();
        data.extend(std::iter::repeat_n(b' ', spaces));
        let mut body = BASE32_NOPAD.encode(&data).into_bytes();
        if body.len().is_multiple_of(8) {
            continue;
        }
        let last = body.len() - 1;
        let value = super::digit(body[last]).expect("encoded alphabet");
        body[last] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"[usize::from(value ^ 1)];
        let bad = format!("pgn2-{}", String::from_utf8_lossy(&body));
        assert!(decode(&bad).is_none());
        assert_eq!(invitation(&bad), bad);
        checked += 1;
    }
    assert_eq!(checked, 4);
}
