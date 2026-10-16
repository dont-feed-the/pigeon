//! Folder policy wire law (04 §4.3): identities, names, default stewards
//! and edit authority survive split reads, 1000 cases per law.
#![allow(clippy::unwrap_used, reason = "property-test fixtures")]
use hegel::{TestCase, generators as gs};
use pigeon_proto::{LineDecoder, Request, Response, encode, policy};
fn text(tc: &TestCase) -> String {
    tc.draw(gs::text().max_size(30))
}
fn flag(tc: &TestCase) -> bool {
    tc.draw(gs::booleans())
}

#[hegel::test(test_cases = 1000)]
fn folder_policy_preserves_names_selections_and_authority(tc: TestCase) {
    let people: Vec<_> = (0..tc.draw(gs::integers::<u8>().max_value(4)))
        .map(|_| policy::Person {
            id: text(&tc),
            name: text(&tc),
            this_person: flag(&tc),
            admin: flag(&tc),
        })
        .collect();
    let rules = (0..tc.draw(gs::integers::<u8>().max_value(4)))
        .map(|_| policy::Rule {
            path: text(&tc),
            owner: flag(&tc).then(|| text(&tc)),
            stewards: people
                .iter()
                .filter(|_| flag(&tc))
                .map(|p| p.id.clone())
                .collect(),
            direct: flag(&tc),
            per_person: flag(&tc),
        })
        .collect();
    let response = Response::FolderRules(Box::new(policy::Report {
        root: text(&tc),
        rules,
        people,
        can_edit: flag(&tc),
    }));
    let bytes = encode(9, &response).unwrap();
    let mut decoder = LineDecoder::new();
    for chunk in bytes.chunks(usize::from(tc.draw(gs::integers::<u8>().min_value(1)))) {
        decoder.push(chunk);
    }
    assert_eq!(
        decoder.next_line::<Response>().unwrap().unwrap().body,
        response
    );
    assert!(decoder.into_rest().is_empty());
    let rule = if flag(&tc) {
        policy::Change::Owned { person: text(&tc) }
    } else if flag(&tc) {
        policy::Change::PerPerson
    } else {
        policy::Change::AddOnly {
            stewards: (0..tc.draw(gs::integers::<u8>().max_value(4)))
                .map(|_| text(&tc))
                .collect(),
        }
    };
    let request = Request::SetFolderRule {
        folder: text(&tc),
        rule,
        confirmed: flag(&tc),
    };
    decoder = LineDecoder::new();
    decoder.push(&encode(10, &request).unwrap());
    assert_eq!(
        decoder.next_line::<Request>().unwrap().unwrap().body,
        request
    );
}
