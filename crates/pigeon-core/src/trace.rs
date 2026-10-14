//! Trace events, for checking runs of the code against TLA+ (12 §12.3).
//!
//! One line of NDJSON per action of a specification: `spec`, `device`,
//! `clock`, `action` and `vars`, read by `scripts/trace-to-tla` (spec 13 §13.3).
//! Each specification with events has one enum here, one variant per
//! action; a test checks that each name is an operator of its `.tla` file.
//! The module exists only with the cargo feature `tla-trace`.
//!
//! Laws (`tests/trace.rs`): a line is one JSON object that parses back to
//! the event; the Lamport clock never decreases and orders a receive after
//! the matching send.

use std::fmt::Write;

/// A value of `vars`, in the encodings `trace-to-tla` accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// An integer.
    Int(i64),
    /// A boolean.
    Bool(bool),
    /// A string of printable ASCII.
    Str(String),
    /// A sequence: a TLA+ tuple.
    List(Vec<Value>),
    /// A non-empty record, keys in order.
    Record(Vec<(String, Value)>),
}

/// One trace event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// The TLA+ specification.
    pub spec: &'static str,
    /// The device that logs it.
    pub device: String,
    /// Its Lamport clock.
    pub clock: u64,
    /// The action's name: an operator of the specification.
    pub action: &'static str,
    /// The action's parameters and the variables it set.
    pub vars: Vec<(String, Value)>,
}

fn string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn value(out: &mut String, v: &Value) {
    match v {
        Value::Int(n) => {
            let _ = write!(out, "{n}");
        }
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Str(s) => string(out, s),
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                value(out, item);
            }
            out.push(']');
        }
        Value::Record(fields) => fields_of(out, fields),
    }
}

fn fields_of(out: &mut String, fields: &[(String, Value)]) {
    out.push('{');
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        string(out, k);
        out.push(':');
        value(out, v);
    }
    out.push('}');
}

impl Event {
    /// The event as one line of NDJSON, without the newline.
    pub fn to_ndjson(&self) -> String {
        let mut out = String::new();
        out.push_str("{\"spec\":");
        string(&mut out, self.spec);
        out.push_str(",\"device\":");
        string(&mut out, &self.device);
        let _ = write!(out, ",\"clock\":{}", self.clock);
        out.push_str(",\"action\":");
        string(&mut out, self.action);
        out.push_str(",\"vars\":");
        fields_of(&mut out, &self.vars);
        out.push('}');
        out
    }
}

/// A device's Lamport clock (12 §12.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lamport(pub u64);

impl Lamport {
    /// The clock of the next local event.
    pub fn tick(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }

    /// The clock of receiving a message stamped `sent`.
    pub fn receive(&mut self, sent: u64) -> u64 {
        self.0 = self.0.max(sent) + 1;
        self.0
    }
}

/// A specification's action names, for the test that compares them with
/// the operators of its `.tla` file.
pub trait Actions: Sized + Copy + 'static {
    /// The specification's module name.
    const SPEC: &'static str;
    /// Every action.
    const ALL: &'static [Self];
    /// The action's operator name.
    fn name(self) -> &'static str;
}

macro_rules! actions {
    ($(#[$doc:meta])* $enum:ident = $spec:literal { $($(#[$vdoc:meta])* $variant:ident),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $enum {
            $($(#[$vdoc])* $variant,)+
        }

        impl Actions for $enum {
            const SPEC: &'static str = $spec;
            const ALL: &'static [Self] = &[$($enum::$variant,)+];
            fn name(self) -> &'static str {
                match self {
                    $($enum::$variant => stringify!($variant),)+
                }
            }
        }
    };
}

actions! {
    /// The actions of `models/tla/Roster.tla`.
    Roster = "Roster" {
        /// A replica signs a roster change on its head.
        Commit,
        /// A replica merges a fetched commit, or fast-forwards to it.
        Merge,
    }
}

actions! {
    /// The actions of `models/tla/CaptureInstall.tla`: the person's, then
    /// pigeon's, then the group's.
    CaptureInstall = "CaptureInstall" {
        /// A person writes at the path.
        WriteAtP,
        /// A person writes through an open handle.
        WriteHandle,
        /// A person replaces the file atomically.
        Replace,
        /// A person deletes the file.
        Delete,
        /// A person opens a handle.
        Open,
        /// A person closes a handle.
        Close,
        /// The clock ticks.
        Tick,
        /// Capture records a version.
        Capture,
        /// `update-index --refresh`.
        Refresh,
        /// Install starts: stage and journal `begin`.
        Start,
        /// The group's version is taken.
        TakeGroup,
        /// Step 4: move aside.
        MoveAside,
        /// Step 5: the writer scan.
        Scan,
        /// Step 6: the check, then `proved`.
        Check,
        /// Step 7: rename in.
        RenameIn,
        /// Step 8: the index.
        Index,
        /// Step 9: `done`.
        Done,
        /// The aside is released.
        Release,
        /// A deferred install is retried.
        Retry,
        /// pigeond stops without warning.
        Crash,
        /// Recovery of one entry at start.
        Recover,
        /// Recovery ends.
        RecoverEnd,
        /// The group makes a new version.
        NewVersion,
        /// A divergence is decided.
        Decide,
        /// The person keeps their version.
        ResolveMine,
    }
}

impl Event {
    /// An event of action `a` of its specification.
    pub fn new<A: Actions>(
        a: A,
        device: impl Into<String>,
        clock: u64,
        vars: Vec<(String, Value)>,
    ) -> Event {
        Event {
            spec: A::SPEC,
            device: device.into(),
            clock,
            action: a.name(),
            vars,
        }
    }
}
