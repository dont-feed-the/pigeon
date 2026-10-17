//! Payload classification laws (ADR 0037, 09 §9.4, 15 §15.3): only
//! content DATA bodies may be throttled. Control lines and Git history
//! remain unshaped, independently of arbitrary transport read boundaries.
#![allow(
    clippy::panic,
    reason = "unexpected adapter action in a property-test fixture"
)]

use hegel::{TestCase, generators as gs};
use pigeon_core::filter::{Action, Config, Filter, Role};

fn filter() -> Filter {
    let mut filter = Filter::new(
        Config {
            role: Role::Laptop,
            auth_token: None,
            peer_uuid: None,
            peer_custodian: false,
        },
        false,
    );
    filter.classify_payloads();
    filter
}

fn wire(payload: &[u8]) -> Vec<u8> {
    let mut bytes = format!("DATA {}\n", payload.len()).into_bytes();
    bytes.extend_from_slice(payload);
    bytes
}

fn collect(actions: Vec<Action>, payload: &mut Vec<u8>, control: &mut Vec<u8>, up: bool) {
    for action in actions {
        match action {
            Action::ContentToServer(bytes) if up => payload.extend(bytes),
            Action::ContentToClient(bytes) if !up => payload.extend(bytes),
            Action::ToServer(bytes) if up => control.extend(bytes),
            Action::ToClient(bytes) if !up => control.extend(bytes),
            other => panic!("unexpected output {other:?}"),
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn data_bodies_alone_are_content_in_both_directions(tc: TestCase) {
    let bytes = tc.draw(gs::binary().max_size(1024));
    let chunk = tc.draw(gs::integers::<usize>().min_value(1).max_value(64));
    for up in [false, true] {
        let mut filter = filter();
        let request = if up {
            b"PUT file key\n".as_slice()
        } else {
            b"GET 0 file key\n".as_slice()
        };
        let _ = filter.from_client(request);
        let mut content = Vec::new();
        let mut control = Vec::new();
        for chunk in wire(&bytes).chunks(chunk) {
            let actions = if up {
                filter.from_client(chunk)
            } else {
                filter.from_server(chunk)
            };
            collect(actions, &mut content, &mut control, up);
        }
        assert_eq!(content, bytes);
        assert_eq!(control, format!("DATA {}\n", bytes.len()).as_bytes());
        assert!(!filter.is_closed());
    }
}

#[hegel::test(test_cases = 1000)]
fn history_and_its_framed_payload_are_never_content(tc: TestCase) {
    let bytes = tc.draw(gs::binary().max_size(1024));
    let chunk = tc.draw(gs::integers::<usize>().min_value(1).max_value(64));
    for up in [false, true] {
        let mut filter = filter();
        let _ = filter.from_client(b"CONNECT git-upload-pack\n");
        let mut content = Vec::new();
        let mut control = Vec::new();
        let wire = wire(&bytes);
        for chunk in wire.chunks(chunk) {
            let actions = if up {
                filter.from_client(chunk)
            } else {
                filter.from_server(chunk)
            };
            collect(actions, &mut content, &mut control, up);
        }
        assert!(content.is_empty());
        assert_eq!(control, wire);
        assert!(!filter.is_closed());
    }
}
