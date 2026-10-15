//! Laws of `pigeond`'s own codecs (13 §13.12): hex and RFC 3339 times
//! round-trip, a journal record parses back from its line (07 §7.5), a
//! roster file parses back to the roster it renders and one a newer release
//! could write is unknown (ADR 0048), a divergence table parses back, and
//! an invitation and a recovery sheet decode to what they encode, and a
//! decision file parses back. HMAC-SHA256 matches RFC 4231.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::BTreeMap;

use hegel::TestCase;
use hegel::generators as gs;
use pigeon_core::authority::Unread;
use pigeon_core::journal::{Begin, End, Record, Stat};
use pigeon_core::roster::{Device, DeviceId, Person, PersonId, Role, Roster};
use pigeond::invite::{Invitation, decode, encode};
use pigeond::roster_file::{RosterDoc, parse, render};
use pigeond::util::{date, hex, hmac_sha256, parse_rfc3339, rfc3339, unhex};

/// Law *hex*: bytes round-trip through their lower-case hex.
#[hegel::test(test_cases = 1000)]
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
#[hegel::test(test_cases = 1000)]
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
    match tc.draw(gs::integers::<u8>().max_value(5)) {
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
        3 => Record::Kept(seq, tc.draw(gs::integers::<i64>())),
        4 => Record::Preserved(seq, word(tc)),
        _ => Record::Released(seq),
    }
}

/// Law *journal*: every record parses back from its line, which is one
/// line.
#[hegel::test(test_cases = 1000)]
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
            relay: tc
                .draw(gs::booleans())
                .then(|| format!("https://{}.example.org", word(tc))),
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
#[hegel::test(test_cases = 1000)]
fn roster_files_round_trip(tc: TestCase) {
    let doc = roster_doc(&tc);
    let text = render(&doc).unwrap();
    assert_eq!(parse(&text), Ok(doc), "{text}");
}

/// Law *closed roster* (ADR 0048): a rendered roster with a newer `format`,
/// or with one key format 1 does not define, at the top or in a person, a
/// device or a folder, is unknown: never read, and never refused.
#[hegel::test(test_cases = 1000)]
fn a_roster_a_newer_release_could_write_is_unknown(tc: TestCase) {
    let doc = roster_doc(&tc);
    let mut newer: toml_edit::DocumentMut = render(&doc).unwrap().parse().unwrap();
    let key = format!("x{}", tc.draw(gs::integers::<u16>()));
    let value = toml_edit::value(tc.draw(gs::integers::<i64>()));
    match tc.draw(gs::integers::<u8>().max_value(4)) {
        0 => newer["format"] = toml_edit::value(tc.draw(gs::integers::<i64>().min_value(2))),
        1 => newer["persons"]["ada"][&key] = value,
        2 => {
            let devices = newer["devices"].as_table_mut().unwrap();
            let (_, first) = devices.iter_mut().next().unwrap();
            first[&key] = value;
        }
        3 if newer.contains_key("folders") => {
            let folders = newer["folders"].as_array_of_tables_mut().unwrap();
            folders.get_mut(0).unwrap()[&key] = value;
        }
        _ => newer[&key] = value,
    }
    let text = newer.to_string();
    assert!(matches!(parse(&text), Err(Unread::Unknown(_))), "{text}");
}

/// Law *invitation*: an invitation decodes from its `pgn1-` code, which is
/// lower-case base32, and a damaged code decodes to nothing else.
#[hegel::test(test_cases = 1000)]
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
        new_person: if tc.draw(gs::booleans()) {
            word(&tc)
        } else {
            String::new()
        },
        admin: tc.draw(gs::booleans()),
        role: if tc.draw(gs::booleans()) {
            "custodian"
        } else {
            "laptop"
        }
        .to_owned(),
        device: word(&tc),
        expires: tc.draw(gs::integers::<i64>().min_value(0).max_value(1 << 40)),
        relay: tc
            .draw(gs::booleans())
            .then(|| format!("https://{}.example.org", word(&tc))),
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

/// Law *round trip* (11 §11.3): a capture batch's intent (`capture.json`,
/// 07 §7.3) parses back to the intent it renders, and one of a newer
/// format reads as nothing.
#[hegel::test(test_cases = 1000)]
fn capture_intents_round_trip(tc: TestCase) {
    use pigeond::capture_file::{FORMAT, Intent, Staged, parse, render};
    let n = tc.draw(gs::integers::<u8>().max_value(4));
    let before = (0..n)
        .map(|_| {
            let e = tc.draw(gs::booleans()).then(|| Staged {
                mode: if tc.draw(gs::booleans()) {
                    "100644"
                } else {
                    "100755"
                }
                .to_owned(),
                oid: hex(&tc.draw(gs::binary().min_size(20).max_size(20))),
            });
            (word(&tc), e)
        })
        .collect();
    let i = Intent {
        format: FORMAT,
        head: maybe(&tc),
        before,
    };
    assert_eq!(parse(&render(&i)), Some(i.clone()));
    let newer = Intent {
        format: FORMAT + 1,
        ..i
    };
    assert_eq!(parse(&render(&newer)), None);
}

/// Law *new person* (09 §9.6): the ID an invitation gives a new person is
/// lowercase ASCII letters and digits, no person of the roster has it, and
/// the person is found by it once added.
#[hegel::test(test_cases = 1000)]
fn a_new_persons_id_is_fresh(tc: TestCase) {
    use pigeon_core::roster::{Person, PersonId, Roster};
    use pigeond::invite::{find_person, new_person_id};
    let mut roster = Roster {
        rules: 1,
        group: "g".into(),
        k: 1,
        retention_days: 30,
        relay: None,
        persons: BTreeMap::new(),
        devices: BTreeMap::new(),
        removed: BTreeMap::new(),
        folders: Vec::new(),
    };
    let names = ["Ana", "ana", "Ana2", "Zoé", "", "Ünal", "person", "ana3"];
    let n = tc.draw(gs::integers::<usize>().max_value(6));
    for _ in 0..n {
        let name = names[tc.draw(gs::integers::<usize>().max_value(names.len() - 1))];
        let id = new_person_id(&roster, name);
        assert!(
            !id.0.is_empty()
                && id
                    .0
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()),
            "{id:?}"
        );
        assert!(!roster.persons.contains_key(&id), "{id:?}");
        let person = Person {
            name: name.to_owned(),
            admin: false,
        };
        roster.persons.insert(id.clone(), person);
        assert_eq!(find_person(&roster, &id.0), Some(PersonId(id.0.clone())));
    }
}

/// Law *round trip* (11 §11.5): a divergence table (`conflicts.json`, 07
/// §7.6) parses back to the table it renders, and one of a newer format
/// reads as nothing, so it is never rewritten.
#[hegel::test(test_cases = 1000)]
fn conflict_tables_round_trip(tc: TestCase) {
    use pigeond::conflicts_file::{Divergence, FORMAT, Kind, State, Table, parse, render};
    let n = tc.draw(gs::integers::<u8>().max_value(4));
    let mut paths = BTreeMap::new();
    for _ in 0..n {
        let kinds = [Kind::Suggestion, Kind::Concurrent, Kind::Recovered];
        let kind = kinds[tc.draw(gs::integers::<usize>().max_value(2))];
        let state = if tc.draw(gs::booleans()) {
            State::Waiting
        } else {
            State::Conflict
        };
        let stat = tc.draw(gs::booleans()).then(|| {
            let u = |tc: &TestCase| i128::from(tc.draw(gs::integers::<u64>()));
            [
                u(&tc),
                u(&tc),
                u(&tc),
                i128::from(tc.draw(gs::integers::<i64>())),
            ]
        });
        let since = tc.draw(gs::integers::<i64>().min_value(0).max_value(4_000_000_000));
        let d = Divergence {
            kind,
            since: rfc3339(since),
            state,
            mine: maybe(&tc),
            mine_stat: stat,
            proposal: maybe(&tc),
            group: maybe(&tc),
            installed: maybe(&tc),
            told: tc.draw(gs::booleans()),
        };
        paths.insert(word(&tc), d);
    }
    let t = Table {
        format: FORMAT,
        paths,
    };
    assert_eq!(parse(&render(&t)), Some(t.clone()));
    let newer = Table {
        format: FORMAT + 1,
        ..t
    };
    assert_eq!(parse(&render(&newer)), None);
}

/// Law *sheet*: a recovery sheet reads back from its `pgr1-` code wherever
/// it stands in the sheet's text, and a code with any character changed
/// reads as nothing (09 §9.9).
#[hegel::test(test_cases = 1000)]
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

/// Law *decision*: a decision file parses back to what it renders, and one
/// with a key or verdict a newer release could write is unknown (ADR 0048).
#[hegel::test(test_cases = 1000)]
fn decisions_round_trip(tc: TestCase) {
    use pigeon_core::authority::Decided;
    use pigeond::decision_file::{DecisionDoc, parse, render};
    let hexes: String = (0..40)
        .map(|_| char::from(b"0123456789abcdef"[tc.draw(gs::integers::<usize>().max_value(15))]))
        .collect();
    let d = DecisionDoc {
        proposal: hexes,
        verdict: if tc.draw(gs::booleans()) {
            Decided::Rejected
        } else {
            Decided::Withdrawn
        },
        note: maybe(&tc),
    };
    let text = render(&d);
    assert_eq!(parse(&text), Ok(d.clone()), "{text}");
    let newer = format!("{text}reason = \"x\"\n");
    assert!(matches!(parse(&newer), Err(Unread::Unknown(_))), "{newer}");
    let verdict = text.replace("verdict = \"rejected\"", "verdict = \"merged\"");
    let verdict = verdict.replace("verdict = \"withdrawn\"", "verdict = \"merged\"");
    assert!(
        matches!(parse(&verdict), Err(Unread::Unknown(_))),
        "{verdict}"
    );
    assert!(matches!(
        parse("format = 1\nproposal = 3\n"),
        Err(Unread::Malformed(_))
    ));
}
