//! Health wire evidence (12 §12.2): optional inventories stay unknown,
//! integer counts, Unicode names and report freshness survive framing.
#![allow(clippy::unwrap_used, reason = "property-test fixtures")]

use hegel::{TestCase, generators as gs};
use pigeon_proto::api::{Level, Machine, State, StatusReport};
use pigeon_proto::health::{Counts, Disk, HealthReport, MachineHealth, Queue, RepairEpoch};
use pigeon_proto::{LineDecoder, Message, Request, Response, encode};

fn text(tc: &TestCase) -> String {
    tc.draw(gs::text().max_size(40))
}
fn n(tc: &TestCase) -> u64 {
    tc.draw(gs::integers::<u64>())
}
fn flag(tc: &TestCase) -> bool {
    tc.draw(gs::booleans())
}
fn counts(tc: &TestCase) -> Option<Counts> {
    flag(tc).then(|| Counts {
        missing: n(tc),
        erased: n(tc),
        local: n(tc),
        published: n(tc),
        uncertain: n(tc),
        protected: n(tc),
    })
}

/// Law: health survives arbitrarily split socket reads, with all unknown
/// values and signed-report metadata intact. Every run has 1000 cases.
#[hegel::test(test_cases = 1000)]
fn health_round_trips_without_inventing_evidence(tc: TestCase) {
    let report = HealthReport {
        group_id: text(&tc),
        group: text(&tc),
        at: text(&tc),
        status: StatusReport {
            headline: Message {
                id: text(&tc),
                args: Default::default(),
            },
            level: Level::NotChecked,
            state: State::Protected,
            versions: Vec::new(),
            not_shared: Vec::new(),
            paused: None,
            scan_only: None,
        },
        omitted_paths: n(&tc),
        current: counts(&tc),
        retained: counts(&tc),
        suggestions: flag(&tc).then(|| n(&tc)),
        endangered: n(&tc),
        machines: (0..tc.draw(gs::integers::<u8>().max_value(3)))
            .map(|_| MachineHealth {
                machine: Machine {
                    name: text(&tc),
                    device: text(&tc),
                    custodian: flag(&tc),
                    recovery: flag(&tc),
                    this_device: flag(&tc),
                    heard_at: flag(&tc).then(|| text(&tc)),
                },
                reported_at: flag(&tc).then(|| text(&tc)),
                report_fresh: flag(&tc),
                scrub: flag(&tc).then(|| pigeon_proto::health::Scrub {
                    last_pass_completed: flag(&tc).then(|| text(&tc)),
                    keys_failed: n(&tc),
                    remaining: flag(&tc).then(|| n(&tc)),
                    current_failures: flag(&tc).then(|| n(&tc)),
                    problem: flag(&tc).then(|| Message {
                        id: text(&tc),
                        args: Default::default(),
                    }),
                }),
                checking: flag(&tc),
                repository_lost: flag(&tc),
                disk: flag(&tc).then(|| Disk {
                    used: n(&tc),
                    free: n(&tc),
                    maxsize: n(&tc),
                    reserve: n(&tc),
                    full: flag(&tc),
                }),
                queue: flag(&tc).then(|| Queue {
                    send_bytes: n(&tc),
                    receive_bytes: n(&tc),
                    deferred_keys: n(&tc),
                    missing_bytes: n(&tc),
                }),
            })
            .collect(),
        repair_epochs: flag(&tc).then(|| {
            (0..tc.draw(gs::integers::<u8>().max_value(3)))
                .map(|_| RepairEpoch {
                    commit: text(&tc),
                    device: text(&tc),
                    name: text(&tc),
                    started_at: text(&tc),
                })
                .collect()
        }),
    };
    let response = Response::Health(Box::new(report));
    let bytes = encode(27, &response).unwrap();
    let mut decoder = LineDecoder::new();
    let size = usize::from(tc.draw(gs::integers::<u8>().min_value(1)));
    for chunk in bytes.chunks(size) {
        decoder.push(chunk);
    }
    assert_eq!(
        decoder.next_line::<Response>().unwrap().unwrap().body,
        response
    );
    assert!(decoder.into_rest().is_empty());
    let mut decoder = LineDecoder::new();
    decoder.push(&encode(28, &Request::Health).unwrap());
    assert_eq!(
        decoder.next_line::<Request>().unwrap().unwrap().body,
        Request::Health
    );
}
