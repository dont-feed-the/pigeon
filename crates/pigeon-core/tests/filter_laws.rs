//! Laws of the P2P filter (09 §9.4; ADRs 0012, 0043, 0054), as hegel-rust
//! property tests. Each law's statement is in its doc comment; the
//! reference framing in this file is written independently of the filter's.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]

use hegel::TestCase;
use hegel::generators as gs;
use pigeon_core::filter::{Action, Closed, Config, Filter, MAX_VERSION, Role, Rule};

fn config(role: Role) -> Config {
    Config {
        role,
        auth_token: None,
        peer_uuid: None,
        peer_custodian: false,
    }
}

/// A random key, from a small set so that keys repeat.
fn key(tc: &TestCase) -> String {
    format!(
        "SHA256E-s1--k{}",
        tc.draw(gs::integers::<u8>().max_value(3))
    )
}

/// Random payload bytes, often holding protocol lines.
fn payload(tc: &TestCase) -> Vec<u8> {
    let mut bytes = tc.draw(gs::binary().max_size(40));
    if tc.draw(gs::booleans()) {
        bytes.extend_from_slice(b"\nREMOVE k\nCONNECT git-receive-pack\nDATA 9\n");
    }
    bytes
}

/// One message of an allowed client stream, as bytes.
fn allowed_message(tc: &TestCase) -> Vec<u8> {
    let k = key(tc);
    match tc.draw(gs::integers::<u8>().max_value(11)) {
        0 => format!(
            "VERSION {}\n",
            tc.draw(gs::integers::<u64>().max_value(MAX_VERSION))
        )
        .into_bytes(),
        1 => format!("CHECKPRESENT {k}\n").into_bytes(),
        2 => format!("LOCKCONTENT {k}\n").into_bytes(),
        3 => b"UNLOCKCONTENT\n".to_vec(),
        4 => format!("GET 0 f {k}\n").into_bytes(),
        5 => format!("PUT f {k}\n").into_bytes(),
        6 => {
            let p = payload(tc);
            let mut m = format!("DATA {}\n", p.len()).into_bytes();
            m.extend_from_slice(&p);
            m
        }
        7 => b"VALID\n".to_vec(),
        8 => b"SUCCESS\n".to_vec(),
        9 => b"GETTIMESTAMP\n".to_vec(),
        10 => b"NOTIFYCHANGE\n".to_vec(),
        _ => b"AUTH 1234 token\n".to_vec(),
    }
}

/// One message of any client stream: allowed, refused or malformed.
fn any_message(tc: &TestCase) -> Vec<u8> {
    let k = key(tc);
    match tc.draw(gs::integers::<u8>().max_value(7)) {
        0 => format!("REMOVE {k}\n").into_bytes(),
        1 => format!("REMOVE-BEFORE 12 {k}\n").into_bytes(),
        2 => b"CONNECT git-receive-pack\n".to_vec(),
        3 => b"CONNECT git-upload-pack\n".to_vec(),
        4 => b"FROBNICATE\n".to_vec(),
        5 => b"DATA 1x\n".to_vec(),
        _ => allowed_message(tc),
    }
}

/// Cuts `bytes` into random chunks, keeping their order.
fn chunks(tc: &TestCase, bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let n = 1 + usize::from(tc.draw(gs::integers::<u8>().max_value(15)));
        let (a, b) = rest.split_at(n.min(rest.len()));
        out.push(a.to_vec());
        rest = b;
    }
    out
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Outputs {
    to_server: Vec<u8>,
    to_client: Vec<u8>,
    refused: Vec<(Rule, Vec<u8>)>,
    closed: Option<Closed>,
}

fn collect(actions: Vec<Action>, out: &mut Outputs) {
    for a in actions {
        assert!(out.closed.is_none(), "an action after Close: {a:?}");
        match a {
            Action::ToServer(b) => out.to_server.extend(b),
            Action::ToClient(b) => out.to_client.extend(b),
            Action::Refused { rule, line } => out.refused.push((rule, line)),
            Action::Close(why) => out.closed = Some(why),
            Action::Sync { .. } | Action::Repair { .. } => {}
        }
    }
}

/// The lines of a stream, as git-annex frames it, payloads skipped.
fn lines(stream: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = stream;
    while let Some(i) = rest.iter().position(|&b| b == b'\n') {
        let line = rest[..i].to_vec();
        rest = &rest[i + 1..];
        if let Some(n) = line.strip_prefix(b"DATA ") {
            let n: usize = String::from_utf8_lossy(n).parse().unwrap_or(0);
            rest = &rest[n.min(rest.len())..];
        }
        out.push(line);
    }
    out
}

/// Law *passthrough*: on a laptop, a stream of allowed messages reaches
/// git-annex unchanged, byte for byte, whatever its chunking.
#[hegel::test]
fn allowed_streams_pass_unchanged(tc: TestCase) {
    let stream: Vec<u8> = (0..tc.draw(gs::integers::<u8>().max_value(12)))
        .flat_map(|_| allowed_message(&tc))
        .collect();
    let mut f = Filter::new(config(Role::Laptop), false);
    let mut out = Outputs::default();
    for c in chunks(&tc, &stream) {
        collect(f.from_client(&c), &mut out);
    }
    assert_eq!(out.closed, None);
    assert_eq!(out.to_server, stream);
}

/// Law *chunking*: what the filter does to a client stream does not depend
/// on how the stream is cut into reads.
#[hegel::test]
fn chunking_does_not_matter(tc: TestCase) {
    let role = if tc.draw(gs::booleans()) {
        Role::Custodian
    } else {
        Role::Laptop
    };
    let stream: Vec<u8> = (0..tc.draw(gs::integers::<u8>().max_value(10)))
        .flat_map(|_| any_message(&tc))
        .collect();
    let (mut whole, mut cut) = (
        Filter::new(config(role), false),
        Filter::new(config(role), false),
    );
    let (mut a, mut b) = (Outputs::default(), Outputs::default());
    collect(whole.from_client(&stream), &mut a);
    for c in chunks(&tc, &stream) {
        collect(cut.from_client(&c), &mut b);
    }
    assert_eq!(a, b);
}

/// Law *no removal, no push*: on a custodian, no `REMOVE`, `REMOVE-BEFORE`
/// or `CONNECT` other than `git-upload-pack` ever reaches git-annex, and
/// every allowed `VERSION` that reaches it is at most 4.
#[hegel::test]
fn custodians_never_forward_removals_or_pushes(tc: TestCase) {
    let stream: Vec<u8> = (0..tc.draw(gs::integers::<u8>().max_value(12)))
        .flat_map(|_| {
            if tc.draw(gs::booleans()) {
                format!("VERSION {}\n", tc.draw(gs::integers::<u32>())).into_bytes()
            } else {
                any_message(&tc)
            }
        })
        .collect();
    let mut f = Filter::new(config(Role::Custodian), false);
    let mut out = Outputs::default();
    for c in chunks(&tc, &stream) {
        collect(f.from_client(&c), &mut out);
    }
    for line in lines(&out.to_server) {
        assert!(!line.starts_with(b"REMOVE"), "forwarded {line:?}");
        assert!(!line.starts_with(b"CONNECT") || line == b"CONNECT git-upload-pack");
        if let Some(v) = line.strip_prefix(b"VERSION ") {
            assert!(String::from_utf8_lossy(v).parse::<u64>().unwrap() <= MAX_VERSION);
        }
    }
    let removals = lines(&stream)
        .iter()
        .take_while(|l| !l.starts_with(b"CONNECT"))
        .filter(|l| l.starts_with(b"REMOVE"))
        .count();
    if out.closed.is_none() {
        assert_eq!(
            out.refused
                .iter()
                .filter(|(r, _)| *r == Rule::Removal)
                .count(),
            removals
        );
    }
}

/// Law *bounded lines*: a line over 4096 bytes or a malformed `DATA` length
/// closes the session, and nothing of it reaches git-annex.
#[hegel::test]
fn malformed_input_closes(tc: TestCase) {
    let bad = if tc.draw(gs::booleans()) {
        vec![b'A'; 4097 + usize::from(tc.draw(gs::integers::<u8>()))]
    } else {
        format!("DATA {}x\n", tc.draw(gs::integers::<u16>())).into_bytes()
    };
    let mut f = Filter::new(config(Role::Laptop), false);
    let mut out = Outputs::default();
    for c in chunks(&tc, &bad) {
        collect(f.from_client(&c), &mut out);
    }
    assert!(matches!(
        out.closed,
        Some(Closed::LineTooLong | Closed::BadData)
    ));
    assert!(out.to_server.is_empty());
    assert!(f.is_closed() && f.from_client(b"VERSION 1\n").is_empty());
}

/// The requests of a custodian session, each answered before the next.
#[derive(Clone, Debug)]
enum Request {
    CheckPresent(String),
    Lock(String),
    Put(String),
    Get(String),
    Remove(String),
}

/// A custodian session against a model of git-annex that answers at
/// random, with syncs that complete at random times and hashes that match
/// at random.
struct Session<'a> {
    tc: &'a TestCase,
    filter: Filter,
    to_server: Vec<u8>,
    to_client: Vec<u8>,
    pending: Vec<u64>,
    tickets: Vec<(u64, String, bool)>,
    outcomes: Vec<(u64, bool)>,
    repairs: Vec<String>,
    closed: bool,
}

impl Session<'_> {
    fn apply(&mut self, actions: Vec<Action>) {
        for a in actions {
            match a {
                Action::ToServer(b) => self.to_server.extend(b),
                Action::ToClient(b) => self.to_client.extend(b),
                Action::Sync { ticket, key, hash } => {
                    self.pending.push(ticket);
                    self.tickets.push((ticket, key, hash));
                }
                Action::Repair { key } => self.repairs.push(key),
                Action::Refused { .. } => {}
                Action::Close(_) => self.closed = true,
            }
        }
    }

    fn client(&mut self, bytes: &[u8]) {
        for c in chunks(self.tc, bytes) {
            let a = self.filter.from_client(&c);
            self.apply(a);
        }
    }

    fn server(&mut self, bytes: &[u8]) {
        for c in chunks(self.tc, bytes) {
            let a = self.filter.from_server(&c);
            self.apply(a);
            self.maybe_sync();
        }
    }

    /// Completes the oldest pending sync; a hash, if asked for, matches or
    /// not at random.
    fn complete(&mut self) {
        let t = self.pending.remove(0);
        let hash = self.tickets.iter().any(|(u, _, h)| *u == t && *h);
        let matched = !hash || self.tc.draw(gs::booleans());
        self.outcomes.push((t, matched));
        let a = self.filter.synced(t, matched);
        self.apply(a);
    }

    /// Completes some pending syncs, chosen at random.
    fn maybe_sync(&mut self) {
        while !self.pending.is_empty() && self.tc.draw(gs::booleans()) {
            self.complete();
        }
    }

    /// What git-annex received since the last call.
    fn server_lines(&mut self) -> Vec<Vec<u8>> {
        lines(&std::mem::take(&mut self.to_server))
    }

    /// The client's answer: syncs complete until it arrives.
    fn answer(&mut self) -> Vec<u8> {
        loop {
            if let Some(i) = self.to_client.iter().position(|&b| b == b'\n') {
                let line = self.to_client[..i].to_vec();
                self.to_client.drain(..=i);
                return line;
            }
            assert!(
                !self.pending.is_empty() && !self.closed,
                "no answer and nothing pending"
            );
            self.complete();
        }
    }

    /// The one sync asked for since `since`, for `key`: it has completed,
    /// so a positive answer may leave; its hash flag and outcome.
    fn sync_since(&self, key: &str, since: usize) -> (bool, bool) {
        assert_eq!(
            self.tickets.len(),
            since + 1,
            "one sync per positive answer"
        );
        let (t, k, hash) = &self.tickets[since];
        assert_eq!(k, key);
        let outcome = self.outcomes.iter().find(|(u, _)| u == t);
        let (_, matched) = outcome.expect("the answer left before its sync");
        (*hash, *matched)
    }
}

/// Laws *sync before success*, *probe* and *hash* (ADR 0054): on a
/// custodian, whatever git-annex answers and whenever syncs complete,
/// (1) a `SUCCESS` or `ALREADY-HAVE` for a key reaches the client only after
/// a sync of that key asked for after git-annex's answer; (2) `CHECKPRESENT
/// k` reaches git-annex as `LOCKCONTENT k`, is `SUCCESS` only if that lock
/// was granted, and the lock is released before the answer; (3) a lock or
/// probe for another custodian is `SUCCESS` only if the object's hash
/// matched, and a mismatch releases the lock and asks for a repair; a
/// laptop's is never hashed; (4) while checking, no lock or probe reaches
/// git-annex and both answer `FAILURE`; (5) every request is answered once
/// its syncs complete; (6) no removal reaches git-annex.
#[hegel::test]
fn custodian_answers_only_after_sync(tc: TestCase) {
    let checking = tc.draw(gs::booleans());
    let peer_custodian = tc.draw(gs::booleans());
    let mut config = config(Role::Custodian);
    config.peer_custodian = peer_custodian;
    let mut s = Session {
        tc: &tc,
        filter: Filter::new(config, checking),
        to_server: Vec::new(),
        to_client: Vec::new(),
        pending: Vec::new(),
        tickets: Vec::new(),
        outcomes: Vec::new(),
        repairs: Vec::new(),
        closed: false,
    };
    s.client(b"VERSION 4\n");
    assert_eq!(s.server_lines(), vec![b"VERSION 4".to_vec()]);
    s.server(b"VERSION 4\n");
    assert_eq!(s.answer(), b"VERSION 4");
    for _ in 0..tc.draw(gs::integers::<u8>().max_value(8)) {
        let k = key(&tc);
        let request = match tc.draw(gs::integers::<u8>().max_value(4)) {
            0 => Request::CheckPresent(k),
            1 => Request::Lock(k),
            2 => Request::Put(k),
            3 => Request::Get(k),
            _ => Request::Remove(k),
        };
        let since = s.tickets.len();
        let repairs = s.repairs.len();
        let yes = tc.draw(gs::booleans());
        match &request {
            Request::CheckPresent(k) | Request::Lock(k) => {
                let lock = matches!(request, Request::Lock(_));
                let name = if lock { "LOCKCONTENT" } else { "CHECKPRESENT" };
                s.client(format!("{name} {k}\n").as_bytes());
                let got = s.server_lines();
                if checking {
                    assert!(got.is_empty());
                    assert_eq!(s.answer(), b"FAILURE");
                    continue;
                }
                assert_eq!(got, vec![format!("LOCKCONTENT {k}").into_bytes()]);
                s.server(if yes { b"SUCCESS\n" } else { b"FAILURE\n" });
                let answer = s.answer();
                if !yes {
                    assert_eq!(answer, b"FAILURE");
                    assert!(s.server_lines().is_empty());
                    continue;
                }
                let (hash, matched) = s.sync_since(k, since);
                assert_eq!(hash, peer_custodian, "hashed exactly for custodians");
                if matched {
                    assert_eq!(answer, b"SUCCESS");
                    if lock {
                        assert!(s.server_lines().is_empty(), "the client holds the lock");
                        s.client(b"UNLOCKCONTENT\n");
                    }
                } else {
                    assert_eq!(answer, b"FAILURE");
                    assert_eq!(&s.repairs[repairs..], std::slice::from_ref(k));
                }
                assert_eq!(s.server_lines(), vec![b"UNLOCKCONTENT".to_vec()]);
            }
            Request::Put(k) => {
                s.client(format!("PUT f {k}\n").as_bytes());
                assert_eq!(s.server_lines(), vec![format!("PUT f {k}").into_bytes()]);
                if tc.draw(gs::booleans()) {
                    s.server(b"ALREADY-HAVE\n");
                    assert_eq!(s.answer(), b"ALREADY-HAVE");
                    assert_eq!(s.sync_since(k, since), (false, true));
                    continue;
                }
                s.server(b"PUT-FROM 0\n");
                assert_eq!(s.answer(), b"PUT-FROM 0");
                let p = payload(&tc);
                let mut data = format!("DATA {}\n", p.len()).into_bytes();
                data.extend_from_slice(&p);
                data.extend_from_slice(b"VALID\n");
                s.client(&data);
                assert_eq!(std::mem::take(&mut s.to_server), data);
                s.server(if yes { b"SUCCESS\n" } else { b"FAILURE\n" });
                let answer = s.answer();
                assert_eq!(
                    answer,
                    if yes {
                        b"SUCCESS".to_vec()
                    } else {
                        b"FAILURE".to_vec()
                    }
                );
                if yes {
                    assert_eq!(s.sync_since(k, since), (false, true));
                }
            }
            Request::Get(k) => {
                s.client(format!("GET 0 f {k}\n").as_bytes());
                assert_eq!(s.server_lines(), vec![format!("GET 0 f {k}").into_bytes()]);
                let p = payload(&tc);
                let mut data = format!("DATA {}\n", p.len()).into_bytes();
                data.extend_from_slice(&p);
                data.extend_from_slice(b"VALID\n");
                s.server(&data);
                assert_eq!(std::mem::take(&mut s.to_client), data);
                s.client(b"SUCCESS\n");
                assert_eq!(s.server_lines(), vec![b"SUCCESS".to_vec()]);
            }
            Request::Remove(k) => {
                s.client(format!("REMOVE {k}\n").as_bytes());
                assert!(s.server_lines().is_empty());
                assert_eq!(s.answer(), b"FAILURE");
            }
        }
        assert!(!s.closed);
    }
}

/// Law *no unattributed success*: a custodian closes, rather than pass on,
/// a positive answer from git-annex that answers no request.
#[hegel::test]
fn unattributed_success_closes(tc: TestCase) {
    let line: &[u8] = if tc.draw(gs::booleans()) {
        b"SUCCESS\n"
    } else {
        b"ALREADY-HAVE\n"
    };
    let mut f = Filter::new(config(Role::Custodian), false);
    let mut out = Outputs::default();
    collect(f.from_server(line), &mut out);
    assert_eq!(out.closed, Some(Closed::Unattributed));
    assert!(out.to_client.is_empty());
}

/// Law *AUTH*: the client's token is replaced by the repository's own, and
/// an `AUTH` for another UUID than the roster's closes the session.
#[hegel::test]
fn auth_is_rewritten_and_checked(tc: TestCase) {
    let honest = tc.draw(gs::booleans());
    let mut f = Filter::new(
        Config {
            role: Role::Custodian,
            auth_token: Some("local".into()),
            peer_uuid: Some("u1".into()),
            peer_custodian: true,
        },
        false,
    );
    let mut out = Outputs::default();
    let line = if honest {
        "AUTH u1 whatever\n"
    } else {
        "AUTH u2 whatever\n"
    };
    for c in chunks(&tc, line.as_bytes()) {
        collect(f.from_client(&c), &mut out);
    }
    if honest {
        assert_eq!(out.to_server, b"AUTH u1 local\n");
    } else {
        assert_eq!(out.closed, Some(Closed::WrongPeer));
        assert!(out.to_server.is_empty());
    }
}
