//! Times and spans, in whole seconds, as observations carry them (06 §6.1).
//!
//! The core never reads a clock: every function that needs the present takes
//! it as an argument, so its result is a function of its inputs.

/// A moment, in seconds since the Unix epoch (UTC).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(pub i64);

/// A length of time, in seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span(pub i64);

impl Span {
    /// A span of `n` minutes.
    pub const fn minutes(n: i64) -> Span {
        Span(n * 60)
    }

    /// A span of `n` hours.
    pub const fn hours(n: i64) -> Span {
        Span(n * 3600)
    }
}

impl Time {
    /// The span from `earlier` to `self`, negative if `earlier` is later.
    pub fn since(self, earlier: Time) -> Span {
        Span(self.0.saturating_sub(earlier.0))
    }

    /// Whether `self` lies within `span` before `now`, `now` included.
    pub fn within(self, span: Span, now: Time) -> bool {
        now.since(self) <= span
    }
}
