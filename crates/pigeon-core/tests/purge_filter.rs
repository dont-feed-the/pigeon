//! Purge transport laws (08 §8.6, 09 §9.4): neither released nor pending
//! keys start a transfer; policy changes stop an active transfer; history
//! traffic remains framed and uninterpreted so peers can learn new refs.
use std::sync::Arc;

use hegel::{TestCase, generators as gs};
use pigeon_core::filter::{Action, Closed, Config, ContentPolicy, Filter, Role};

fn filter(tc: &TestCase) -> Filter {
    Filter::new(
        Config {
            role: if tc.draw(gs::booleans()) {
                Role::Custodian
            } else {
                Role::Laptop
            },
            auth_token: None,
            peer_uuid: None,
            peer_custodian: false,
        },
        false,
    )
}

#[hegel::test(test_cases = 1000)]
fn released_and_unresolved_keys_never_start_content_operations(tc: TestCase) {
    let key = format!("key-{}", tc.draw(gs::integers::<u32>()));
    let waiting = tc.draw(gs::booleans());
    let policy = Arc::new(ContentPolicy {
        waiting,
        released: [key.clone()].into(),
    });
    for command in [
        format!("GET 0 file {key}\n"),
        format!("PUT file {key}\n"),
        format!("CHECKPRESENT {key}\n"),
        format!("LOCKCONTENT {key}\n"),
    ] {
        let mut f = filter(&tc);
        f.set_content_policy(policy.clone());
        let mut actions = Vec::new();
        for byte in command.bytes() {
            actions.extend(f.from_client(&[byte]));
        }
        assert_eq!(
            actions,
            [Action::Close(if waiting {
                Closed::PurgePending
            } else {
                Closed::Erased
            })]
        );
        assert!(f.from_server(b"SUCCESS\n").is_empty());
    }
}

#[hegel::test(test_cases = 1000)]
fn a_new_release_stops_an_active_get_before_more_payload_is_forwarded(tc: TestCase) {
    let key = format!("key-{}", tc.draw(gs::integers::<u32>()));
    let mut f = filter(&tc);
    assert_eq!(
        f.from_client(format!("GET 0 file {key}\n").as_bytes())
            .len(),
        1
    );
    assert!(!f.from_server(b"DATA 8\nfirst").is_empty());
    f.set_content_policy(Arc::new(ContentPolicy {
        waiting: false,
        released: [key.clone()].into(),
    }));
    assert_eq!(f.from_server(b"end"), [Action::Close(Closed::Erased)]);
    let mut later = filter(&tc);
    later.set_content_policy(Arc::new(ContentPolicy::default()));
    assert_eq!(
        later.from_client(format!("GET 0 file {key}\n").as_bytes()),
        [Action::ToServer(format!("GET 0 file {key}\n").into_bytes())]
    );
}

#[hegel::test(test_cases = 1000)]
fn pending_purge_checks_preserve_metadata_and_do_not_parse_its_payload(tc: TestCase) {
    let mut f = filter(&tc);
    f.set_content_policy(Arc::new(ContentPolicy {
        waiting: true,
        released: Default::default(),
    }));
    assert_eq!(
        f.from_client(b"CONNECT git-upload-pack\n"),
        [Action::ToServer(b"CONNECT git-upload-pack\n".to_vec())]
    );
    let mut payload = tc.draw(gs::binary().max_size(64));
    payload.extend_from_slice(b"GET 0 file erased\nPUT file erased\n");
    let mut framed = format!("DATA {}\n", payload.len()).into_bytes();
    framed.extend_from_slice(&payload);
    let forwarded: Vec<u8> = f
        .from_client(&framed)
        .into_iter()
        .flat_map(|action| match action {
            Action::ToServer(bytes) => bytes,
            _ => Vec::new(),
        })
        .collect();
    assert_eq!(forwarded, framed);
    assert!(!f.is_closed());
}

#[hegel::test(test_cases = 1000)]
fn bounded_tracking_cannot_forget_an_active_laptop_transfer(tc: TestCase) {
    let mut f = Filter::new(
        Config {
            role: Role::Laptop,
            auth_token: None,
            peer_uuid: None,
            peer_custodian: false,
        },
        false,
    );
    let n = tc.draw(gs::integers::<u8>()) % 32 + 17;
    for i in 0..n {
        assert_eq!(
            f.from_client(format!("GET 0 file key-{i}\n").as_bytes())
                .len(),
            1
        );
    }
    assert!(!f.from_server(b"DATA 8\nfirst").is_empty());
    f.set_content_policy(Arc::new(ContentPolicy {
        waiting: false,
        released: [format!("key-{}", n - 1)].into(),
    }));
    // The bounded tracker no longer knows the exact active key. It must
    // reconnect conservatively rather than forwarding untracked content.
    assert_eq!(f.from_server(b"end"), [Action::Close(Closed::PurgePending)]);
}

#[hegel::test(test_cases = 1000)]
fn a_pending_reading_stops_an_active_put_before_more_bytes(tc: TestCase) {
    let key = format!("key-{}", tc.draw(gs::integers::<u32>()));
    let mut f = filter(&tc);
    assert_eq!(
        f.from_client(format!("PUT file {key}\n").as_bytes()).len(),
        1
    );
    assert!(!f.from_server(b"PUT-FROM 0\n").is_empty());
    assert!(!f.from_client(b"DATA 8\nfirst").is_empty());
    f.set_content_policy(Arc::new(ContentPolicy {
        waiting: true,
        released: Default::default(),
    }));
    assert_eq!(f.from_client(b"end"), [Action::Close(Closed::PurgePending)]);
    assert!(f.synced(0, true).is_empty());
}
