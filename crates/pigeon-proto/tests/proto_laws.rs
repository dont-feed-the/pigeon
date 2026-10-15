//! Laws of the socket framing and the stream header (09 §9.3, §9.8), as
//! hegel-rust property tests (ADR 0012): every request and response
//! survives encoding and any chunking, and a raw stream that follows the
//! last line is handed back byte for byte.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::BTreeMap;

use hegel::TestCase;
use hegel::generators as gs;
use pigeon_proto::api::{Level, Machine, State, StatusReport, Suggestion, VersionStatus};
use pigeon_proto::stream::{Refusal, StreamAnswer, StreamHeader};
use pigeon_proto::{Arg, Envelope, LineDecoder, Message, Problem, Request, Response, encode};

fn text(tc: &TestCase) -> String {
    tc.draw(gs::text().max_size(12))
}

fn maybe_text(tc: &TestCase) -> Option<String> {
    tc.draw(gs::booleans()).then(|| text(tc))
}

fn flag(tc: &TestCase) -> bool {
    tc.draw(gs::booleans())
}

fn request(tc: &TestCase) -> Request {
    match tc.draw(gs::integers::<u8>().max_value(21)) {
        0 => Request::Hello {
            version: tc.draw(gs::integers::<u32>()),
        },
        1 => Request::Status {
            path: maybe_text(tc),
            not_shared: flag(tc),
        },
        2 => Request::Create {
            name: text(tc),
            folder: text(tc),
            k: tc.draw(gs::integers::<u32>()),
            device_name: text(tc),
        },
        3 => Request::Invite {
            person: text(tc),
            admin: flag(tc),
            custodian: flag(tc),
            name: maybe_text(tc),
        },
        4 => Request::Join {
            invitation: text(tc),
            folder: maybe_text(tc),
            keep: flag(tc),
        },
        5 => Request::Keep { folder: text(tc) },
        6 => Request::Evict { folder: text(tc) },
        7 => Request::History { path: text(tc) },
        8 => Request::Restore {
            path: text(tc),
            version: tc
                .draw(gs::booleans())
                .then(|| tc.draw(gs::integers::<u64>())),
            at: maybe_text(tc),
        },
        9 => Request::Machines,
        10 => Request::RemoveDevice { name: text(tc) },
        11 => Request::RecoverySheet,
        12 => Request::Recover {
            sheet: text(tc),
            folder: maybe_text(tc),
            from: maybe_text(tc),
        },
        13 => Request::Doctor { details: flag(tc) },
        14 => Request::DebugBundle {
            peers: (0..tc.draw(gs::integers::<u8>().max_value(3)))
                .map(|_| text(tc))
                .collect(),
        },
        15 => Request::Address {
            repository: text(tc),
        },
        16 => Request::Connect {
            repository: text(tc),
            device: text(tc),
        },
        18 => Request::Proposals,
        19 => Request::Accept {
            proposal: text(tc),
            reason: maybe_text(tc),
        },
        20 => Request::Reject {
            proposal: text(tc),
            note: maybe_text(tc),
        },
        _ => Request::Listen {
            repository: text(tc),
            socket: text(tc),
        },
    }
}

fn message(tc: &TestCase) -> Message {
    let mut args = BTreeMap::new();
    for _ in 0..tc.draw(gs::integers::<u8>().max_value(3)) {
        let arg = if flag(tc) {
            Arg::Number(tc.draw(gs::integers::<i64>()))
        } else {
            Arg::Text(text(tc))
        };
        args.insert(text(tc), arg);
    }
    Message { id: text(tc), args }
}

fn state(tc: &TestCase) -> State {
    match tc.draw(gs::integers::<u8>().max_value(4)) {
        0 => State::Local,
        1 => State::Published,
        2 => State::Protected,
        3 => State::Missing,
        _ => State::Erased,
    }
}

fn version(tc: &TestCase) -> VersionStatus {
    VersionStatus {
        path: text(tc),
        number: tc.draw(gs::integers::<u64>()),
        saved_at: text(tc),
        state: state(tc),
        observed_at: maybe_text(tc),
        reason: flag(tc).then(|| message(tc)),
    }
}

fn response(tc: &TestCase) -> Response {
    match tc.draw(gs::integers::<u8>().max_value(11)) {
        0 => Response::Hello {
            version: tc.draw(gs::integers::<u32>()),
        },
        1 => Response::Done,
        2 => Response::Said {
            message: message(tc),
        },
        3 => Response::Problem(Problem {
            code: text(tc),
            message: message(tc),
            hint: flag(tc).then(|| message(tc)),
            details: maybe_text(tc),
        }),
        4 => Response::Progress {
            message: message(tc),
            done: tc
                .draw(gs::booleans())
                .then(|| tc.draw(gs::integers::<u64>())),
            total: tc
                .draw(gs::booleans())
                .then(|| tc.draw(gs::integers::<u64>())),
        },
        5 => Response::Status(StatusReport {
            headline: message(tc),
            level: match tc.draw(gs::integers::<u8>().max_value(4)) {
                0 => Level::NeedsYou,
                1 => Level::NotSafe,
                2 => Level::NotChecked,
                3 => Level::Working,
                _ => Level::Safe,
            },
            state: state(tc),
            versions: (0..tc.draw(gs::integers::<u8>().max_value(3)))
                .map(|_| version(tc))
                .collect(),
            not_shared: Vec::new(),
        }),
        6 => Response::Machines {
            machines: (0..tc.draw(gs::integers::<u8>().max_value(3)))
                .map(|_| Machine {
                    name: text(tc),
                    device: text(tc),
                    custodian: flag(tc),
                    heard_at: maybe_text(tc),
                    this_device: flag(tc),
                })
                .collect(),
        },
        7 => Response::History {
            versions: (0..tc.draw(gs::integers::<u8>().max_value(3)))
                .map(|_| version(tc))
                .collect(),
        },
        8 => Response::Address { device: text(tc) },
        9 => Response::Connected,
        10 => Response::Proposals {
            proposals: (0..tc.draw(gs::integers::<u8>().max_value(3)))
                .map(|_| Suggestion {
                    id: text(tc),
                    author: text(tc),
                    device: text(tc),
                    saved_at: text(tc),
                    paths: (0..tc.draw(gs::integers::<u8>().max_value(3)))
                        .map(|_| text(tc))
                        .collect(),
                    decide: flag(tc),
                    force: flag(tc),
                    mine: flag(tc),
                })
                .collect(),
        },
        _ => Response::Listening,
    }
}

/// Cuts `bytes` into random chunks, keeping their order.
fn chunks(tc: &TestCase, bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let n = 1 + usize::from(tc.draw(gs::integers::<u8>().max_value(40)));
        let (a, b) = rest.split_at(n.min(rest.len()));
        out.push(a.to_vec());
        rest = b;
    }
    out
}

/// Law *one line*: an encoded message is one line, its only newline last.
#[hegel::test]
fn an_envelope_is_one_line(tc: TestCase) {
    let line = encode(tc.draw(gs::integers::<u64>()), &request(&tc)).unwrap();
    assert_eq!(line.iter().filter(|&&b| b == b'\n').count(), 1);
    assert_eq!(line.last(), Some(&b'\n'));
}

/// Law *round trip*: any sequence of requests, encoded and cut into any
/// reads, decodes to the same requests with the same ids, in order.
#[hegel::test]
fn requests_round_trip(tc: TestCase) {
    let sent: Vec<Envelope<Request>> = (0..tc.draw(gs::integers::<u8>().max_value(6)))
        .map(|i| Envelope {
            id: u64::from(i),
            body: request(&tc),
        })
        .collect();
    let bytes: Vec<u8> = sent
        .iter()
        .flat_map(|e| encode(e.id, &e.body).unwrap())
        .collect();
    let mut decoder = LineDecoder::new();
    let mut got = Vec::new();
    for c in chunks(&tc, &bytes) {
        decoder.push(&c);
        while let Some(e) = decoder.next_line::<Request>().unwrap() {
            got.push(e);
        }
    }
    assert_eq!(got, sent);
    assert!(decoder.into_rest().is_empty());
}

/// Law *raw rest*: responses up to `Connected`, then raw bytes: the decoder
/// returns those responses, and what it read past them plus what was not
/// yet read is exactly the raw stream.
#[hegel::test]
fn raw_bytes_after_the_last_line_are_kept(tc: TestCase) {
    let mut sent: Vec<Response> = (0..tc.draw(gs::integers::<u8>().max_value(4)))
        .map(|_| response(&tc))
        .filter(|r| *r != Response::Connected)
        .collect();
    sent.push(Response::Connected);
    let raw = tc.draw(gs::binary().max_size(200));
    let mut bytes: Vec<u8> = sent.iter().flat_map(|r| encode(7, r).unwrap()).collect();
    bytes.extend_from_slice(&raw);
    let pieces = chunks(&tc, &bytes);
    let mut decoder = LineDecoder::new();
    let mut got = Vec::new();
    let mut read = 0;
    'reading: for c in &pieces {
        decoder.push(c);
        read += 1;
        while let Some(e) = decoder.next_line::<Response>().unwrap() {
            let connected = e.body == Response::Connected;
            got.push(e.body);
            if connected {
                break 'reading;
            }
        }
    }
    assert_eq!(got, sent);
    let mut rest = decoder.into_rest();
    for c in &pieces[read..] {
        rest.extend_from_slice(c);
    }
    assert_eq!(rest, raw);
}

/// Law *responses round trip*: any response decodes to itself.
#[hegel::test]
fn responses_round_trip(tc: TestCase) {
    let r = response(&tc);
    let mut decoder = LineDecoder::new();
    decoder.push(&encode(3, &r).unwrap());
    assert_eq!(
        decoder.next_line::<Response>().unwrap(),
        Some(Envelope { id: 3, body: r })
    );
}

/// Law *header*: a stream header and its answer survive their line.
#[hegel::test]
fn stream_headers_round_trip(tc: TestCase) {
    let header = StreamHeader {
        v: 1,
        group: text(&tc),
        traceparent: maybe_text(&tc),
        addresses: (0..tc.draw(gs::integers::<u8>().max_value(2)))
            .map(|_| text(&tc))
            .collect(),
    };
    let line = header.to_line();
    assert_eq!(line.iter().filter(|&&b| b == b'\n').count(), 1);
    let back: StreamHeader = serde_json::from_slice(&line).unwrap();
    assert_eq!(back, header);
    let answer = if flag(&tc) {
        StreamAnswer::accept()
    } else {
        StreamAnswer::refuse(Refusal::NotInRoster)
    };
    let back: StreamAnswer = serde_json::from_slice(&answer.to_line()).unwrap();
    assert_eq!(back, answer);
}
