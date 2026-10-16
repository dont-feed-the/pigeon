//! Admission laws (proposed ADR 0075): no empty or incomplete quorum,
//! exact capability and causal acknowledgement boundaries, and no sealing
//! an old decision across a pending or changed membership epoch.
#![allow(clippy::unwrap_used, reason = "admission law fixture")]
use hegel::{TestCase, generators as gs};
use pigeon_core::admission::{Ack, Kind, View, may_seal};
use std::collections::{BTreeMap, BTreeSet};

#[hegel::test(test_cases = 1000)]
fn every_required_ack_must_name_the_supported_protocol_and_both_causal_boundaries(tc: TestCase) {
    let n = tc.draw(gs::integers::<u8>()) % 16 + 1;
    let participants: BTreeSet<_> = (0..n).collect();
    let custodians = [0].into();
    let mut acks: BTreeMap<_, _> = participants
        .iter()
        .map(|d| {
            (
                *d,
                Ack {
                    main: 4,
                    protocol: 2,
                },
            )
        })
        .collect();
    let ready = |acks: &BTreeMap<_, _>, main| {
        View {
            main: &main,
            introduced: &2,
            kind: &Kind::Upgrade,
            custodians: &custodians,
            participants: &participants,
            acknowledgements: acks,
            unclosed_purges: false,
        }
        .ready(|tip, ancestor| tip >= ancestor)
    };
    assert!(ready(&acks, 5));
    let device = tc.draw(gs::integers::<u8>()) % n;
    let original = acks.remove(&device).unwrap();
    assert!(!ready(&acks, 5));
    for invalid in [
        Ack {
            main: 1,
            protocol: 2,
        },
        Ack {
            main: 4,
            protocol: 1,
        },
        Ack {
            main: 4,
            protocol: 3,
        },
    ] {
        acks.insert(device, invalid);
        assert!(!ready(&acks, 5));
    }
    acks.insert(device, original);
    assert!(!ready(&acks, 3));
}

#[hegel::test(test_cases = 1000)]
fn a_new_backup_needs_its_own_ack_and_empty_custody_requires_all_participants(tc: TestCase) {
    let n = tc.draw(gs::integers::<u8>()) % 16 + 1;
    let participants: BTreeSet<_> = (0..n).collect();
    let mut acks: BTreeMap<_, _> = participants
        .iter()
        .map(|d| {
            (
                *d,
                Ack {
                    main: 4,
                    protocol: 2,
                },
            )
        })
        .collect();
    let kind = Kind::Custodian(n);
    let mut custodians = BTreeSet::new();
    let view = |custodians: &BTreeSet<_>, acks: &BTreeMap<_, _>| {
        View {
            main: &5,
            introduced: &2,
            kind: &kind,
            custodians,
            participants: &participants,
            acknowledgements: acks,
            unclosed_purges: false,
        }
        .ready(|tip, ancestor| tip >= ancestor)
    };
    assert!(!view(&custodians, &acks));
    acks.insert(
        n,
        Ack {
            main: 4,
            protocol: 2,
        },
    );
    assert!(view(&custodians, &acks));
    let absent = tc.draw(gs::integers::<u8>()) % n;
    let ack = acks.remove(&absent).unwrap();
    assert!(!view(&custodians, &acks));
    acks.insert(absent, ack);
    custodians.insert(0);
    assert!(view(&custodians, &acks));
    let blocked = View {
        main: &5,
        introduced: &2,
        kind: &kind,
        custodians: &custodians,
        participants: &participants,
        acknowledgements: &acks,
        unclosed_purges: true,
    };
    assert!(!blocked.ready(|tip, ancestor| tip >= ancestor));
}

#[hegel::test(test_cases = 1000)]
fn an_upgrade_cannot_enable_erasure_without_a_custodian_or_a_participant(tc: TestCase) {
    let identity = tc.draw(gs::integers::<u8>());
    let participants = [identity].into();
    let acks = [(
        identity,
        Ack {
            main: 4,
            protocol: 2,
        },
    )]
    .into();
    let view = View {
        main: &5,
        introduced: &2,
        kind: &Kind::Upgrade,
        custodians: &BTreeSet::new(),
        participants: &participants,
        acknowledgements: &acks,
        unclosed_purges: false,
    };
    assert!(!view.ready(|tip, ancestor| tip >= ancestor));
    let custodians = [identity].into();
    let view = View {
        custodians: &custodians,
        participants: &BTreeSet::new(),
        ..view
    };
    assert!(!view.ready(|tip, ancestor| tip >= ancestor));
}

#[hegel::test(test_cases = 1000)]
fn a_pending_or_changed_quorum_cannot_seal_an_old_roster_decision(tc: TestCase) {
    let n = tc.draw(gs::integers::<u8>()) % 32 + 1;
    let original: BTreeSet<_> = (0..n).collect();
    assert!(may_seal(&original, &original, false));
    assert!(!may_seal(&original, &original, true));
    let mut enlarged = original.clone();
    enlarged.insert(n);
    assert!(!may_seal(&original, &enlarged, false));
    let mut reduced = original.clone();
    reduced.remove(&(tc.draw(gs::integers::<u8>()) % n));
    assert!(!may_seal(&original, &reduced, false));
    assert!(!may_seal::<u8>(&BTreeSet::new(), &BTreeSet::new(), false));
}
