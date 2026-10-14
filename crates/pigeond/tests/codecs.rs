//! Laws of `pigeond`'s own codecs (13 §13.12): hex and RFC 3339 times
//! round-trip, a journal record parses back from its line (07 §7.5), a
//! roster file parses back to the roster it renders, and an invitation and
//! a recovery sheet decode to what they encode. HMAC-SHA256 matches RFC
//! 4231.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::BTreeMap;

use hegel::TestCase;
use hegel::generators as gs;
use pigeon_core::journal::{Begin, End, Record, Stat};
use pigeon_core::roster::{Device, DeviceId, Person, PersonId, Role, Roster};
use pigeond::invite::{Invitation, decode, encode};
use pigeond::roster_file::{RosterDoc, parse, render};
use pigeond::util::{date, hex, hmac_sha256, parse_rfc3339, rfc3339, unhex};

/// Law *hex*: bytes round-trip through their lower-case hex.
#[hegel::test]
fn hex_round_trips(tc: TestCase) {
    let bytes = tc.draw(gs::binary().max_size(64));
    let h = hex(&bytes);
    assert_eq!(h.len(), 2 * bytes.len());
    assert!(
        h.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_eq!(unhex(&h), Some(bytes));
}

/// Law *time*: a time from 1970 to 9999 round-trips through RFC 3339, and
/// its date is the text's first ten characters.
#[hegel::test]
fn rfc3339_round_trips(tc: TestCase) {
    let t = tc.draw(
        gs::integers::<i64>()
            .min_value(0)
            .max_value(253_402_300_799),
    );
    let text = rfc3339(t);
    assert_eq!(parse_rfc3339(&text), Some(t), "{text}");
    assert_eq!(text.get(..10), Some(date(t).as_str()));
    let shifted = format!("{}+02:00", text.trim_end_matches('Z'));
    assert_eq!(parse_rfc3339(&shifted), Some(t - 7200));
}

fn word(tc: &TestCase) -> String {
    tc.draw(gs::text().max_size(12))
}

fn maybe(tc: &TestCase) -> Option<String> {
    tc.draw(gs::booleans()).then(|| word(tc))
}

fn record(tc: &TestCase) -> Record {
    let seq = tc.draw(gs::integers::<u64>().max_value(1 << 40));
    match tc.draw(gs::integers::<u8>().max_value(3)) {
        0 => Record::Begin(Begin {
            seq,
            path: word(tc),
            old_key: maybe(tc),
            new_key: maybe(tc),
            stat: tc.draw(gs::booleans()).then(|| Stat {
                dev: tc.draw(gs::integers::<u64>()),
                ino: tc.draw(gs::integers::<u64>()),
                size: tc.draw(gs::integers::<u64>().max_value(1 << 50)),
                mtime_ns: i128::from(tc.draw(gs::integers::<i64>())),
            }),
            stage: maybe(tc),
            aside: word(tc),
        }),
        1 => {
            let n = tc.draw(gs::integers::<u8>().max_value(4));
            Record::Proved(
                (0..n)
                    .map(|_| tc.draw(gs::integers::<u64>().max_value(1 << 40)))
                    .collect(),
            )
        }
        2 => {
            let end = match tc.draw(gs::integers::<u8>().max_value(3)) {
                0 => End::Done,
                1 => End::Skip,
                2 => End::Raced,
                _ => End::Recovered,
            };
            Record::End(seq, end)
        }
        _ => Record::Released(seq),
    }
}

/// Law *journal*: every record parses back from its line, which is one
/// line.
#[hegel::test]
fn journal_records_round_trip(tc: TestCase) {
    let r = record(&tc);
    let line = pigeond::journal_file::line(&r);
    assert!(!line.trim_end_matches('\n').contains('\n'), "{line}");
    assert_eq!(pigeond::journal_file::parse(&line), Some(r));
}

fn device_id(tc: &TestCase) -> DeviceId {
    DeviceId(hex(&tc.draw(gs::binary().min_size(32).max_size(32))))
}

fn roster_doc(tc: &TestCase) -> RosterDoc {
    let person = PersonId("ada".to_owned());
    let mut devices = BTreeMap::new();
    let mut removed = BTreeMap::new();
    for i in 0..tc.draw(gs::integers::<u8>().min_value(1).max_value(4)) {
        let id = device_id(tc);
        let role = match tc.draw(gs::integers::<u8>().max_value(2)) {
            0 => Role::Laptop,
            1 => Role::Custodian,
            _ => Role::Recovery,
        };
        let maxsize = tc
            .draw(gs::booleans())
            .then(|| tc.draw(gs::integers::<u64>().max_value(1 << 50)));
        devices.insert(
            id,
            Device {
                person: person.clone(),
                name: format!("{}{i}", word(tc)),
                role,
                annex_uuid: maybe(tc),
                maxsize,
            },
        );
        if tc.draw(gs::booleans()) {
            // An empty UUID is written for none.
            let uuid = maybe(tc).filter(|u| !u.is_empty());
            removed.insert(device_id(tc), uuid);
        }
    }
    RosterDoc {
        roster: Roster {
            rules: 1,
            group: word(tc),
            k: tc.draw(gs::integers::<u32>().min_value(1).max_value(5)),
            retention_days: tc.draw(gs::integers::<u32>().max_value(3650)),
            persons: BTreeMap::from([(
                person,
                Person {
                    name: word(tc),
                    admin: tc.draw(gs::booleans()),
                },
            )]),
            devices,
            removed,
            folders: Vec::new(),
        },
    }
}

/// Law *roster file*: a roster parses back from the text it renders to.
#[hegel::test]
fn roster_files_round_trip(tc: TestCase) {
    let doc = roster_doc(&tc);
    let text = render(&doc).unwrap();
    assert_eq!(parse(&text), Ok(doc), "{text}");
}

/// Law *invitation*: an invitation decodes from its `pgn1-` code, which is
/// lower-case base32, and a damaged code decodes to nothing else.
#[hegel::test]
fn invitations_round_trip(tc: TestCase) {
    let n = tc.draw(gs::integers::<u8>().max_value(3));
    let inv = Invitation {
        v: 1,
        group: hex(&tc.draw(gs::binary().min_size(20).max_size(20))),
        name: word(&tc),
        k: tc.draw(gs::integers::<u32>().min_value(1).max_value(5)),
        inviter: device_id(&tc).0,
        from: word(&tc),
        addresses: (0..n)
            .map(|i| format!("127.0.0.1:{}", 4000 + u32::from(i)))
            .collect(),
        secret: hex(&tc.draw(gs::binary().min_size(32).max_size(32))),
        person: word(&tc),
        role: "custodian".to_owned(),
        device: word(&tc),
        expires: tc.draw(gs::integers::<i64>().min_value(0).max_value(1 << 40)),
    };
    let code = encode(&inv);
    let body = code.strip_prefix("pgn1-").unwrap();
    assert!(
        body.bytes()
            .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
    );
    assert_eq!(decode(&code), Some(inv.clone()));
    assert_eq!(
        decode(&code.to_uppercase().replacen("PGN1-", "pgn1-", 1)),
        Some(inv)
    );
    assert_eq!(decode(body), None);
}

/// Law *sheet*: a recovery sheet reads back from its `pgr1-` code wherever
/// it stands in the sheet's text, and a code with any character changed
/// reads as nothing (09 §9.9).
#[hegel::test]
fn recovery_sheets_round_trip(tc: TestCase) {
    use pigeond::recovery::{Sheet, SheetCustodian, decode, encode};
    let n = tc.draw(gs::integers::<u8>().max_value(3));
    let sheet = Sheet {
        v: 1,
        group: hex(&tc.draw(gs::binary().min_size(20).max_size(20))),
        name: word(&tc),
        k: tc.draw(gs::integers::<u32>().min_value(1).max_value(9)),
        person: word(&tc),
        key: hex(&tc.draw(gs::binary().min_size(32).max_size(32))),
        custodians: (0..n)
            .map(|_| SheetCustodian {
                device: hex(&tc.draw(gs::binary().min_size(32).max_size(32))),
                name: word(&tc),
                addresses: vec![format!("127.0.0.1:{}", tc.draw(gs::integers::<u16>()))],
            })
            .collect(),
    };
    let code = encode(&sheet);
    let text = format!("pigeon recovery sheet\n  {code}\nKeep it safe.\n");
    assert_eq!(decode(&text), Some(sheet));
    let at = tc.draw(
        gs::integers::<usize>()
            .min_value(5)
            .max_value(code.len() - 1),
    );
    let mut typo: Vec<char> = code.chars().collect();
    typo[at] = if typo[at] == 'a' { 'b' } else { 'a' };
    let typo: String = typo.into_iter().collect();
    assert_eq!(decode(&typo), None);
}

/// RFC 4231, test case 2.
#[test]
fn hmac_matches_rfc_4231() {
    let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
    assert_eq!(
        hex(&mac),
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}
