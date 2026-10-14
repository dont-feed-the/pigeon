//! The filter between a peer and git-annex's `remotedaemon` (09 §9.4).
//!
//! A pure state machine over the two byte streams of one git-annex P2P
//! session (ADR 0014, spec 09 §9.4, 09 §9.2): the client's messages pass
//! an allow-list, `VERSION` is capped, removals are refused on custodians and
//! pushes everywhere, a custodian answers `CHECKPRESENT` with a lock probe,
//! a custodian's positive answer for a key waits until that object is synced
//! to disk, and a lock that proves another custodian's drop waits for a hash. Only the framing is parsed, so a payload passes by its
//! length whatever the read sizes. `pigeond` feeds it the bytes of each side
//! and carries out the [`Action`]s it returns, in order. Its laws are
//! property tests in `tests/filter_laws.rs` (ADR 0012).

use std::collections::VecDeque;

/// The highest P2P protocol version the filter knows: a client's
/// `VERSION n` is forwarded as `VERSION min(n, MAX_VERSION)`.
pub const MAX_VERSION: u64 = 4;

/// The longest protocol line, its newline excluded; a longer one closes.
pub const MAX_LINE: usize = 4096;

/// The most digits a `DATA` length may have.
const MAX_LENGTH_DIGITS: usize = 15;

/// The client's messages that are allowed, for protocol versions 0 to 4,
/// besides `CONNECT git-upload-pack` and, on laptops, the removals.
pub const ALLOWED: [&str; 18] = [
    "AUTH",
    "VERSION",
    "BYPASS",
    "CHECKPRESENT",
    "LOCKCONTENT",
    "UNLOCKCONTENT",
    "GET",
    "PUT",
    "DATA",
    "VALID",
    "INVALID",
    "DATA-PRESENT",
    "SUCCESS",
    "FAILURE",
    "GETTIMESTAMP",
    "NOTIFYCHANGE",
    "CONNECTDONE",
    "ERROR",
];

/// The only service a peer may `CONNECT` to: git fetches, never pushes.
pub const UPLOAD_PACK: &[u8] = b"CONNECT git-upload-pack";

/// The line that answers a refused `CONNECT`, in git-annex's own protocol.
pub const PUSH_REFUSED: &[u8] = b"ERROR pigeon: this device accepts no push\n";

/// Which side of the custody rules the serving device is on (ADR 0002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Serves append-only: no removal, positive answers after a sync.
    Custodian,
    /// Keeps git-annex's defaults, since a laptop's copies never count.
    Laptop,
}

/// What the filter knows of the session before it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// The serving device's role in the group.
    pub role: Role,
    /// The token this repository's git-annex accepts. When set, it replaces
    /// the token of the client's `AUTH`, since `pigeond` has authenticated
    /// the peer by its device key already (09 §9.2).
    pub auth_token: Option<String>,
    /// The git-annex UUID the roster gives the peer. When set, an `AUTH`
    /// naming another UUID closes the session.
    pub peer_uuid: Option<String>,
    /// Whether the peer is a custodian: then a custodian's lock for it, or
    /// its probe, is granted only for an object whose hash matched
    /// (spec 09 §9.4).
    pub peer_custodian: bool,
}

/// What `pigeond` must do, in the order given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Write these bytes to git-annex.
    ToServer(Vec<u8>),
    /// Write these bytes to the peer.
    ToClient(Vec<u8>),
    /// Sync the object of `key` and its directories to disk and, if `hash`,
    /// check its SHA-256 against the key; then call [`Filter::synced`] with
    /// `ticket` and whether the hash matched.
    Sync {
        /// Names this request in [`Filter::synced`].
        ticket: u64,
        /// The git-annex key whose object must be durable.
        key: String,
        /// Whether the object must also be hashed (spec 09 §9.4).
        hash: bool,
    },
    /// The object of `key` failed its hash: queue `git annex fsck --key`,
    /// which moves it out and starts repair (08 §8.4).
    Repair {
        /// The key whose object is rotten.
        key: String,
    },
    /// A message refused by the rules; logged at `error` with the peer.
    Refused {
        /// Which rule refused it.
        rule: Rule,
        /// The refused line, without its newline.
        line: Vec<u8>,
    },
    /// Close both directions. Nothing follows.
    Close(Closed),
}

/// The rules that refuse a message without closing the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// `REMOVE` or `REMOVE-BEFORE` on a custodian: answered `FAILURE`.
    Removal,
    /// A `CONNECT` to anything but `git-upload-pack`: answered `ERROR`.
    Push,
}

/// Why the filter closed a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closed {
    /// A line longer than [`MAX_LINE`] bytes.
    LineTooLong,
    /// A `DATA` line whose length is not 1 to 15 digits.
    BadData,
    /// A `VERSION` line whose version is not a number.
    BadVersion,
    /// An `AUTH`, `CHECKPRESENT`, `LOCKCONTENT` or `PUT` without its key.
    BadArguments,
    /// A message off the allow-list, or anything but data while relaying git.
    UnknownMessage,
    /// A `CONNECT` to anything but `git-upload-pack`.
    Push,
    /// An `AUTH` naming another git-annex UUID than the roster's.
    WrongPeer,
    /// A custodian's client sent a request before the last was answered.
    Pipelined,
    /// git-annex sent a positive answer that answers no request.
    Unattributed,
    /// git-annex's own output broke the framing.
    ServerFraming,
}

/// One unit of either stream, as the framing cuts it.
#[derive(Debug, PartialEq, Eq)]
enum Frame {
    /// A whole line, its newline excluded.
    Line(Vec<u8>),
    /// Some bytes of a `DATA` payload.
    Payload(Vec<u8>),
    /// The last byte of a `DATA` payload has passed.
    PayloadEnd,
}

/// git-annex's P2P framing for one direction: lines, or `DATA n` and n
/// raw bytes. It never looks inside a payload.
#[derive(Debug, Default)]
struct Framer {
    line: Vec<u8>,
    payload_left: u64,
}

impl Framer {
    fn push(&mut self, mut bytes: &[u8], frames: &mut Vec<Frame>) -> Result<(), Closed> {
        while !bytes.is_empty() {
            if self.payload_left > 0 {
                let n = usize::try_from(self.payload_left)
                    .unwrap_or(usize::MAX)
                    .min(bytes.len());
                let (chunk, rest) = bytes.split_at(n);
                frames.push(Frame::Payload(chunk.to_vec()));
                self.payload_left -= n as u64;
                if self.payload_left == 0 {
                    frames.push(Frame::PayloadEnd);
                }
                bytes = rest;
                continue;
            }
            match bytes.iter().position(|&b| b == b'\n') {
                None => {
                    self.line.extend_from_slice(bytes);
                    if self.line.len() > MAX_LINE {
                        return Err(Closed::LineTooLong);
                    }
                    bytes = &[];
                }
                Some(i) => {
                    self.line.extend_from_slice(&bytes[..i]);
                    bytes = &bytes[i + 1..];
                    if self.line.len() > MAX_LINE {
                        return Err(Closed::LineTooLong);
                    }
                    let line = std::mem::take(&mut self.line);
                    let length = data_length(&line)?;
                    frames.push(Frame::Line(line));
                    match length {
                        Some(0) => frames.push(Frame::PayloadEnd),
                        Some(n) => self.payload_left = n,
                        None => {}
                    }
                }
            }
        }
        Ok(())
    }
}

/// The length of a `DATA` line, or `None` for any other line.
fn data_length(line: &[u8]) -> Result<Option<u64>, Closed> {
    if word(line) != b"DATA" {
        return Ok(None);
    }
    let digits = line.get(5..).unwrap_or_default();
    if digits.is_empty()
        || digits.len() > MAX_LENGTH_DIGITS
        || !digits.iter().all(u8::is_ascii_digit)
    {
        return Err(Closed::BadData);
    }
    let mut n: u64 = 0;
    for d in digits {
        n = n * 10 + u64::from(d - b'0');
    }
    Ok(Some(n))
}

/// The first word of a line: its message name.
fn word(line: &[u8]) -> &[u8] {
    line.split(|&b| b == b' ').next().unwrap_or_default()
}

/// The words of a line after its message name.
fn arguments(line: &[u8]) -> Vec<&[u8]> {
    line.split(|&b| b == b' ').skip(1).collect()
}

/// What a custodian's filter waits for from git-annex.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Awaiting {
    /// Any one answer, passed on unchanged.
    Reply,
    /// The answer to `VERSION`, which sets the session's version.
    Version,
    /// The answer to the filter's own `LOCKCONTENT`, sent for `CHECKPRESENT`.
    Probe(String),
    /// The answer to the client's `LOCKCONTENT`.
    Lock(String),
    /// The answer to `PUT`: `PUT-FROM` or `ALREADY-HAVE`.
    Put(String),
    /// The answer once a `PUT`'s data has been sent.
    PutData(String),
    /// The data a `GET` asked for, up to its validity.
    Get,
}

/// Bytes on their way to the peer; a ticketed segment waits for its sync.
#[derive(Debug)]
struct Segment {
    ticket: Option<u64>,
    bytes: Vec<u8>,
}

/// What a held answer is, so that its sync's outcome can complete it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Held {
    /// git-annex granted the client's `LOCKCONTENT`; the client unlocks.
    Lock(String),
    /// git-annex granted the filter's own lock for a `CHECKPRESENT`; the
    /// filter unlocks once the sync is done.
    Probe(String),
    /// git-annex stored the object, or already had it.
    Put,
}

/// The filter of one session. See the module documentation.
#[derive(Debug)]
pub struct Filter {
    config: Config,
    checking: bool,
    closed: bool,
    relaying: bool,
    version: u64,
    client: Framer,
    server: Framer,
    awaiting: VecDeque<Awaiting>,
    outbox: VecDeque<Segment>,
    held: Vec<(u64, Held)>,
    next_ticket: u64,
}

impl Filter {
    /// A filter for a new session; `checking` is the repository's state
    /// (06 §6.3): while it is re-checked, no lock is granted.
    pub fn new(config: Config, checking: bool) -> Self {
        Filter {
            config,
            checking,
            closed: false,
            relaying: false,
            version: 0,
            client: Framer::default(),
            server: Framer::default(),
            awaiting: VecDeque::new(),
            outbox: VecDeque::new(),
            held: Vec::new(),
            next_ticket: 0,
        }
    }

    /// Whether the session is closed; then every call returns nothing.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// The repository starts or ends being re-checked (ADR 0046).
    pub fn set_checking(&mut self, checking: bool) {
        self.checking = checking;
    }

    /// Bytes from the peer.
    pub fn from_client(&mut self, bytes: &[u8]) -> Vec<Action> {
        let mut out = Vec::new();
        if self.closed {
            return out;
        }
        let mut frames = Vec::new();
        let framed = self.client.push(bytes, &mut frames);
        for frame in frames {
            if let Err(why) = self.client_frame(frame, &mut out) {
                self.close(why, &mut out);
                return out;
            }
        }
        if let Err(why) = framed {
            self.close(why, &mut out);
        }
        out
    }

    /// Bytes from git-annex.
    pub fn from_server(&mut self, bytes: &[u8]) -> Vec<Action> {
        let mut out = Vec::new();
        if self.closed {
            return out;
        }
        let mut frames = Vec::new();
        let framed = self.server.push(bytes, &mut frames);
        for frame in frames {
            if let Err(why) = self.server_frame(frame, &mut out) {
                self.close(why, &mut out);
                return out;
            }
        }
        if framed.is_err() {
            self.close(Closed::ServerFraming, &mut out);
        }
        out
    }

    /// The sync asked for with `ticket` has completed; `matched` says
    /// whether the hash matched, and is true when none was asked for. A
    /// failed sync is no answer: `pigeond` closes the session instead.
    pub fn synced(&mut self, ticket: u64, matched: bool) -> Vec<Action> {
        let mut out = Vec::new();
        if self.closed {
            return out;
        }
        let Some(i) = self.held.iter().position(|(t, _)| *t == ticket) else {
            return out;
        };
        let (_, held) = self.held.remove(i);
        let (unlock, key) = match &held {
            Held::Lock(k) => (!matched, Some(k.clone())),
            Held::Probe(k) => (true, Some(k.clone())),
            Held::Put => (false, None),
        };
        if unlock {
            out.push(Action::ToServer(b"UNLOCKCONTENT\n".to_vec()));
        }
        for segment in self.outbox.iter_mut() {
            if segment.ticket == Some(ticket) {
                segment.ticket = None;
                if !matched {
                    segment.bytes = b"FAILURE\n".to_vec();
                }
            }
        }
        if let (false, Some(key)) = (matched, key) {
            out.push(Action::Repair { key });
        }
        self.release(&mut out);
        out
    }

    fn close(&mut self, why: Closed, out: &mut Vec<Action>) {
        self.closed = true;
        self.outbox.clear();
        out.push(Action::Close(why));
    }

    fn custodian(&self) -> bool {
        self.config.role == Role::Custodian
    }

    /// Bytes for the peer, after anything that waits for a sync.
    fn send_client(&mut self, bytes: Vec<u8>, out: &mut Vec<Action>) {
        if self.outbox.is_empty() {
            out.push(Action::ToClient(bytes));
        } else if let Some(last) = self.outbox.back_mut().filter(|s| s.ticket.is_none()) {
            last.bytes.extend_from_slice(&bytes);
        } else {
            self.outbox.push_back(Segment {
                ticket: None,
                bytes,
            });
        }
    }

    /// A positive answer for `key`, held until the object is synced, and
    /// hashed if it proves another custodian's drop.
    fn hold(&mut self, line: &[u8], key: String, held: Held, out: &mut Vec<Action>) {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        let hash = self.config.peer_custodian && matches!(held, Held::Lock(_) | Held::Probe(_));
        self.held.push((ticket, held));
        out.push(Action::Sync { ticket, key, hash });
        let mut bytes = line.to_vec();
        bytes.push(b'\n');
        self.outbox.push_back(Segment {
            ticket: Some(ticket),
            bytes,
        });
    }

    fn release(&mut self, out: &mut Vec<Action>) {
        while self.outbox.front().is_some_and(|s| s.ticket.is_none()) {
            if let Some(segment) = self.outbox.pop_front() {
                out.push(Action::ToClient(segment.bytes));
            }
        }
    }

    /// A request the custodian must see answered before the next one.
    fn expect(&mut self, awaiting: Awaiting) -> Result<(), Closed> {
        if self.custodian() {
            if !self.awaiting.is_empty() {
                return Err(Closed::Pipelined);
            }
            self.awaiting.push_back(awaiting);
        }
        Ok(())
    }

    /// A request the filter answers itself, never forwarded.
    fn answer(&mut self, line: &[u8], out: &mut Vec<Action>) -> Result<(), Closed> {
        if !self.awaiting.is_empty() {
            return Err(Closed::Pipelined);
        }
        self.send_client(line.to_vec(), out);
        Ok(())
    }

    fn client_frame(&mut self, frame: Frame, out: &mut Vec<Action>) -> Result<(), Closed> {
        let line = match frame {
            Frame::Payload(bytes) => {
                out.push(Action::ToServer(bytes));
                return Ok(());
            }
            Frame::PayloadEnd => return Ok(()),
            Frame::Line(line) => line,
        };
        let name = word(&line);
        if self.relaying {
            if matches!(name, b"DATA" | b"CONNECTDONE" | b"ERROR") {
                out.push(Action::ToServer(with_newline(&line)));
                return Ok(());
            }
            return Err(Closed::UnknownMessage);
        }
        let key = || {
            arguments(&line)
                .last()
                .filter(|k| !k.is_empty())
                .map(|k| String::from_utf8_lossy(k).into_owned())
                .ok_or(Closed::BadArguments)
        };
        match name {
            b"VERSION" => {
                let digits = line.get(8..).unwrap_or_default();
                if digits.is_empty() || digits.len() > 9 || !digits.iter().all(u8::is_ascii_digit) {
                    return Err(Closed::BadVersion);
                }
                let asked: u64 = String::from_utf8_lossy(digits).parse().unwrap_or(0);
                self.expect(Awaiting::Version)?;
                let capped = format!("VERSION {}\n", asked.min(MAX_VERSION));
                out.push(Action::ToServer(capped.into_bytes()));
            }
            b"AUTH" => {
                let args = arguments(&line);
                let uuid = args
                    .first()
                    .filter(|u| !u.is_empty())
                    .ok_or(Closed::BadArguments)?;
                if let Some(expected) = &self.config.peer_uuid
                    && *uuid != expected.as_bytes()
                {
                    return Err(Closed::WrongPeer);
                }
                self.expect(Awaiting::Reply)?;
                let forwarded = match &self.config.auth_token {
                    Some(token) => {
                        let mut l = b"AUTH ".to_vec();
                        l.extend_from_slice(uuid);
                        l.push(b' ');
                        l.extend_from_slice(token.as_bytes());
                        l.push(b'\n');
                        l
                    }
                    None => with_newline(&line),
                };
                out.push(Action::ToServer(forwarded));
            }
            b"REMOVE" | b"REMOVE-BEFORE" => {
                if self.custodian() {
                    out.push(Action::Refused {
                        rule: Rule::Removal,
                        line: line.clone(),
                    });
                    self.answer(b"FAILURE\n", out)?;
                } else {
                    out.push(Action::ToServer(with_newline(&line)));
                }
            }
            b"CONNECT" => {
                if line == UPLOAD_PACK {
                    self.relaying = true;
                    out.push(Action::ToServer(with_newline(&line)));
                } else {
                    out.push(Action::Refused {
                        rule: Rule::Push,
                        line: line.clone(),
                    });
                    self.send_client(PUSH_REFUSED.to_vec(), out);
                    return Err(Closed::Push);
                }
            }
            b"CHECKPRESENT" if self.custodian() => {
                let k = key()?;
                if self.checking {
                    self.answer(b"FAILURE\n", out)?;
                } else {
                    self.expect(Awaiting::Probe(k.clone()))?;
                    out.push(Action::ToServer(format!("LOCKCONTENT {k}\n").into_bytes()));
                }
            }
            b"LOCKCONTENT" if self.custodian() => {
                let k = key()?;
                if self.checking {
                    self.answer(b"FAILURE\n", out)?;
                } else {
                    self.expect(Awaiting::Lock(k))?;
                    out.push(Action::ToServer(with_newline(&line)));
                }
            }
            b"PUT" if self.custodian() => {
                let k = key()?;
                self.expect(Awaiting::Put(k))?;
                out.push(Action::ToServer(with_newline(&line)));
            }
            b"GET" => {
                self.expect(Awaiting::Get)?;
                out.push(Action::ToServer(with_newline(&line)));
            }
            b"CHECKPRESENT" | b"LOCKCONTENT" | b"PUT" | b"GETTIMESTAMP" | b"NOTIFYCHANGE" => {
                self.expect(Awaiting::Reply)?;
                out.push(Action::ToServer(with_newline(&line)));
            }
            _ if ALLOWED.iter().any(|a| a.as_bytes() == name) => {
                out.push(Action::ToServer(with_newline(&line)));
            }
            _ => return Err(Closed::UnknownMessage),
        }
        Ok(())
    }

    fn server_frame(&mut self, frame: Frame, out: &mut Vec<Action>) -> Result<(), Closed> {
        let line = match frame {
            Frame::Payload(bytes) => {
                self.send_client(bytes, out);
                return Ok(());
            }
            Frame::PayloadEnd => {
                if self.version == 0 && self.awaiting.front() == Some(&Awaiting::Get) {
                    self.awaiting.pop_front();
                }
                return Ok(());
            }
            Frame::Line(line) => line,
        };
        let name = word(&line);
        if self.relaying || matches!(name, b"DATA" | b"CONNECTDONE") {
            self.send_client(with_newline(&line), out);
            return Ok(());
        }
        if matches!(name, b"VALID" | b"INVALID") {
            if self.awaiting.front() == Some(&Awaiting::Get) {
                self.awaiting.pop_front();
            }
            self.send_client(with_newline(&line), out);
            return Ok(());
        }
        let positive = matches!(
            name,
            b"SUCCESS" | b"SUCCESS-PLUS" | b"ALREADY-HAVE" | b"ALREADY-HAVE-PLUS"
        );
        match self.awaiting.pop_front() {
            Some(Awaiting::Version) => {
                let v = line.get(8..).unwrap_or_default();
                self.version = String::from_utf8_lossy(v)
                    .parse::<u64>()
                    .unwrap_or(0)
                    .min(MAX_VERSION);
                self.send_client(with_newline(&line), out);
            }
            Some(Awaiting::Probe(k)) => {
                if name == b"SUCCESS" {
                    self.hold(b"SUCCESS", k.clone(), Held::Probe(k), out);
                } else {
                    self.send_client(b"FAILURE\n".to_vec(), out);
                }
            }
            Some(Awaiting::Lock(k)) if name == b"SUCCESS" => {
                self.hold(&line, k.clone(), Held::Lock(k), out);
            }
            Some(Awaiting::Put(k)) if name == b"PUT-FROM" => {
                self.awaiting.push_front(Awaiting::PutData(k));
                self.send_client(with_newline(&line), out);
            }
            Some(Awaiting::Put(k) | Awaiting::PutData(k)) if positive => {
                self.hold(&line, k, Held::Put, out);
            }
            Some(_) => self.send_client(with_newline(&line), out),
            None if positive && self.custodian() => return Err(Closed::Unattributed),
            None => self.send_client(with_newline(&line), out),
        }
        Ok(())
    }
}

fn with_newline(line: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(line.len() + 1);
    bytes.extend_from_slice(line);
    bytes.push(b'\n');
    bytes
}
