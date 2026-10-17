//! Time codec laws (10 §10.1): UTC round trips, explicit offsets denote
//! the same instant, and impossible calendar dates or offsets are refused.

use hegel::{TestCase, generators as gs};
use pigeon_proto::time::{parse_rfc3339, rfc3339};

/// Law *UTC inverse*: every second in a four-digit civil year survives text.
#[hegel::test(test_cases = 1000)]
fn utc_round_trips(tc: TestCase) {
    let second = tc.draw(
        gs::integers::<i64>()
            .min_value(-62_167_219_200)
            .max_value(253_402_300_799),
    );
    assert_eq!(parse_rfc3339(&rfc3339(second)), Some(second));
}

/// Law *offset equivalence*: shifting the clock and writing its offset
/// denotes the original UTC instant, including a change of calendar day.
#[hegel::test(test_cases = 1000)]
fn offsets_denote_the_same_instant(tc: TestCase) {
    let second = tc.draw(
        gs::integers::<i64>()
            .min_value(-62_167_132_800)
            .max_value(253_402_214_399),
    );
    let minutes = tc.draw(gs::integers::<i64>().min_value(-1439).max_value(1439));
    let clock = rfc3339(second + minutes * 60);
    let sign = if minutes < 0 { '-' } else { '+' };
    let absolute = minutes.abs();
    let text = format!(
        "{}{sign}{:02}:{:02}",
        &clock[..19],
        absolute / 60,
        absolute % 60
    );
    assert_eq!(parse_rfc3339(&text), Some(second));
}

/// Law *calendar validity*: a day beyond a month's length never normalizes
/// into a different month. Gregorian century rules apply to February.
#[hegel::test(test_cases = 1000)]
fn impossible_dates_are_refused(tc: TestCase) {
    let year = tc.draw(gs::integers::<u16>().max_value(9999));
    let (month, day) = match tc.draw(gs::integers::<u8>().max_value(4)) {
        0 => (
            2,
            if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
                30
            } else {
                29
            },
        ),
        1 => (4, 31),
        2 => (6, 31),
        3 => (9, 31),
        _ => (11, 31),
    };
    assert_eq!(
        parse_rfc3339(&format!("{year:04}-{month:02}-{day:02}T12:34:56Z")),
        None
    );
}

/// Law *offset validity*: the sign is followed by two decimal hours in
/// 00..23 and two decimal minutes in 00..59, never another sign.
#[hegel::test(test_cases = 1000)]
fn malformed_offsets_are_refused(tc: TestCase) {
    let hour = tc.draw(gs::integers::<u8>().min_value(24).max_value(99));
    let minute = tc.draw(gs::integers::<u8>().min_value(60).max_value(99));
    let index = tc.draw(gs::integers::<u8>().max_value(5));
    let offset = match index {
        0 => format!("+{hour:02}:00"),
        1 => format!("-00:{minute:02}"),
        2 => "++1:00".to_owned(),
        3 => "--1:00".to_owned(),
        4 => "+00:+1".to_owned(),
        _ => "-00:-1".to_owned(),
    };
    assert_eq!(parse_rfc3339(&format!("2026-10-17T12:34:56{offset}")), None);
}

#[test]
fn calendar_boundaries_keep_valid_leap_days() {
    for text in [
        "0000-02-29T00:00:00Z",
        "2000-02-29T00:00:00Z",
        "2024-02-29T23:59:59Z",
    ] {
        assert!(parse_rfc3339(text).is_some(), "{text}");
    }
    for text in [
        "1900-02-29T00:00:00Z",
        "2100-02-29T00:00:00Z",
        "2099-04-31T00:00:00Z",
    ] {
        assert_eq!(parse_rfc3339(text), None, "{text}");
    }
}
